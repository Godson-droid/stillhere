//! Engine tests for the StillHere template.
//!
//! These run the real Ootle engine in-process via `tari_template_test_tooling`.
//! Time is controlled by injecting the `CurrentEpoch` virtual substate, which
//! is exactly what `Consensus::current_epoch()` reads inside the template.

use tari_template_lib::models::Bucket;
use tari_template_lib::types::constants::TARI_TOKEN;
use tari_template_lib::types::{Amount, ComponentAddress, NonFungibleAddress, ResourceAddress};
use tari_template_test_tooling::crypto::RistrettoSecretKey;
use tari_template_test_tooling::engine_types::virtual_substate::{VirtualSubstate, VirtualSubstateId};
use tari_template_test_tooling::transaction::args;
use tari_template_test_tooling::TemplateTest;

const TEMPLATE_NAME: &str = "StillHere";
/// The harness funds accounts with 1,000,000,000 XTR from the faucet.
const FAUCET_BALANCE: u64 = 1_000_000_000;
const DEPOSIT: u64 = 500_000;
const INTERVAL: u64 = 10;

struct Setup {
    vault: ComponentAddress,
    owner: ComponentAddress,
    owner_proof: NonFungibleAddress,
    owner_key: RistrettoSecretKey,
    beneficiary: ComponentAddress,
    badge_resource: ResourceAddress,
}

fn set_epoch(test: &mut TemplateTest, epoch: u64) {
    test.set_virtual_substate(
        VirtualSubstateId::CurrentEpoch,
        VirtualSubstate::CurrentEpoch(epoch),
    );
}

/// Creates a vault at epoch 0 owned by a fresh funded account, with a fresh
/// empty account as beneficiary. The owner badge ends up in the owner's account.
fn create_vault(test: &mut TemplateTest, deposit: u64, interval: u64) -> Setup {
    let (owner, owner_proof, owner_key) = test.create_funded_account();
    let (beneficiary, _, _) = test.create_empty_account();
    let template = test.get_template_address(TEMPLATE_NAME);

    test.execute_expect_success(
        test.transaction()
            .call_method(owner, "withdraw", args![TARI_TOKEN, deposit])
            .put_last_instruction_output_on_workspace("deposit")
            .call_function(
                template,
                "new",
                args![Workspace("deposit"), beneficiary, interval],
            )
            .put_last_instruction_output_on_workspace("created")
            .call_method(owner, "deposit", args![Workspace("created.1")])
            .build_and_seal(&owner_key),
        vec![owner_proof.clone()],
    );
    // Return values are indexed by instruction, and the workspace puts count
    // as instructions too: 0 = withdraw, 1 = put on workspace, 2 = `new`,
    // 3 = put on workspace, 4 = deposit the badge bucket into the owner.
    let (vault, _badge_bucket) = test.expect_return::<(ComponentAddress, Bucket)>(2);
    let badge_resource: ResourceAddress =
        test.call_method(vault, "owner_badge_resource", args![], vec![]);

    Setup {
        vault,
        owner,
        owner_proof,
        owner_key,
        beneficiary,
        badge_resource,
    }
}

fn xtr_balance(test: &mut TemplateTest, account: ComponentAddress) -> Amount {
    test.call_method(account, "balance", args![TARI_TOKEN], vec![])
}

fn vault_balance(test: &mut TemplateTest, vault: ComponentAddress) -> Amount {
    test.call_method(vault, "balance", args![], vec![])
}

fn vault_state(test: &mut TemplateTest, vault: ComponentAddress) -> String {
    test.call_method(vault, "state", args![], vec![])
}

fn vault_deadline(test: &mut TemplateTest, vault: ComponentAddress) -> u64 {
    test.call_method(vault, "deadline_epoch", args![], vec![])
}

fn vault_is_lapsed(test: &mut TemplateTest, vault: ComponentAddress) -> bool {
    test.call_method(vault, "is_lapsed", args![], vec![])
}

/// Builds the proof dance that owner-only methods require: create a proof of
/// the owner badge from the owner's account and authorize it for the call.
fn owner_check_in(test: &mut TemplateTest, setup: &Setup) {
    test.execute_expect_success(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "check_in", args![])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
}

/// Withdraws `amount` from the vault back into the owner's account, using
/// the same proof dance as `owner_check_in`.
fn owner_withdraw(test: &mut TemplateTest, setup: &Setup, amount: u64) {
    test.execute_expect_success(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "withdraw", args![amount])
            .put_last_instruction_output_on_workspace("taken")
            .call_method(setup.owner, "deposit", args![Workspace("taken")])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
}

/// Changes the vault beneficiary, using the same proof dance as
/// `owner_check_in`.
fn owner_set_beneficiary(test: &mut TemplateTest, setup: &Setup, new_beneficiary: ComponentAddress) {
    test.execute_expect_success(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "set_beneficiary", args![new_beneficiary])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
}

/// Closes the vault, returning the full balance to the owner's account,
/// using the same proof dance as `owner_check_in`.
fn owner_close(test: &mut TemplateTest, setup: &Setup) {
    test.execute_expect_success(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "close", args![])
            .put_last_instruction_output_on_workspace("returned")
            .call_method(setup.owner, "deposit", args![Workspace("returned")])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
}

// ---------------------------------------------------------------------------
// Creation
// ---------------------------------------------------------------------------

#[test]
fn create_records_the_vault_terms() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(DEPOSIT));
    assert_eq!(vault_state(&mut test, setup.vault), "Active");
    let beneficiary: ComponentAddress =
        test.call_method(setup.vault, "beneficiary", args![], vec![]);
    assert_eq!(beneficiary, setup.beneficiary);
    let interval: u64 = test.call_method(setup.vault, "check_in_interval_epochs", args![], vec![]);
    assert_eq!(interval, INTERVAL);
    let last_check_in: u64 = test.call_method(setup.vault, "last_check_in_epoch", args![], vec![]);
    assert_eq!(last_check_in, 0);
    // Created at epoch 0 with a 10 epoch window: deadline is epoch 10.
    assert_eq!(vault_deadline(&mut test, setup.vault), INTERVAL);
    let lapsed: bool = test.call_method(setup.vault, "is_lapsed", args![], vec![]);
    assert!(!lapsed);
    // The owner paid the deposit out of their faucet balance.
    assert_eq!(
        xtr_balance(&mut test, setup.owner),
        Amount::from(FAUCET_BALANCE - DEPOSIT)
    );
}

