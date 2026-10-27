//! Invoice milestone payment contract for staged disbursement (issue #654).
//!
//! Releases invoice proceeds to the issuer in tranches tied to verified
//! project milestones, protecting investors until deliverables are confirmed.

#![no_std]

mod errors;
mod events;
mod types;

#[cfg(test)]
mod test;

pub use errors::Error;
pub use types::{Milestone, MilestoneStatus, StorageKey};

use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, Vec};

fn require_admin(env: &Env) -> Result<(), Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&StorageKey::Admin)
        .ok_or(Error::NotInitialized)?;
    admin.require_auth();
    Ok(())
}

#[contract]
pub struct InvoiceMilestone;

#[contractimpl]
impl InvoiceMilestone {
    /// Initialise the contract.
    pub fn initialize(env: Env, admin: Address, payment_token: Address) -> Result<(), Error> {
        if env.storage().instance().has(&StorageKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&StorageKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&StorageKey::PaymentToken, &payment_token);
        Ok(())
    }

    /// Create milestone schedule for an invoice. Called at invoice creation.
    pub fn create_milestones(
        env: Env,
        invoice_id: BytesN<32>,
        issuer: Address,
        milestones: Vec<Milestone>,
    ) -> Result<(), Error> {
        require_admin(&env)?;
        if env
            .storage()
            .persistent()
            .has(&StorageKey::Milestones(invoice_id.clone()))
        {
            return Err(Error::MilestonesAlreadySet);
        }
        if milestones.is_empty() {
            return Err(Error::InvalidAmount);
        }
        // Validate all amounts are positive.
        for i in 0..milestones.len() {
            let m = milestones.get(i).unwrap();
            if m.amount <= 0 {
                return Err(Error::InvalidAmount);
            }
        }
        env.storage()
            .persistent()
            .set(&StorageKey::Milestones(invoice_id.clone()), &milestones);
        env.storage()
            .persistent()
            .set(&StorageKey::Issuer(invoice_id), &issuer);
        Ok(())
    }

    /// Admin-only: verify a milestone and release its tranche to the issuer.
    pub fn verify_milestone(
        env: Env,
        invoice_id: BytesN<32>,
        milestone_index: u32,
    ) -> Result<(), Error> {
        require_admin(&env)?;

        let mut milestones: Vec<Milestone> = env
            .storage()
            .persistent()
            .get(&StorageKey::Milestones(invoice_id.clone()))
            .ok_or(Error::InvoiceNotFound)?;

        if milestone_index >= milestones.len() {
            return Err(Error::InvalidIndex);
        }

        let mut m = milestones.get(milestone_index).unwrap();
        if m.verified {
            return Err(Error::AlreadyVerified);
        }
        m.verified = true;
        milestones.set(milestone_index, m.clone());
        env.storage()
            .persistent()
            .set(&StorageKey::Milestones(invoice_id.clone()), &milestones);

        let issuer: Address = env
            .storage()
            .persistent()
            .get(&StorageKey::Issuer(invoice_id.clone()))
            .ok_or(Error::InvoiceNotFound)?;

        let payment_token: Address = env
            .storage()
            .instance()
            .get(&StorageKey::PaymentToken)
            .ok_or(Error::NotInitialized)?;

        let token_client = token::Client::new(&env, &payment_token);
        token_client.transfer(&env.current_contract_address(), &issuer, &m.amount);

        events::milestone_verified(&env, &invoice_id, milestone_index);
        events::tranche_disbursed(&env, &invoice_id, milestone_index, m.amount);

        Ok(())
    }

    /// Returns all milestones for an invoice with their verified/pending status.
    pub fn get_milestone_status(
        env: Env,
        invoice_id: BytesN<32>,
    ) -> Result<Vec<MilestoneStatus>, Error> {
        let milestones: Vec<Milestone> = env
            .storage()
            .persistent()
            .get(&StorageKey::Milestones(invoice_id.clone()))
            .ok_or(Error::InvoiceNotFound)?;

        let mut statuses = Vec::new(&env);
        for i in 0..milestones.len() {
            let m = milestones.get(i).unwrap();
            statuses.push_back(MilestoneStatus {
                index: i,
                amount: m.amount,
                description_hash: m.description_hash,
                verified: m.verified,
            });
        }
        Ok(statuses)
    }
}
