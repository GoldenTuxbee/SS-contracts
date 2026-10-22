//! Invoice performance alerting contract (issue #625).
//!
//! Monitors invoice performance metrics and emits structured risk alerts when
//! an invoice approaches maturity without being fully funded or shows late
//! payment indicators.
//!
//! The backend operator drives the contract: it records the performance
//! metrics of an invoice and calls `check_invoice_health` to evaluate the risk
//! indicators. Every triggered indicator that is not suppressed emits an
//! `InvoiceAtRisk` event carrying the invoice, the risk type and a severity.
//!
//! - `Underfunded`: funding below 80% of the target while the invoice closes
//!   within the next 7 days. The severity follows the funding ratio, 60% is
//!   low, 30% is medium and anything below is high.
//! - `Overdue`: the payment due date passed without a payment. Always high.
//! - `DefaultProximity`: the payment due date is within the next 7 days.
//!   The severity follows the time left, one day is high, three days is
//!   medium and anything below seven days is low.
//!
//! Alerts are suppressed for 24 hours per invoice and risk type, so a cron
//! job can poll the contract without flooding the subscribers.

#![no_std]

mod errors;
mod events;
mod types;

#[cfg(test)]
mod test;

pub use errors::Error;
pub use types::{
    AlertRecord, DataKey, InvoiceMetrics, PaymentStatus, RiskAlert, RiskStatus, RiskType, Severity,
};

use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};

/// Seconds in a day.
const SECONDS_PER_DAY: u64 = 86_400;

/// Basis point denominator (10_000 = 100%).
const BPS_DENOMINATOR: u32 = 10_000;

/// Funding below this ratio (80%) is treated as underfunded.
const FUNDING_THRESHOLD_BPS: u32 = 8_000;

/// Funding at or above this ratio (60%) is a low severity risk.
const UNDERFUNDED_LOW_BPS: u32 = 6_000;

/// Funding at or above this ratio (30%) is a medium severity risk, below it
/// the risk is high.
const UNDERFUNDED_MEDIUM_BPS: u32 = 3_000;

/// Window before the invoice closes in which the funding is evaluated (7 days).
const CLOSE_WINDOW_SECS: u64 = 7 * SECONDS_PER_DAY;

/// Window before the payment due date in which the default proximity is
/// evaluated (7 days).
const DEFAULT_WATCH_SECS: u64 = 7 * SECONDS_PER_DAY;

/// A default is imminent within this window (1 day).
const DEFAULT_HIGH_SECS: u64 = SECONDS_PER_DAY;

/// A default is approaching within this window (3 days).
const DEFAULT_MEDIUM_SECS: u64 = 3 * SECONDS_PER_DAY;

/// Duplicate alerts of the same risk type are suppressed for 24 hours.
const ALERT_SUPPRESSION_SECS: u64 = 24 * 60 * 60;

/// Outcome of evaluating the risk indicators of an invoice.
struct Evaluation {
    underfunded: bool,
    overdue: bool,
    default_proximity: bool,
    underfunded_severity: Severity,
    default_severity: Severity,
    funding_bps: u32,
    days_to_close: u64,
    days_to_due: u64,
}

#[contract]
pub struct InvoicePerformanceAlert;