#[test]
fn create_with_zero_interval_is_rejected() {
    let mut test = TemplateTest::my_crate();
    let (owner, owner_proof, owner_key) = test.create_funded_account();
    let (beneficiary, _, _) = test.create_empty_account();
    let template = test.get_template_address(TEMPLATE_NAME);

    test.execute_expect_failure(
        test.transaction()
            .call_method(owner, "withdraw", args![TARI_TOKEN, DEPOSIT])
            .put_last_instruction_output_on_workspace("deposit")
            .call_function(template, "new", args![Workspace("deposit"), beneficiary, 0u64])
            .build_and_seal(&owner_key),
        vec![owner_proof],
    );
}

#[test]
fn create_with_zero_deposit_is_rejected() {
    let mut test = TemplateTest::my_crate();
    let (owner, owner_proof, owner_key) = test.create_funded_account();
    let (beneficiary, _, _) = test.create_empty_account();
    let template = test.get_template_address(TEMPLATE_NAME);

    // A zero-value bucket: withdrawing 0 from the account yields an empty
    // bucket, and the template must refuse to open a vault around it.
    test.execute_expect_failure(
        test.transaction()
            .call_method(owner, "withdraw", args![TARI_TOKEN, 0u64])
            .put_last_instruction_output_on_workspace("deposit")
            .call_function(
                template,
                "new",
                args![Workspace("deposit"), beneficiary, INTERVAL],
            )
            .build_and_seal(&owner_key),
        vec![owner_proof],
    );
}

// ---------------------------------------------------------------------------
// Deposits
// ---------------------------------------------------------------------------

#[test]
fn anyone_can_top_up_and_balance_accumulates() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // A complete stranger tops up the vault: deposits are public by design.
    let (stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_success(
        test.transaction()
            .call_method(stranger, "withdraw", args![TARI_TOKEN, 125_000u64])
            .put_last_instruction_output_on_workspace("topup")
            .call_method(setup.vault, "deposit", args![Workspace("topup")])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );

    assert_eq!(
        vault_balance(&mut test, setup.vault),
        Amount::from(DEPOSIT + 125_000)
    );
    // Depositing does not reset the heartbeat.
    assert_eq!(vault_deadline(&mut test, setup.vault), INTERVAL);
}

#[test]
fn zero_deposit_top_up_is_rejected() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.owner, "withdraw", args![TARI_TOKEN, 0u64])
            .put_last_instruction_output_on_workspace("empty")
            .call_method(setup.vault, "deposit", args![Workspace("empty")])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(DEPOSIT));
}

// ---------------------------------------------------------------------------
// Check-in / heartbeat
// ---------------------------------------------------------------------------

#[test]
fn check_in_extends_the_deadline() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    set_epoch(&mut test, 7);
    owner_check_in(&mut test, &setup);

    let last_check_in: u64 = test.call_method(setup.vault, "last_check_in_epoch", args![], vec![]);
    assert_eq!(last_check_in, 7);
    assert_eq!(vault_deadline(&mut test, setup.vault), 7 + INTERVAL);

    // The old deadline no longer releases the vault.
    set_epoch(&mut test, INTERVAL);
    let (_stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );
    assert_eq!(vault_state(&mut test, setup.vault), "Active");
}

#[test]
fn late_check_in_revives_a_lapsed_vault() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // The deadline (epoch 10) passes with no check-in and no release.
    set_epoch(&mut test, 15);
    let lapsed: bool = test.call_method(setup.vault, "is_lapsed", args![], vec![]);
    assert!(lapsed);

    // The owner resurfaces before anyone releases: the vault revives.
    owner_check_in(&mut test, &setup);
    assert_eq!(vault_deadline(&mut test, setup.vault), 15 + INTERVAL);
    let lapsed: bool = test.call_method(setup.vault, "is_lapsed", args![], vec![]);
    assert!(!lapsed);

    // And a release at the old lapse point is now refused.
    let (_stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );
}

// ---------------------------------------------------------------------------
// Release
// ---------------------------------------------------------------------------

#[test]
fn release_before_lapse_is_rejected() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    set_epoch(&mut test, INTERVAL - 1);
    let (_stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );

    assert_eq!(vault_state(&mut test, setup.vault), "Active");
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(DEPOSIT));
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::ZERO);
}

#[test]
fn release_at_deadline_pays_the_beneficiary_exactly() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // A third party (not the owner, not the beneficiary) triggers the release.
    let (caller, caller_proof, caller_key) = test.create_funded_account();
    let caller_before = xtr_balance(&mut test, caller);

    set_epoch(&mut test, INTERVAL);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&caller_key),
        vec![caller_proof],
    );

    // The beneficiary received the full vault balance, to the unit.
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::from(DEPOSIT));
    // The caller gained nothing: release cannot be diverted.
    assert_eq!(xtr_balance(&mut test, caller), caller_before);
    // The vault is empty and terminal.
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::ZERO);
    assert_eq!(vault_state(&mut test, setup.vault), "Released");
}

#[test]
fn release_pays_the_full_accumulated_balance() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // Top up twice before the lapse.
    for amount in [100_000u64, 250_000] {
        test.execute_expect_success(
            test.transaction()
                .call_method(setup.owner, "withdraw", args![TARI_TOKEN, amount])
                .put_last_instruction_output_on_workspace("topup")
                .call_method(setup.vault, "deposit", args![Workspace("topup")])
                .build_and_seal(&setup.owner_key),
            vec![setup.owner_proof.clone()],
        );
    }

    set_epoch(&mut test, INTERVAL + 3);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    assert_eq!(
        xtr_balance(&mut test, setup.beneficiary),
        Amount::from(DEPOSIT + 100_000 + 250_000)
    );
}

#[test]
fn double_release_is_rejected() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    set_epoch(&mut test, INTERVAL);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    set_epoch(&mut test, INTERVAL + 5);
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    // The beneficiary was paid exactly once.
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::from(DEPOSIT));
}

