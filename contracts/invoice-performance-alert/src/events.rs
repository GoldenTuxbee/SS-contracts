//! Event definitions for the invoice performance alert contract.

use crate::types::{RiskAlert, RiskType, Severity};
use soroban_sdk::{Env, Symbol};

/// Emitted when an invoice crosses one of the monitored risk thresholds.
///
/// The invoice id is repeated in the event data so that a single event
/// carries the full alert: the invoice, the risk type and its severity.
pub fn invoice_at_risk(
    env: &Env,
    invoice_id: &Symbol,
    risk_type: &RiskType,
    severity: &Severity,
    timestamp: u64,
) {
    env.events().publish(
        (Symbol::new(env, "InvoiceAtRisk"), invoice_id.clone()),
        RiskAlert {
            invoice_id: invoice_id.clone(),
            risk_type: *risk_type,
            severity: *severity,
            timestamp,
        },
    );
}