#[contractimpl]
impl InvoicePerformanceAlert {
    /// Initialise the contract with the admin and the backend operator.
    ///
    /// The operator is the only address allowed to run the health checks.
    pub fn initialize(env: Env, admin: Address, operator: Address) -> Result<(), Error> {
        admin.require_auth();
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Operator, &operator);
        Ok(())
    }

    /// Admin-only: rotate the backend operator allowed to run the health
    /// checks.
    pub fn set_operator(env: Env, admin: Address, operator: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if stored_admin != admin {
            return Err(Error::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Operator, &operator);
        Ok(())
    }

    /// Record or update the performance metrics of an invoice.
    ///
    /// Callable by the admin and by the backend operator.
    pub fn set_invoice_metrics(
        env: Env,
        caller: Address,
        metrics: InvoiceMetrics,
    ) -> Result<(), Error> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        let operator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Operator)
            .ok_or(Error::NotInitialized)?;
        if caller != admin && caller != operator {
            return Err(Error::Unauthorized);
        }
        if metrics.target_amount <= 0 || metrics.funded_amount < 0 {
            return Err(Error::InvalidAmount);
        }
        if metrics.maturity_timestamp == 0 || metrics.due_date == 0 {
            return Err(Error::InvalidTimestamp);
        }

        let key = DataKey::Metrics(metrics.invoice_id.clone());
        env.storage().persistent().set(&key, &metrics);
        Ok(())
    }

    /// Operator-only: evaluate the risk indicators of an invoice.
    ///
    /// An `InvoiceAtRisk` event is emitted for every triggered indicator that
    /// was not emitted for the same invoice and risk type within the last 24
    /// hours. Returns the number of alerts emitted by this call.
    pub fn check_invoice_health(env: Env, invoice_id: Symbol) -> Result<u32, Error> {
        let operator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Operator)
            .ok_or(Error::NotInitialized)?;
        operator.require_auth();

        let metrics = load_metrics(&env, &invoice_id)?;

        let now = env.ledger().timestamp();
        let evaluation = evaluate(&metrics, now);

        let mut emitted = 0u32;
        if evaluation.underfunded {
            let risk = RiskType::Underfunded;
            let severity = evaluation.underfunded_severity;
            emitted += emit_alert(&env, &invoice_id, risk, severity);
        }
        if evaluation.overdue {
            let risk = RiskType::Overdue;
            let severity = Severity::High;
            emitted += emit_alert(&env, &invoice_id, risk, severity);
        }
        if evaluation.default_proximity {
            let risk = RiskType::DefaultProximity;
            let severity = evaluation.default_severity;
            emitted += emit_alert(&env, &invoice_id, risk, severity);
        }

        Ok(emitted)
    }

    /// View: the current risk flags of an invoice and its latest alert.
    ///
    /// The flags are evaluated against the current ledger timestamp, the
    /// alert history is the one recorded by the emitted alerts.
    pub fn get_risk_status(env: Env, invoice_id: Symbol) -> Result<RiskStatus, Error> {
        let metrics = load_metrics(&env, &invoice_id)?;

        let now = env.ledger().timestamp();
        let evaluation = evaluate(&metrics, now);

        let mut latest: Option<AlertRecord> = None;
        let mut latest_type: Option<RiskType> = None;
        for risk in [
            RiskType::Underfunded,
            RiskType::Overdue,
            RiskType::DefaultProximity,
        ] {
            if let Some(record) = last_alert(&env, &invoice_id, &risk) {
                if latest.is_none_or(|current| record.timestamp > current.timestamp) {
                    latest = Some(record);
                    latest_type = Some(risk);
                }
            }
        }

        Ok(RiskStatus {
            invoice_id,
            underfunded: evaluation.underfunded,
            overdue: evaluation.overdue,
            default_proximity: evaluation.default_proximity,
            funding_bps: evaluation.funding_bps,
            days_to_close: evaluation.days_to_close,
            days_to_due: evaluation.days_to_due,
            last_alert_at: latest.map_or(0, |record| record.timestamp),
            last_alert_type: latest_type,
            last_severity: latest.map(|record| record.severity),
        })
    }

    /// View: the performance metrics recorded for an invoice.
    pub fn get_invoice_metrics(env: Env, invoice_id: Symbol) -> Result<InvoiceMetrics, Error> {
        load_metrics(&env, &invoice_id)
    }

    /// View: the last alert emitted for an invoice and a risk type.
    pub fn get_last_alert(env: Env, invoice_id: Symbol, risk: RiskType) -> Option<AlertRecord> {
        last_alert(&env, &invoice_id, &risk)
    }
}

/// The performance metrics recorded for an invoice.
fn load_metrics(env: &Env, invoice_id: &Symbol) -> Result<InvoiceMetrics, Error> {
    let key = DataKey::Metrics(invoice_id.clone());
    let metrics: Option<InvoiceMetrics> = env.storage().persistent().get(&key);
    metrics.ok_or(Error::InvoiceNotFound)
}

