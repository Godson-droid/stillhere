#![no_std]

//! StillHere: a dead-man's-switch vault on Tari Ootle.
//!
//! The owner deposits funds and names a beneficiary. The owner must call
//! `check_in` at least once every `check_in_interval_epochs` epochs. If the
//! window lapses, anyone can call `release`, which pays the entire vault
//! balance directly to the beneficiary's account. While the owner is active,
//! only the holder of the owner badge can check in, withdraw, change the
//! beneficiary, or close the vault.

extern crate alloc;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use tari_template_lib::prelude::*;

// Use talc as the global allocator
#[cfg(not(test))]
#[global_allocator]
static ALLOCATOR: talc::TalckWasm = unsafe { talc::TalckWasm::new_global() };

#[template]
mod still_here {
    use super::*;

    /// Vault lifecycle, stored as a `u8` in component state. `Active` is the
    /// only state in which funds can move at the owner's direction;
    /// `Released` and `Closed` are terminal.
    pub const STATE_ACTIVE: u8 = 0;
    pub const STATE_RELEASED: u8 = 1;
    pub const STATE_CLOSED: u8 = 2;

    pub struct StillHere {
        vault: Vault,
        beneficiary: ComponentAddress,
        check_in_interval_epochs: u64,
        last_check_in_epoch: u64,
        deadline_epoch: u64,
        owner_badge_resource: ResourceAddress,
        state: u8,
    }

    impl StillHere {
        /// Creates a new vault.
        ///
        /// Takes ownership of `initial_deposit` (must be a positive amount of a
        /// fungible resource), names `beneficiary` as the account that will
        /// receive the funds if the heartbeat lapses, and sets the check-in
        /// window to `check_in_interval_epochs` epochs (must be at least 1).
        ///
        /// Returns the component and a bucket containing the single owner badge.
        /// Whoever holds the owner badge controls the vault: the badge is the
        /// only way to check in, withdraw, change the beneficiary, or close.
        /// Exactly one badge is ever minted and minting is locked at creation.
        pub fn new(
            initial_deposit: Bucket,
            beneficiary: ComponentAddress,
            check_in_interval_epochs: u64,
        ) -> (Component<Self>, Bucket) {
            assert!(
                check_in_interval_epochs > 0,
                "Check-in interval must be at least 1 epoch"
            );
            assert!(
                initial_deposit.amount().is_positive(),
                "Initial deposit must be greater than zero"
            );
            let resource = initial_deposit.resource_address();
            // Amount-based resources only: public fungible tokens and stealth
            // tokens (TARI itself is a stealth resource). Non-fungible and
            // confidential resources cannot be handled by this template's
            // amount-based vault logic and are refused.
            let resource_type = ResourceManager::get(resource).resource_type();
            assert!(
                matches!(resource_type, ResourceType::Fungible | ResourceType::Stealth),
                "Vault resource must be a fungible or stealth token, got {:?}",
                resource_type
            );

            // The owner badge: exactly one exists, it can never be minted again
            // or burned, and its rules are locked so nobody can change that.
            let owner_badge = ResourceBuilder::non_fungible()
                .with_token_symbol("SHOWN")
                .add_metadata("name", "StillHere Owner")
                .mintable(AccessRule::DenyAll, LOCKED)
                .burnable(AccessRule::DenyAll, LOCKED)
                .initial_supply([NonFungibleId::from_u64(0)]);
            let owner_badge_resource = owner_badge.resource_address();
            let require_owner = rule!(resource(owner_badge_resource));

            let now = Consensus::current_epoch();
            let deadline_epoch = now + check_in_interval_epochs;
            let initial_amount = initial_deposit.amount();

            // Owner-only methods are gated on presenting a proof of the owner
            // badge. Everything not listed defaults to AllowAll: deposits,
            // release, and the read-only views are intentionally public.
            let access_rules = AccessRules::new()
                .add_method_rule("check_in", require_owner.clone())
                .add_method_rule("withdraw", require_owner.clone())
                .add_method_rule("set_beneficiary", require_owner.clone())
                .add_method_rule("close", require_owner.clone())
                .default(AccessRule::AllowAll);

            let component = Component::new(Self {
                vault: Vault::from_bucket(initial_deposit),
                beneficiary,
                check_in_interval_epochs,
                last_check_in_epoch: now,
                deadline_epoch,
                owner_badge_resource,
                state: STATE_ACTIVE,
            })
            // SECURITY: the component deliberately has NO owner
            // (`OwnerRule::None`). In the Ootle engine a component owner can
            // call any method and can rewrite the access rules, which would
            // leave the creating account with permanent control even after
            // handing over the badge. With no owner, the badge method rules
            // above are the only authority there will ever be, and they can
            // never be changed after creation.
            .with_owner_rule(OwnerRule::None)
            .with_access_rules(access_rules)
            .create();

            let mut payload = Metadata::new();
            payload.insert("beneficiary", &beneficiary);
            payload.insert("check_in_interval_epochs", &check_in_interval_epochs);
            payload.insert("created_epoch", &now);
            payload.insert("deadline_epoch", &deadline_epoch);
            payload.insert("initial_amount", &initial_amount);
            emit_event("still_here.created", payload);

            (component, owner_badge)
        }