#[test]
fn release_after_owner_drained_the_vault_pays_zero_and_closes_the_switch() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // Owner withdraws everything while active (covered in detail elsewhere).
    test.execute_expect_success(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "withdraw", args![DEPOSIT])
            .put_last_instruction_output_on_workspace("drained")
            .call_method(setup.owner, "deposit", args![Workspace("drained")])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::ZERO);

    set_epoch(&mut test, INTERVAL);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::ZERO);
    assert_eq!(vault_state(&mut test, setup.vault), "Released");
}

// ---------------------------------------------------------------------------
// Owner-only enforcement
// ---------------------------------------------------------------------------

#[test]
fn non_owner_cannot_check_in() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    let (_stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "check_in", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );
    let last_check_in: u64 = test.call_method(setup.vault, "last_check_in_epoch", args![], vec![]);
    assert_eq!(last_check_in, 0);
}

#[test]
fn non_owner_cannot_withdraw() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    let (_stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "withdraw", args![100u64])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(DEPOSIT));
}

#[test]
fn non_owner_cannot_change_the_beneficiary() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    let (stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "set_beneficiary", args![stranger])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );
    let beneficiary: ComponentAddress =
        test.call_method(setup.vault, "beneficiary", args![], vec![]);
    assert_eq!(beneficiary, setup.beneficiary);
}

#[test]
fn non_owner_cannot_close() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    let (_stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "close", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );
    assert_eq!(vault_state(&mut test, setup.vault), "Active");
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(DEPOSIT));
}

#[test]
fn badge_holder_who_is_not_the_original_owner_can_check_in() {
    // Ownership is the badge, not the creating account: if the owner sends
    // the badge to another account, that account becomes the operator.
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    let (heir, heir_proof, heir_key) = test.create_funded_account();
    // Transfer the badge NFT from the owner account to the heir account.
    test.execute_expect_success(
        test.transaction()
            .call_method(
                setup.owner,
                "withdraw_non_fungible",
                args![setup.badge_resource, tari_template_lib::types::NonFungibleId::from_u64(0)],
            )
            .put_last_instruction_output_on_workspace("badge")
            .call_method(heir, "deposit", args![Workspace("badge")])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    // The original owner has lost control.
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "check_in", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    // The heir can check in.
    set_epoch(&mut test, 4);
    test.execute_expect_success(
        test.transaction()
            .create_proof(heir, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "check_in", args![])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&heir_key),
        vec![heir_proof],
    );
    assert_eq!(vault_deadline(&mut test, setup.vault), 4 + INTERVAL);
}

// ---------------------------------------------------------------------------
// Owner withdrawals and beneficiary changes
// ---------------------------------------------------------------------------

#[test]
fn owner_withdraws_part_and_the_heartbeat_is_unchanged() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let owner_before = xtr_balance(&mut test, setup.owner);

    test.execute_expect_success(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "withdraw", args![200_000u64])
            .put_last_instruction_output_on_workspace("taken")
            .call_method(setup.owner, "deposit", args![Workspace("taken")])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    assert_eq!(
        vault_balance(&mut test, setup.vault),
        Amount::from(DEPOSIT - 200_000)
    );
    assert_eq!(
        xtr_balance(&mut test, setup.owner),
        owner_before + Amount::from(200_000u64)
    );
    // Withdrawing is not a check-in.
    assert_eq!(vault_deadline(&mut test, setup.vault), INTERVAL);
}

#[test]
fn owner_cannot_withdraw_more_than_the_balance() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    test.execute_expect_failure(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "withdraw", args![DEPOSIT + 1])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(DEPOSIT));
}

#[test]
fn owner_cannot_withdraw_zero() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    test.execute_expect_failure(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "withdraw", args![0u64])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
}

#[test]
fn changing_the_beneficiary_redirects_the_release() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let (new_beneficiary, _, _) = test.create_empty_account();

    test.execute_expect_success(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "set_beneficiary", args![new_beneficiary])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    set_epoch(&mut test, INTERVAL);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    assert_eq!(xtr_balance(&mut test, new_beneficiary), Amount::from(DEPOSIT));
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::ZERO);
}

// ---------------------------------------------------------------------------
// Close and terminal-state behaviour
// ---------------------------------------------------------------------------

#[test]
fn owner_close_returns_everything_and_blocks_release() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let owner_before = xtr_balance(&mut test, setup.owner);

    test.execute_expect_success(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "close", args![])
            .put_last_instruction_output_on_workspace("returned")
            .call_method(setup.owner, "deposit", args![Workspace("returned")])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    assert_eq!(vault_state(&mut test, setup.vault), "Closed");
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::ZERO);
    assert_eq!(
        xtr_balance(&mut test, setup.owner),
        owner_before + Amount::from(DEPOSIT)
    );

    // Even long after the deadline, a closed vault cannot be released.
    set_epoch(&mut test, INTERVAL * 3);
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::ZERO);
}

#[test]
fn terminal_vaults_refuse_further_actions() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    set_epoch(&mut test, INTERVAL);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    // Deposit after release.
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.owner, "withdraw", args![TARI_TOKEN, 10u64])
            .put_last_instruction_output_on_workspace("late")
            .call_method(setup.vault, "deposit", args![Workspace("late")])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    // Check-in after release (owner badge presented correctly; state refuses).
    test.execute_expect_failure(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "check_in", args![])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    // Withdraw after release.
    test.execute_expect_failure(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "withdraw", args![10u64])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
}

// ---------------------------------------------------------------------------
// Badge transfer: the old holder loses everything, the new holder gains it
// ---------------------------------------------------------------------------

