//! Data types for the invoice performance alert contract.

use soroban_sdk::{contracttype, Symbol};

/// Storage key enum for instance and persistent storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Instance: admin allowed to manage the contract.
    Admin,
    /// Instance: backend operator allowed to run the health checks.
    Operator,
    /// Persistent: performance metrics recorded for an invoice.
    Metrics(Symbol),
    /// Persistent: last alert emitted for an invoice and a risk type.
    LastAlert(Symbol, RiskType),
}

/// Risk categories evaluated for every invoice.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiskType {
    /// The invoice is not fully funded and closes within the next week.
    Underfunded,
    /// The payment due date has passed without a payment.
    Overdue,
    /// The payment due date is approaching without a payment.
    DefaultProximity,
}

/// Severity assigned to a risk alert.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    /// Low impact risk.
    Low,
    /// Medium impact risk.
    Medium,
    /// High impact risk.
    High,
}

/// Payment state of an invoice.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaymentStatus {
    /// The invoice is still awaiting payment.
    Pending,
    /// The invoice has been paid in full.
    Paid,
}

/// Performance metrics tracked for an invoice.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceMetrics {
    /// Identifier of the invoice the metrics belong to.
    pub invoice_id: Symbol,
    /// Amount the invoice needs to be fully funded.
    pub target_amount: i128,
    /// Amount currently funded.
    pub funded_amount: i128,
    /// Ledger timestamp the invoice closes (matures).
    pub maturity_timestamp: u64,
    /// Ledger timestamp the payment is due.
    pub due_date: u64,
    /// Whether the invoice is still awaiting payment.
    pub payment_status: PaymentStatus,
}

/// The last alert emitted for an invoice and a risk type.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlertRecord {
    /// Ledger timestamp the alert was emitted at.
    pub timestamp: u64,
    /// Severity carried by the alert.
    pub severity: Severity,
}

/// Structured payload of the `InvoiceAtRisk` event.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskAlert {
    /// Invoice the alert refers to.
    pub invoice_id: Symbol,
    /// Risk category that triggered the alert.
    pub risk_type: RiskType,
    /// Severity assigned to the risk.
    pub severity: Severity,
    /// Ledger timestamp the alert was emitted at.
    pub timestamp: u64,
}

/// Current risk flags and alert history of an invoice.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskStatus {
    /// Invoice the status refers to.
    pub invoice_id: Symbol,
    /// Funding is below the threshold and the invoice closes within a week.
    pub underfunded: bool,
    /// The payment due date has passed without a payment.
    pub overdue: bool,
    /// The payment due date is approaching without a payment.
    pub default_proximity: bool,
    /// Funding progress in basis points (10_000 = fully funded).
    pub funding_bps: u32,
    /// Whole days left before the invoice closes (0 once closed).
    pub days_to_close: u64,
    /// Whole days left before the payment is due (0 once due).
    pub days_to_due: u64,
    /// Timestamp of the most recent alert (0 when no alert was emitted).
    pub last_alert_at: u64,
    /// Risk type of the most recent alert.
    pub last_alert_type: Option<RiskType>,
    /// Severity of the most recent alert.
    pub last_severity: Option<Severity>,
}