        /// Deposits more funds into the vault. Anyone may top up a vault; the
        /// resource must match the vault's resource. Depositing does NOT reset
        /// the heartbeat: only `check_in` does.
        pub fn deposit(&mut self, bucket: Bucket) {
            self.assert_active();
            assert_eq!(
                bucket.resource_address(),
                self.vault.resource_address(),
                "Deposit resource does not match the vault resource"
            );
            assert!(
                bucket.amount().is_positive(),
                "Deposit amount must be greater than zero"
            );
            let amount = bucket.amount();
            self.vault.deposit(bucket);

            let mut payload = Metadata::new();
            payload.insert("amount", &amount);
            payload.insert("balance", &self.vault.balance());
            emit_event("still_here.deposit", payload);
        }

        /// The heartbeat. Owner only. Resets the deadline to
        /// `current_epoch + check_in_interval_epochs`. A check-in that arrives
        /// after the deadline but before anyone has released the vault revives
        /// it: the owner is demonstrably back, so the switch resets.
        pub fn check_in(&mut self) {
            self.assert_active();
            let now = Consensus::current_epoch();
            self.last_check_in_epoch = now;
            self.deadline_epoch = now + self.check_in_interval_epochs;

            let mut payload = Metadata::new();
            payload.insert("epoch", &now);
            payload.insert("deadline_epoch", &self.deadline_epoch);
            emit_event("still_here.check_in", payload);
        }

        /// Withdraws `amount` back to the owner. Owner only. Withdrawing does
        /// NOT reset the heartbeat: an owner who drains the vault still has to
        /// check in, and an empty vault that lapses releases nothing.
        pub fn withdraw(&mut self, amount: Amount) -> Bucket {
            self.assert_active();
            assert!(amount.is_positive(), "Withdrawal amount must be greater than zero");
            assert!(
                amount <= self.vault.balance(),
                "Withdrawal amount exceeds the vault balance"
            );
            let bucket = self.vault.withdraw(amount);

            let mut payload = Metadata::new();
            payload.insert("amount", &amount);
            payload.insert("balance", &self.vault.balance());
            emit_event("still_here.withdraw", payload);

            bucket
        }

        /// Changes the beneficiary. Owner only. The new beneficiary receives
        /// the funds if the vault is released from this point on.
        pub fn set_beneficiary(&mut self, new_beneficiary: ComponentAddress) {
            self.assert_active();
            let old_beneficiary = self.beneficiary;
            self.beneficiary = new_beneficiary;

            let mut payload = Metadata::new();
            payload.insert("old_beneficiary", &old_beneficiary);
            payload.insert("new_beneficiary", &new_beneficiary);
            emit_event("still_here.beneficiary_changed", payload);
        }

        /// Closes the vault and returns the full balance to the owner. Owner
        /// only. Terminal: a closed vault can never be released, deposited to,
        /// or checked into.
        pub fn close(&mut self) -> Bucket {
            self.assert_active();
            self.state = STATE_CLOSED;
            let bucket = self.vault.withdraw_all();

            let mut payload = Metadata::new();
            payload.insert("amount", &bucket.amount());
            payload.insert("epoch", &Consensus::current_epoch());
            emit_event("still_here.closed", payload);

            bucket
        }

        /// Releases the vault to the beneficiary. Callable by ANYONE, but only
        /// once the heartbeat has lapsed (current epoch is at or past the
        /// deadline). The full balance is deposited directly into the
        /// beneficiary's account component by the vault itself, so the caller
        /// can never divert the funds. Terminal: a second release is refused.
        pub fn release(&mut self) {
            match self.state {
                STATE_ACTIVE => {},
                STATE_RELEASED => panic!("Vault has already been released"),
                _ => panic!("Vault is closed and cannot be released"),
            }
            let now = Consensus::current_epoch();
            assert!(
                now >= self.deadline_epoch,
                "Vault has not lapsed: the owner last checked in at epoch {} and release is possible from epoch {} (current epoch {})",
                self.last_check_in_epoch,
                self.deadline_epoch,
                now
            );

            self.state = STATE_RELEASED;
            let amount = self.vault.balance();
            let bucket = self.vault.withdraw_all();
            ComponentManager::get(self.beneficiary).invoke("deposit", args![bucket]);

            let mut payload = Metadata::new();
            payload.insert("beneficiary", &self.beneficiary);
            payload.insert("amount", &amount);
            payload.insert("epoch", &now);
            emit_event("still_here.released", payload);
        }

        pub fn balance(&self) -> Amount {
            self.vault.balance()
        }

        pub fn beneficiary(&self) -> ComponentAddress {
            self.beneficiary
        }

        pub fn deadline_epoch(&self) -> u64 {
            self.deadline_epoch
        }

        pub fn last_check_in_epoch(&self) -> u64 {
            self.last_check_in_epoch
        }

        pub fn check_in_interval_epochs(&self) -> u64 {
            self.check_in_interval_epochs
        }

        pub fn owner_badge_resource(&self) -> ResourceAddress {
            self.owner_badge_resource
        }

        /// The lifecycle state as a label: "Active", "Released", or "Closed".
        pub fn state(&self) -> String {
            match self.state {
                STATE_ACTIVE => "Active".to_string(),
                STATE_RELEASED => "Released".to_string(),
                _ => "Closed".to_string(),
            }
        }

        /// True while the vault is Active and the current epoch is at or past
        /// the deadline, meaning anyone may call `release`.
        pub fn is_lapsed(&self) -> bool {
            self.state == STATE_ACTIVE && Consensus::current_epoch() >= self.deadline_epoch
        }

        fn assert_active(&self) {
            match self.state {
                STATE_ACTIVE => {},
                STATE_RELEASED => panic!("Vault has already been released"),
                _ => panic!("Vault is closed"),
            }
        }
    }
}