/// Moves the owner badge from the setup owner to a fresh funded account and
/// returns that account. After this call the original owner holds nothing.
fn transfer_badge(
    test: &mut TemplateTest,
    setup: &Setup,
) -> (ComponentAddress, NonFungibleAddress, RistrettoSecretKey) {
    let (heir, heir_proof, heir_key) = test.create_funded_account();
    test.execute_expect_success(
        test.transaction()
            .call_method(
                setup.owner,
                "withdraw_non_fungible",
                args![setup.badge_resource, tari_template_lib::types::NonFungibleId::from_u64(0)],
            )
            .put_last_instruction_output_on_workspace("badge")
            .call_method(heir, "deposit", args![Workspace("badge")])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    (heir, heir_proof, heir_key)
}

#[test]
fn transferred_badge_old_holder_cannot_withdraw() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let (_heir, _heir_proof, _heir_key) = transfer_badge(&mut test, &setup);

    // The old holder no longer holds the badge, so the proof dance itself
    // cannot be completed and the withdrawal is refused.
    test.execute_expect_failure(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "withdraw", args![100u64])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(DEPOSIT));
}

#[test]
fn transferred_badge_old_holder_cannot_set_beneficiary() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let (_heir, _heir_proof, _heir_key) = transfer_badge(&mut test, &setup);
    let (attacker, _, _) = test.create_empty_account();

    test.execute_expect_failure(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "set_beneficiary", args![attacker])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    let beneficiary: ComponentAddress =
        test.call_method(setup.vault, "beneficiary", args![], vec![]);
    assert_eq!(beneficiary, setup.beneficiary);
}

#[test]
fn transferred_badge_old_holder_cannot_close() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let (_heir, _heir_proof, _heir_key) = transfer_badge(&mut test, &setup);

    test.execute_expect_failure(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "close", args![])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(vault_state(&mut test, setup.vault), "Active");
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(DEPOSIT));
}

#[test]
fn transferred_badge_new_holder_can_withdraw() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let (heir, heir_proof, heir_key) = transfer_badge(&mut test, &setup);
    let heir_before = xtr_balance(&mut test, heir);

    test.execute_expect_success(
        test.transaction()
            .create_proof(heir, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "withdraw", args![150_000u64])
            .put_last_instruction_output_on_workspace("taken")
            .call_method(heir, "deposit", args![Workspace("taken")])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&heir_key),
        vec![heir_proof],
    );

    assert_eq!(
        vault_balance(&mut test, setup.vault),
        Amount::from(DEPOSIT - 150_000)
    );
    assert_eq!(
        xtr_balance(&mut test, heir),
        heir_before + Amount::from(150_000u64)
    );
    // The original owner gained nothing from the new holder withdrawal.
    assert_eq!(
        xtr_balance(&mut test, setup.owner),
        Amount::from(FAUCET_BALANCE - DEPOSIT)
    );
}

#[test]
fn transferred_badge_new_holder_can_set_beneficiary() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let (heir, heir_proof, heir_key) = transfer_badge(&mut test, &setup);
    let (new_beneficiary, _, _) = test.create_empty_account();

    test.execute_expect_success(
        test.transaction()
            .create_proof(heir, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "set_beneficiary", args![new_beneficiary])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&heir_key),
        vec![heir_proof.clone()],
    );
    let beneficiary: ComponentAddress =
        test.call_method(setup.vault, "beneficiary", args![], vec![]);
    assert_eq!(beneficiary, new_beneficiary);

    // And the redirected beneficiary is the one who gets paid on release.
    set_epoch(&mut test, INTERVAL);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&heir_key),
        vec![heir_proof],
    );
    assert_eq!(xtr_balance(&mut test, new_beneficiary), Amount::from(DEPOSIT));
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::ZERO);
}

#[test]
fn transferred_badge_new_holder_can_close() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let (heir, heir_proof, heir_key) = transfer_badge(&mut test, &setup);
    let heir_before = xtr_balance(&mut test, heir);

    test.execute_expect_success(
        test.transaction()
            .create_proof(heir, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "close", args![])
            .put_last_instruction_output_on_workspace("returned")
            .call_method(heir, "deposit", args![Workspace("returned")])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&heir_key),
        vec![heir_proof],
    );

    assert_eq!(vault_state(&mut test, setup.vault), "Closed");
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::ZERO);
    // The full balance went to the new holder, not the original owner.
    assert_eq!(
        xtr_balance(&mut test, heir),
        heir_before + Amount::from(DEPOSIT)
    );
    assert_eq!(
        xtr_balance(&mut test, setup.owner),
        Amount::from(FAUCET_BALANCE - DEPOSIT)
    );
}

// ---------------------------------------------------------------------------
// Deadline boundary behaviour
// ---------------------------------------------------------------------------

#[test]
fn check_in_exactly_at_the_deadline_epoch_succeeds_and_extends() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // Epoch 10 is exactly the deadline: the vault reads as lapsed, but a
    // check-in landing on the deadline epoch itself still revives it,
    // because release and check-in are both judged at the current epoch
    // and the check-in lands first here.
    set_epoch(&mut test, INTERVAL);
    let lapsed: bool = test.call_method(setup.vault, "is_lapsed", args![], vec![]);
    assert!(lapsed);

    owner_check_in(&mut test, &setup);
    let last_check_in: u64 = test.call_method(setup.vault, "last_check_in_epoch", args![], vec![]);
    assert_eq!(last_check_in, INTERVAL);
    assert_eq!(vault_deadline(&mut test, setup.vault), INTERVAL * 2);
    let lapsed: bool = test.call_method(setup.vault, "is_lapsed", args![], vec![]);
    assert!(!lapsed);
}

#[test]
fn check_in_at_the_deadline_blocks_release_at_the_old_deadline() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // Owner checks in on the deadline epoch itself.
    set_epoch(&mut test, INTERVAL);
    owner_check_in(&mut test, &setup);

    // A release at that same epoch, which would have succeeded against the
    // old deadline, is now refused because the deadline moved to epoch 20.
    let (_stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof.clone()],
    );
    assert_eq!(vault_state(&mut test, setup.vault), "Active");

    // One epoch before the new deadline release is still refused, and at
    // the new deadline it succeeds.
    set_epoch(&mut test, INTERVAL * 2 - 1);
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof.clone()],
    );
    set_epoch(&mut test, INTERVAL * 2);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::from(DEPOSIT));
}

#[test]
fn release_one_epoch_after_the_deadline_pays_the_beneficiary() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    set_epoch(&mut test, INTERVAL + 1);
    let lapsed: bool = test.call_method(setup.vault, "is_lapsed", args![], vec![]);
    assert!(lapsed);

    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::from(DEPOSIT));
    assert_eq!(vault_state(&mut test, setup.vault), "Released");
}