/// Evaluate the risk indicators of an invoice at the given ledger timestamp.
fn evaluate(metrics: &InvoiceMetrics, now: u64) -> Evaluation {
    let funding_bps = funding_bps(metrics.target_amount, metrics.funded_amount);
    let secs_to_close = metrics.maturity_timestamp.saturating_sub(now);
    let secs_to_due = metrics.due_date.saturating_sub(now);
    let unpaid = metrics.payment_status == PaymentStatus::Pending;

    // Underfunded: the invoice is not fully funded and closes within a week.
    let underfunded = unpaid
        && funding_bps < FUNDING_THRESHOLD_BPS
        && secs_to_close > 0
        && secs_to_close <= CLOSE_WINDOW_SECS;

    // Overdue: the payment deadline has passed without a payment.
    let overdue = unpaid && now > metrics.due_date;

    // Default proximity: the payment deadline is close and not passed yet.
    let default_proximity = unpaid && !overdue && secs_to_due <= DEFAULT_WATCH_SECS;

    Evaluation {
        underfunded,
        overdue,
        default_proximity,
        underfunded_severity: underfunded_severity(funding_bps),
        default_severity: default_severity(secs_to_due),
        funding_bps,
        days_to_close: days_remaining(secs_to_close),
        days_to_due: days_remaining(secs_to_due),
    }
}

/// Funding progress of an invoice in basis points.
fn funding_bps(target_amount: i128, funded_amount: i128) -> u32 {
    if target_amount <= 0 {
        return 0;
    }
    let bps = funded_amount.saturating_mul(BPS_DENOMINATOR as i128) / target_amount;
    if bps < 0 {
        0
    } else if bps > BPS_DENOMINATOR as i128 {
        BPS_DENOMINATOR
    } else {
        bps as u32
    }
}

/// Severity of an underfunded invoice, based on how much of the target was
/// funded.
fn underfunded_severity(funding_bps: u32) -> Severity {
    if funding_bps >= UNDERFUNDED_LOW_BPS {
        Severity::Low
    } else if funding_bps >= UNDERFUNDED_MEDIUM_BPS {
        Severity::Medium
    } else {
        Severity::High
    }
}

/// Severity of a default proximity alert, based on how close the payment
/// deadline is.
fn default_severity(secs_to_due: u64) -> Severity {
    if secs_to_due <= DEFAULT_HIGH_SECS {
        Severity::High
    } else if secs_to_due <= DEFAULT_MEDIUM_SECS {
        Severity::Medium
    } else {
        Severity::Low
    }
}

/// Whole days left until the given deadline.
fn days_remaining(secs: u64) -> u64 {
    secs.div_ceil(SECONDS_PER_DAY)
}

/// The last alert emitted for an invoice and a risk type.
fn last_alert(env: &Env, invoice_id: &Symbol, risk: &RiskType) -> Option<AlertRecord> {
    let key = DataKey::LastAlert(invoice_id.clone(), *risk);
    env.storage().persistent().get(&key)
}

/// Whether an alert for the invoice and the risk type was emitted within the
/// suppression window.
fn is_suppressed(env: &Env, invoice_id: &Symbol, risk: &RiskType, now: u64) -> bool {
    match last_alert(env, invoice_id, risk) {
        Some(record) => now.saturating_sub(record.timestamp) < ALERT_SUPPRESSION_SECS,
        None => false,
    }
}

/// Emit the alert of a risk unless the same risk was alerted recently.
///
/// Returns the number of alerts emitted: one when the alert was published and
/// zero when it was suppressed.
fn emit_alert(env: &Env, invoice_id: &Symbol, risk: RiskType, severity: Severity) -> u32 {
    let now = env.ledger().timestamp();
    if is_suppressed(env, invoice_id, &risk, now) {
        return 0;
    }

    events::invoice_at_risk(env, invoice_id, &risk, &severity, now);

    let record = AlertRecord {
        timestamp: now,
        severity,
    };
    let key = DataKey::LastAlert(invoice_id.clone(), risk);
    env.storage().persistent().set(&key, &record);
    1
}
