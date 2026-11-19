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