// ---------------------------------------------------------------------------
// Beneficiary change after the window has lapsed
// ---------------------------------------------------------------------------

#[test]
fn set_beneficiary_after_lapse_redirects_the_release() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let (new_beneficiary, _, _) = test.create_empty_account();

    // The window lapses with no check-in and no release yet.
    set_epoch(&mut test, INTERVAL + 5);
    let lapsed: bool = test.call_method(setup.vault, "is_lapsed", args![], vec![]);
    assert!(lapsed);

    // The template only requires the vault to still be Active for a
    // beneficiary change, so the owner can still redirect the payout after
    // the lapse, as long as nobody has released first.
    test.execute_expect_success(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "set_beneficiary", args![new_beneficiary])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    let beneficiary: ComponentAddress =
        test.call_method(setup.vault, "beneficiary", args![], vec![]);
    assert_eq!(beneficiary, new_beneficiary);
    // Changing the beneficiary is not a check-in: the vault stays lapsed.
    let lapsed: bool = test.call_method(setup.vault, "is_lapsed", args![], vec![]);
    assert!(lapsed);
    assert_eq!(vault_deadline(&mut test, setup.vault), INTERVAL);

    // The release now pays the new beneficiary, and the old one gets zero.
    let (_stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );
    assert_eq!(xtr_balance(&mut test, new_beneficiary), Amount::from(DEPOSIT));
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::ZERO);
}

// ---------------------------------------------------------------------------
// Release triggered by the beneficiary and by the owner
// ---------------------------------------------------------------------------

#[test]
fn release_triggered_by_the_beneficiary_pays_them() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // Point the vault at a funded account so the beneficiary can sign a
    // transaction of their own.
    let (beneficiary_acct, beneficiary_proof, beneficiary_key) = test.create_funded_account();
    test.execute_expect_success(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "set_beneficiary", args![beneficiary_acct])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    let beneficiary_before = xtr_balance(&mut test, beneficiary_acct);

    // After the lapse the beneficiary triggers the release themselves.
    set_epoch(&mut test, INTERVAL + 2);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&beneficiary_key),
        vec![beneficiary_proof],
    );

    assert_eq!(
        xtr_balance(&mut test, beneficiary_acct),
        beneficiary_before + Amount::from(DEPOSIT)
    );
    assert_eq!(vault_state(&mut test, setup.vault), "Released");
}

#[test]
fn release_triggered_by_the_owner_pays_the_beneficiary_not_the_owner() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let owner_before = xtr_balance(&mut test, setup.owner);

    // The owner triggers the release after the lapse. Release needs no
    // badge proof: it is public, and the funds can only go to the
    // beneficiary, never to the caller.
    set_epoch(&mut test, INTERVAL + 1);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::from(DEPOSIT));
    assert_eq!(xtr_balance(&mut test, setup.owner), owner_before);
    assert_eq!(vault_state(&mut test, setup.vault), "Released");
}

// ---------------------------------------------------------------------------
// One epoch windows and non fungible creation
// ---------------------------------------------------------------------------

#[test]
fn create_with_interval_of_one_epoch_lapses_at_the_next_epoch() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, 1);

    // The shortest legal window: created at epoch 0, the deadline is epoch 1.
    assert_eq!(vault_deadline(&mut test, setup.vault), 1);
    assert!(!vault_is_lapsed(&mut test, setup.vault));

    // One epoch later the vault has lapsed and can be released.
    set_epoch(&mut test, 1);
    assert!(vault_is_lapsed(&mut test, setup.vault));
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::from(DEPOSIT));
    assert_eq!(vault_state(&mut test, setup.vault), "Released");
}

#[test]
fn create_with_a_non_fungible_deposit_is_rejected() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let (beneficiary, _, _) = test.create_empty_account();
    let template = test.get_template_address(TEMPLATE_NAME);

    // The owner badge of the first vault is a non fungible token. Opening a
    // second vault around a bucket of it must be refused: a vault can only
    // ever hold an amount based resource, never a non fungible one.
    test.execute_expect_failure(
        test.transaction()
            .call_method(
                setup.owner,
                "withdraw_non_fungible",
                args![setup.badge_resource, tari_template_lib::types::NonFungibleId::from_u64(0)],
            )
            .put_last_instruction_output_on_workspace("nft")
            .call_function(template, "new", args![Workspace("nft"), beneficiary, INTERVAL])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
}

// ---------------------------------------------------------------------------
// Deposit resource rules and the heartbeat
// ---------------------------------------------------------------------------

#[test]
fn deposit_of_a_different_resource_is_rejected() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let other = create_vault(&mut test, DEPOSIT, INTERVAL);

    // The second vault's owner badge is a bucket of a completely different
    // resource from this vault's TARI. Depositing it here must be refused:
    // a vault only ever holds the resource it was created with.
    test.execute_expect_failure(
        test.transaction()
            .call_method(
                other.owner,
                "withdraw_non_fungible",
                args![other.badge_resource, tari_template_lib::types::NonFungibleId::from_u64(0)],
            )
            .put_last_instruction_output_on_workspace("foreign")
            .call_method(setup.vault, "deposit", args![Workspace("foreign")])
            .build_and_seal(&other.owner_key),
        vec![other.owner_proof.clone()],
    );

    assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(DEPOSIT));
    assert_eq!(vault_deadline(&mut test, setup.vault), INTERVAL);
    // The second vault is untouched.
    assert_eq!(vault_balance(&mut test, other.vault), Amount::from(DEPOSIT));
}

#[test]
fn deposit_does_not_extend_the_deadline() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // Most of the window passes, then a top up lands.
    set_epoch(&mut test, 6);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.owner, "withdraw", args![TARI_TOKEN, 50_000u64])
            .put_last_instruction_output_on_workspace("topup")
            .call_method(setup.vault, "deposit", args![Workspace("topup")])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    // The balance grew, but the heartbeat did not move: the deadline is
    // still the original epoch 10 and the last check in is still epoch 0.
    assert_eq!(
        vault_balance(&mut test, setup.vault),
        Amount::from(DEPOSIT + 50_000)
    );
    assert_eq!(vault_deadline(&mut test, setup.vault), INTERVAL);
    let last_check_in: u64 = test.call_method(setup.vault, "last_check_in_epoch", args![], vec![]);
    assert_eq!(last_check_in, 0);

    // So at the original deadline the vault releases and pays the topped
    // up balance, exactly as if the deposit had bought no time at all.
    set_epoch(&mut test, INTERVAL);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(
        xtr_balance(&mut test, setup.beneficiary),
        Amount::from(DEPOSIT + 50_000)
    );
}

#[test]
fn deposit_after_lapse_increases_what_the_beneficiary_receives() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // The window lapses with no check in and no release yet.
    set_epoch(&mut test, INTERVAL + 4);
    assert!(vault_is_lapsed(&mut test, setup.vault));

    // A stranger tops up the lapsed vault. Depositing only requires the
    // vault to still be Active, and it does not revive the heartbeat.
    let (stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_success(
        test.transaction()
            .call_method(stranger, "withdraw", args![TARI_TOKEN, 200_000u64])
            .put_last_instruction_output_on_workspace("topup")
            .call_method(setup.vault, "deposit", args![Workspace("topup")])
            .build_and_seal(&stranger_key),
        vec![stranger_proof.clone()],
    );
    assert_eq!(
        vault_balance(&mut test, setup.vault),
        Amount::from(DEPOSIT + 200_000)
    );
    assert_eq!(vault_deadline(&mut test, setup.vault), INTERVAL);
    assert!(vault_is_lapsed(&mut test, setup.vault));

    // The release pays the topped up balance in full.
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );
    assert_eq!(
        xtr_balance(&mut test, setup.beneficiary),
        Amount::from(DEPOSIT + 200_000)
    );
}

// ---------------------------------------------------------------------------
// Heartbeat endurance
// ---------------------------------------------------------------------------

#[test]
fn repeated_check_ins_keep_extending_the_deadline() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // The owner checks in three times, each time inside the running window,
    // and every check in moves the deadline out by a full interval.
    for epoch in [3u64, 9, 15] {
        set_epoch(&mut test, epoch);
        owner_check_in(&mut test, &setup);
        let last_check_in: u64 =
            test.call_method(setup.vault, "last_check_in_epoch", args![], vec![]);
        assert_eq!(last_check_in, epoch);
        assert_eq!(vault_deadline(&mut test, setup.vault), epoch + INTERVAL);
    }

    // One epoch before the third deadline the vault still cannot release.
    let (_stranger, stranger_proof, stranger_key) = test.create_funded_account();
    set_epoch(&mut test, 15 + INTERVAL - 1);
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof.clone()],
    );
    assert_eq!(vault_state(&mut test, setup.vault), "Active");

    // At the third deadline it releases and pays in full.
    set_epoch(&mut test, 15 + INTERVAL);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::from(DEPOSIT));
}

#[test]
fn owner_silent_for_five_intervals_revives_with_a_single_check_in() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // Five whole windows pass with no check in and no release.
    set_epoch(&mut test, INTERVAL * 5);
    assert!(vault_is_lapsed(&mut test, setup.vault));

    // One check in revives the vault completely, however long the silence.
    owner_check_in(&mut test, &setup);
    let last_check_in: u64 = test.call_method(setup.vault, "last_check_in_epoch", args![], vec![]);
    assert_eq!(last_check_in, INTERVAL * 5);
    assert_eq!(vault_deadline(&mut test, setup.vault), INTERVAL * 6);
    assert!(!vault_is_lapsed(&mut test, setup.vault));

    // Release is refused inside the new window and succeeds at its end.
    let (_stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof.clone()],
    );
    set_epoch(&mut test, INTERVAL * 6);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::from(DEPOSIT));
}

// ---------------------------------------------------------------------------
// Badge transfer: completing the old holder matrix
// ---------------------------------------------------------------------------

#[test]
fn transferred_badge_old_holder_cannot_check_in() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let (_heir, _heir_proof, _heir_key) = transfer_badge(&mut test, &setup);

    // The old holder cannot even complete the proof dance, exactly as with
    // withdraw, set beneficiary and close.
    test.execute_expect_failure(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "check_in", args![])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    let last_check_in: u64 = test.call_method(setup.vault, "last_check_in_epoch", args![], vec![]);
    assert_eq!(last_check_in, 0);
    assert_eq!(vault_deadline(&mut test, setup.vault), INTERVAL);
}

#[test]
fn badge_passed_on_twice_only_the_final_holder_can_act() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let (heir, heir_proof, heir_key) = transfer_badge(&mut test, &setup);

    // The first heir passes the badge on to a second heir.
    let (final_holder, final_proof, final_key) = test.create_funded_account();
    test.execute_expect_success(
        test.transaction()
            .call_method(
                heir,
                "withdraw_non_fungible",
                args![setup.badge_resource, tari_template_lib::types::NonFungibleId::from_u64(0)],
            )
            .put_last_instruction_output_on_workspace("badge")
            .call_method(final_holder, "deposit", args![Workspace("badge")])
            .build_and_seal(&heir_key),
        vec![heir_proof.clone()],
    );

    // Neither the original owner nor the middle holder can check in now.
    test.execute_expect_failure(
        test.transaction()
            .create_proof(setup.owner, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "check_in", args![])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    test.execute_expect_failure(
        test.transaction()
            .create_proof(heir, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "check_in", args![])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&heir_key),
        vec![heir_proof.clone()],
    );
    let last_check_in: u64 = test.call_method(setup.vault, "last_check_in_epoch", args![], vec![]);
    assert_eq!(last_check_in, 0);

    // The final holder can.
    set_epoch(&mut test, 4);
    test.execute_expect_success(
        test.transaction()
            .create_proof(final_holder, setup.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(setup.vault, "check_in", args![])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&final_key),
        vec![final_proof],
    );
    assert_eq!(vault_deadline(&mut test, setup.vault), 4 + INTERVAL);
}

// ---------------------------------------------------------------------------
// Withdrawals in parts and in the lapsed window
// ---------------------------------------------------------------------------

#[test]
fn withdraw_in_several_parts_accounts_to_the_unit() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // Four withdrawals that sum exactly to the deposit, the last of them a
    // single unit. The vault balance is checked after every part.
    let parts = [100_000u64, 150_000, 249_999, 1];
    let mut remaining = DEPOSIT;
    for part in parts {
        owner_withdraw(&mut test, &setup, part);
        remaining -= part;
        assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(remaining));
    }

    // Every unit came home: the owner is whole again, the vault is empty
    // but still Active, and none of the withdrawals moved the deadline.
    assert_eq!(
        xtr_balance(&mut test, setup.owner),
        Amount::from(FAUCET_BALANCE)
    );
    assert_eq!(vault_state(&mut test, setup.vault), "Active");
    assert_eq!(vault_deadline(&mut test, setup.vault), INTERVAL);
}

#[test]
fn owner_can_still_withdraw_after_the_lapse_until_release() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // The window lapses. Until somebody actually releases, the vault is
    // still Active, so the owner badge still works in the lapsed window.
    set_epoch(&mut test, INTERVAL + 3);
    assert!(vault_is_lapsed(&mut test, setup.vault));

    owner_withdraw(&mut test, &setup, 200_000);
    assert_eq!(
        vault_balance(&mut test, setup.vault),
        Amount::from(DEPOSIT - 200_000)
    );
    assert_eq!(vault_state(&mut test, setup.vault), "Active");
    assert!(vault_is_lapsed(&mut test, setup.vault));

    // The release then pays out only what is left.
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(
        xtr_balance(&mut test, setup.beneficiary),
        Amount::from(DEPOSIT - 200_000)
    );
}

// ---------------------------------------------------------------------------
// Beneficiary changes and exits around the lapse
// ---------------------------------------------------------------------------

#[test]
fn changing_the_beneficiary_twice_pays_the_final_beneficiary() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let (second, _, _) = test.create_empty_account();
    let (third, _, _) = test.create_empty_account();

    // Two changes in a row: only the last one counts.
    owner_set_beneficiary(&mut test, &setup, second);
    owner_set_beneficiary(&mut test, &setup, third);
    let beneficiary: ComponentAddress =
        test.call_method(setup.vault, "beneficiary", args![], vec![]);
    assert_eq!(beneficiary, third);

    set_epoch(&mut test, INTERVAL);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );

    assert_eq!(xtr_balance(&mut test, third), Amount::from(DEPOSIT));
    assert_eq!(xtr_balance(&mut test, second), Amount::ZERO);
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::ZERO);
}

#[test]
fn owner_can_close_a_lapsed_vault_before_anyone_releases_it() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);
    let owner_before = xtr_balance(&mut test, setup.owner);

    // The window lapses, but closing only requires the vault to still be
    // Active, so the owner can still take everything back first.
    set_epoch(&mut test, INTERVAL + 2);
    assert!(vault_is_lapsed(&mut test, setup.vault));

    owner_close(&mut test, &setup);
    assert_eq!(vault_state(&mut test, setup.vault), "Closed");
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::ZERO);
    assert_eq!(
        xtr_balance(&mut test, setup.owner),
        owner_before + Amount::from(DEPOSIT)
    );

    // And now release is impossible: the beneficiary gets nothing.
    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::ZERO);
}

#[test]
fn release_long_after_the_deadline_still_pays_in_full() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    // Ten whole windows pass before anyone touches the vault. There is no
    // expiry on the release right itself.
    set_epoch(&mut test, INTERVAL * 10);
    assert!(vault_is_lapsed(&mut test, setup.vault));

    let (caller, caller_proof, caller_key) = test.create_funded_account();
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&caller_key),
        vec![caller_proof],
    );
    assert_eq!(xtr_balance(&mut test, setup.beneficiary), Amount::from(DEPOSIT));
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::ZERO);
    assert_eq!(vault_state(&mut test, setup.vault), "Released");
}

// ---------------------------------------------------------------------------
// State and lapse views
// ---------------------------------------------------------------------------

#[test]
fn state_reports_active_then_released_and_active_then_closed() {
    let mut test = TemplateTest::my_crate();
    let released = create_vault(&mut test, DEPOSIT, INTERVAL);
    let closed = create_vault(&mut test, DEPOSIT, INTERVAL);

    assert_eq!(vault_state(&mut test, released.vault), "Active");
    assert_eq!(vault_state(&mut test, closed.vault), "Active");

    // The first vault runs its full course to release.
    set_epoch(&mut test, INTERVAL);
    test.execute_expect_success(
        test.transaction()
            .call_method(released.vault, "release", args![])
            .build_and_seal(&released.owner_key),
        vec![released.owner_proof.clone()],
    );
    assert_eq!(vault_state(&mut test, released.vault), "Released");
    // The second vault is unaffected and still reports Active.
    assert_eq!(vault_state(&mut test, closed.vault), "Active");

    // The second vault is closed by its owner instead.
    owner_close(&mut test, &closed);
    assert_eq!(vault_state(&mut test, closed.vault), "Closed");
}

#[test]
fn is_lapsed_turns_true_exactly_at_the_deadline_and_false_after_revival() {
    let mut test = TemplateTest::my_crate();
    let setup = create_vault(&mut test, DEPOSIT, INTERVAL);

    assert!(!vault_is_lapsed(&mut test, setup.vault));
    set_epoch(&mut test, INTERVAL - 1);
    assert!(!vault_is_lapsed(&mut test, setup.vault));
    // The flip happens exactly at the deadline epoch, not one after it.
    set_epoch(&mut test, INTERVAL);
    assert!(vault_is_lapsed(&mut test, setup.vault));

    // A check in at the deadline revives the vault and the view flips back.
    owner_check_in(&mut test, &setup);
    assert!(!vault_is_lapsed(&mut test, setup.vault));
    set_epoch(&mut test, INTERVAL * 2 - 1);
    assert!(!vault_is_lapsed(&mut test, setup.vault));
    set_epoch(&mut test, INTERVAL * 2);
    assert!(vault_is_lapsed(&mut test, setup.vault));
}

// ---------------------------------------------------------------------------
// Two vaults side by side
// ---------------------------------------------------------------------------

#[test]
fn two_vaults_run_independently_side_by_side() {
    let mut test = TemplateTest::my_crate();
    let first = create_vault(&mut test, DEPOSIT, INTERVAL);
    let second = create_vault(&mut test, 300_000, INTERVAL * 3);

    // At the first vault's deadline it has lapsed; the second has not.
    set_epoch(&mut test, INTERVAL);
    assert!(vault_is_lapsed(&mut test, first.vault));
    assert!(!vault_is_lapsed(&mut test, second.vault));

    // Releasing the first pays its beneficiary and leaves the second vault
    // completely untouched.
    test.execute_expect_success(
        test.transaction()
            .call_method(first.vault, "release", args![])
            .build_and_seal(&first.owner_key),
        vec![first.owner_proof.clone()],
    );
    assert_eq!(xtr_balance(&mut test, first.beneficiary), Amount::from(DEPOSIT));
    assert_eq!(vault_state(&mut test, first.vault), "Released");
    assert_eq!(vault_balance(&mut test, second.vault), Amount::from(300_000u64));
    assert_eq!(vault_state(&mut test, second.vault), "Active");
    assert_eq!(vault_deadline(&mut test, second.vault), INTERVAL * 3);
    assert_eq!(xtr_balance(&mut test, second.beneficiary), Amount::ZERO);

    // The second vault's own heartbeat still works on its own schedule.
    set_epoch(&mut test, INTERVAL + 2);
    owner_check_in(&mut test, &second);
    assert_eq!(
        vault_deadline(&mut test, second.vault),
        INTERVAL + 2 + INTERVAL * 3
    );
    assert_eq!(vault_state(&mut test, first.vault), "Released");
}

#[test]
fn one_vaults_badge_does_not_authorize_another_vault() {
    let mut test = TemplateTest::my_crate();
    let first = create_vault(&mut test, DEPOSIT, INTERVAL);
    let second = create_vault(&mut test, DEPOSIT, INTERVAL);

    // Each vault mints its own badge resource at creation.
    assert_ne!(first.badge_resource, second.badge_resource);

    // A proof of the first vault's badge does not satisfy the second
    // vault's owner rule, neither for check in nor for withdraw.
    test.execute_expect_failure(
        test.transaction()
            .create_proof(first.owner, first.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(second.vault, "check_in", args![])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&first.owner_key),
        vec![first.owner_proof.clone()],
    );
    test.execute_expect_failure(
        test.transaction()
            .create_proof(first.owner, first.badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(second.vault, "withdraw", args![100u64])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&first.owner_key),
        vec![first.owner_proof.clone()],
    );
    let last_check_in: u64 = test.call_method(second.vault, "last_check_in_epoch", args![], vec![]);
    assert_eq!(last_check_in, 0);
    assert_eq!(vault_balance(&mut test, second.vault), Amount::from(DEPOSIT));

    // The second vault's own badge works fine, so the refusals above were
    // about the badge, not about the vault.
    set_epoch(&mut test, 5);
    owner_check_in(&mut test, &second);
    assert_eq!(vault_deadline(&mut test, second.vault), 5 + INTERVAL);
}

// ---------------------------------------------------------------------------
// Amount extremes
// ---------------------------------------------------------------------------

#[test]
fn very_large_amounts_are_tracked_and_paid_exactly() {
    let mut test = TemplateTest::my_crate();
    // Nearly the entire faucet balance of the harness. Amounts in this
    // engine are integers of micro Tari, so 999,999,999 units is 999.999999
    // Tari, and every unit of it must be accounted for.
    let setup = create_vault(&mut test, FAUCET_BALANCE - 1, INTERVAL);
    assert_eq!(
        vault_balance(&mut test, setup.vault),
        Amount::from(FAUCET_BALANCE - 1)
    );
    assert_eq!(xtr_balance(&mut test, setup.owner), Amount::from(1u64));

    // A single micro Tari top up from a stranger brings the vault to a
    // round one thousand Tari in total.
    let (stranger, stranger_proof, stranger_key) = test.create_funded_account();
    test.execute_expect_success(
        test.transaction()
            .call_method(stranger, "withdraw", args![TARI_TOKEN, 1u64])
            .put_last_instruction_output_on_workspace("topup")
            .call_method(setup.vault, "deposit", args![Workspace("topup")])
            .build_and_seal(&stranger_key),
        vec![stranger_proof],
    );
    assert_eq!(
        vault_balance(&mut test, setup.vault),
        Amount::from(FAUCET_BALANCE)
    );

    set_epoch(&mut test, INTERVAL);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(
        xtr_balance(&mut test, setup.beneficiary),
        Amount::from(FAUCET_BALANCE)
    );
}

#[test]
fn fractional_amounts_down_to_one_micro_unit_are_paid_exactly() {
    let mut test = TemplateTest::my_crate();
    // Amounts are integers of micro Tari: one unit is 0.000001 Tari, the
    // smallest fraction the chain can represent. This vault is built
    // entirely from fractions of a single Tari.
    let setup = create_vault(&mut test, 1, INTERVAL);
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(1u64));

    // Top ups of 123,456 and 654,321 micro Tari (0.123456 and 0.654321
    // Tari) bring the balance to 777,778 micro Tari.
    for amount in [123_456u64, 654_321] {
        test.execute_expect_success(
            test.transaction()
                .call_method(setup.owner, "withdraw", args![TARI_TOKEN, amount])
                .put_last_instruction_output_on_workspace("topup")
                .call_method(setup.vault, "deposit", args![Workspace("topup")])
                .build_and_seal(&setup.owner_key),
            vec![setup.owner_proof.clone()],
        );
    }
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(777_778u64));

    // Withdrawing a single micro unit leaves exactly 777,777.
    owner_withdraw(&mut test, &setup, 1);
    assert_eq!(vault_balance(&mut test, setup.vault), Amount::from(777_777u64));

    // And the release pays the beneficiary that exact fractional balance,
    // not a rounded one.
    set_epoch(&mut test, INTERVAL);
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.vault, "release", args![])
            .build_and_seal(&setup.owner_key),
        vec![setup.owner_proof.clone()],
    );
    assert_eq!(
        xtr_balance(&mut test, setup.beneficiary),
        Amount::from(777_777u64)
    );
}
