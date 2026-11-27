# QA gate — StillHere

Checked 2026-11-16 against the verified contest rules in `../RESEARCH.md`
(Tari November Build Contest, theme "MVP", deadline 2026-12-01 00:00 UTC).
Last updated 2026-11-27 (see the closing update at the end of this file).

## Contest rules vs deliverables

| Rule (from RESEARCH.md) | Status |
| --- | --- |
| New code only, written after the 2026-11-01 theme announcement | PASS — repo created and written 2026-11-16; git history starts today |
| OSI approved open source licence, MIT recommended | PASS — MIT `LICENSE`, Copyright 2026 Godson |
| Rust / Ootle MVP build | PASS — `src/lib.rs`, one template, `tari_template_lib` 0.40.1 |
| Working MVP app link for judges | PASS — simulation hosted and live at https://godson-droid.github.io/stillhere/sim/ |
| Public code link | PASS — published at https://github.com/Godson-droid/stillhere (repo created 2026-11-19) |
| Submission post in community.tari.com topic 520 (code link, app link, description, Tari payment address, announcement link) | PASS — submitted as post 30 in topic 520, rewritten in full on 2026-11-21 after Fox granted edit access in post 37, with code link, app link, description, Tari payment address and announcement link all in the post |
| Announcement on Bluesky, Reddit, Medium or YouTube (X does not count) | PASS — YouTube announcement at https://www.youtube.com/watch?v=FG9eTxrUMZw |
| Judging: theme adherence (MVP), innovation, code quality | Addressed by design: one focused product shaped template, an unoccupied category in the field (no dead man's switch among the 21 entries), full test coverage, documented safety properties |

## Build and test evidence

- Toolchain: rustc/cargo 1.100.0, `wasm32-unknown-unknown` target, kept under
  `~/workspace/tari-november-build/toolchain/` (sandbox has no system Rust).
- Resolved dependencies: `tari_template_lib` 0.40.1,
  `tari_template_test_tooling` 0.53.5, `tari_engine` 0.53.5.
- Full suite re run from clean (`cargo clean` then `cargo test`):
  **23 passed, 0 failed**, no warnings from this crate's code.
- `cargo build-wasm` (the template's own WASM build): finishes clean.
- Coverage: creation terms and guards (zero interval, zero deposit), public
  top ups, check in extension, late check in revival of a lapsed vault,
  release refused one epoch before the deadline and allowed exactly at it,
  exact beneficiary payment with a third party caller receiving nothing,
  release of the full accumulated balance, double release refusal, release of
  a drained vault, non owner refusal for check in, withdraw, set beneficiary
  and close, badge transfer changing the operator (including the previous
  holder losing access), partial withdraw leaving the deadline unchanged,
  over balance and zero withdraw refusal, beneficiary redirection, close
  semantics, terminal state refusals.

## Failures found during the build and fixed

1. `to_string` unavailable in the `no_std` template (ToString trait not in
   scope). Fixed with explicit `alloc` imports.
2. Event payloads: `Metadata::insert` takes values by reference and encodes
   them typed. Fixed by inserting typed values by reference instead of
   string conversions.
3. The template macro rejects a fieldless enum defined inside the template
   module ("a template must have associated functions and/or methods").
   Vault lifecycle state is therefore stored as a `u8` with named constants
   (`STATE_ACTIVE`, `STATE_RELEASED`, `STATE_CLOSED`).
4. A fungible only guard rejected TARI itself: in this engine version XTR is
   a Stealth resource. The guard now accepts Fungible and Stealth resources
   and refuses NonFungible and Confidential ones, which the amount based
   vault logic cannot handle.
5. Test harness return indexing: workspace put instructions count as
   instructions, so the constructor output sits at index 2. Documented in
   the test helper.
6. SECURITY, the important one: in this engine a component owner can call
   any method and rewrite the access rules, and the default component owner
   is the creating signer. That silently left the creator in permanent
   control and broke badge transfer semantics (found by the badge transfer
   test, confirmed with a three way probe: the badge moved, the new holder
   could check in, the old holder could no longer create a proof, yet the
   old holder's proof free call still passed). Fixed by creating the
   component with `OwnerRule::None`: no owner exists, the badge method
   rules are the only authority, and they can never be changed after
   creation. The badge transfer test now verifies the previous holder loses
   access.
7. Cosmetic unused variable warnings in tests: fixed; the suite compiles
   warning free. (One future incompatibility warning remains from the
   upstream `proc-macro-error2` dependency; not this crate's code.)

## Hygiene checks

- Secrets scan over the repo: no keys, tokens, passwords or seed material.
  The only matches for "key" are the test harness's ephemeral
  `RistrettoSecretKey` type used for throwaway in process accounts.
- Placeholder scan: no TODO, FIXME or placeholder logic in `src/`, `tests/`
  or `sim/`.
- Repo contents: `.cargo/config.toml`, `.gitignore` (target ignored),
  `Cargo.toml`, `Cargo.lock`, `LICENSE`, `README.md`, `QA.md`,
  `src/lib.rs`, `tests/test.rs`, `sim/index.html`. Nothing else.
- README: explains the concept, the method table, the safety properties,
  how to run the tests, and how the simulation maps to the template.

## Simulation checks (`sim/index.html`)

- Single self contained HTML file, no build step, no external assets; runs
  from `file://` or any static host.
- Clearly labelled as a simulation of the template logic, not a wallet and
  not connected to any network.
- JavaScript syntax checked; the state machine was exercised headlessly
  (create, top up, check in, lapse, release, plus the refusal paths) and
  matched the template's behaviour and refusal messages.
- Design: dark fintech surface in the project's house style (near black
  background, one mint accent, glass KPI cards, hairline borders, event log
  table), original HTML/CSS only, no kit assets.

## Known limitations and open items

- The template is deployed and verified on both networks: Esmeralda on
  19 November 2026 and mainnet on 21 November 2026 (template
  template_207a50d20c6bac05cac441fa3560377e92ed493217fbac3147f72405c27b38bb,
  publish transaction
  7da7290cd8bb0f116c7ec53c70bd12d43e1a2566efe240375f26e295f343991d,
  epoch 39124). The full lifecycle has also been run live on mainnet; see
  the 27 November 2026 update at the end of this file.
- The mainnet publish required burning 3 XTM on layer 1 and claiming it
  on layer 2, approved by Godson on 2026-11-21.
- All submission steps are complete: the repo is public, the simulation
  is hosted, the Tari payment address is in the submission post, the
  YouTube announcement is posted, and the submission itself is post 30
  in topic 520. No open items remain in this section.

## Rename note — 2026-11-16

The project was renamed from **Heartbeat Vault** to **StillHere** on
2026-11-16. The rename covers the Cargo package name (`stillhere`), the
template module and struct (`still_here` / `StillHere`), the template name
the tests resolve, the emitted event names (`still_here.*`), the owner
badge symbol (`SHOWN`, metadata name "StillHere Owner"), the README, this
QA file, the simulation page branding, and the repository directory
itself (`stillhere`). The word "heartbeat" remains only as a generic
description of the check-in mechanism. Post-rename verification from the
repo root: `cargo test` — **23 passed, 0 failed**, identical to the
pre-rename count, and `sim/index.html` re-checked as a single
self-contained file with no external references.

## Update, 19 November 2026

The engine suite now has 35 tests and the full suite passes, 35 passed and
0 failed, run again on 19 November 2026 against the final code. The twelve
added tests cover the badge transfer cases (the old holder refused on
withdraw, set beneficiary and close, and the new holder able to do each),
the exact deadline boundary (a check in exactly at the deadline epoch
succeeds and extends, a check in at the deadline blocks release at the old
deadline, and release one epoch after the deadline pays the beneficiary),
a beneficiary change after the window has lapsed redirecting the release,
and release triggered by the beneficiary and by the owner, with payment
going to the beneficiary in both cases.

The template was published to the Esmeralda testnet on 19 November 2026.
Template address template_e7164ec19c5c7ea0e84c8d0cce06c874565cd44e97109088bf3eedfdd5a1664d,
publish transaction 217ce22b3f7ee37a4df0efb5c728d255dfe546dd30deb93e45b9163cdf766e4b,
confirmed in epoch 13234 and verified on the public Esmeralda indexer. The
published binary is 147,558 bytes and is the exact build of the code in
this repository. The known limitation above about the template not being
deployed to a live network no longer applies to Esmeralda. Mainnet remains
undeployed and the First to Launch bonus remains unattempted.

## Update, 20 November 2026

The engine suite now has 55 tests and the full suite passes, 55 passed and
0 failed, run on 20 November 2026 against the code in this repository. The
twenty added tests cover creation with a one epoch interval, refusal of a
non fungible deposit and of a deposit in the wrong resource, proof that a
deposit does not extend the deadline, a top up after the lapse increasing
what the beneficiary receives, repeated check ins, revival after five
silent intervals, the transferred badge old holder refused on check in, a
badge passed on twice, several partial withdrawals accounting to the unit,
withdrawal and closing inside the lapsed window before release, a second
beneficiary change, release long after the deadline, the state and
is_lapsed views at their exact boundaries, two vaults running side by side
with no cross talk, one vault badge refused on another vault, and very
large and single micro unit amounts tracked and paid exactly. No template
code changed in this update, tests only.

## Update, 27 November 2026

This closing update overtakes two statements in the dated updates above:
the 19 November update says mainnet remains undeployed and the
20 November update was written before the publish. The template was in
fact published to mainnet on 21 November 2026 (template
template_207a50d20c6bac05cac441fa3560377e92ed493217fbac3147f72405c27b38bb,
publish transaction
7da7290cd8bb0f116c7ec53c70bd12d43e1a2566efe240375f26e295f343991d,
committed in epoch 39124, verified on the mainnet indexer), and every
submission step listed as still to come in the original QA pass is
complete, as the rules table at the top of this file now records.

On 27 November 2026 the full lifecycle was also run end to end on
mainnet against that published template, funded from the publish change:

- Split and account creation, transaction
  04036af18381211c327a92dc4070c5b9316be4f77decb8ff9b8172aad47b6ab8
  (epoch 39552). The creator account and a separate beneficiary account
  were created in this transaction.
- Vault A created, transaction
  d12297f86edee0c76e76462353c4ac6a8a455a77bb2ee5e1050542cd4d5f42aa:
  component
  component_8c081ccb95d232efb59d02b5ab6f71491a0356adcc5a1299d298e65b4bf36866,
  deposit 400,000 microT, beneficiary the separate beneficiary account,
  check in interval 100 epochs, deadline epoch 39652.
- Owner check in on vault A, transaction
  493b83f282a2c8a17b47d3fd603e3b0db0de3369b6a8565ea3ec4f9600a0ea6e
  (epoch 39552), with the still_here.check_in event emitted.
- Vault B created, transaction
  5e4fd5dd147a1f6488a74969e23aaa2a40b380a5b71f15393939ddf2f9fa4067:
  component
  component_963cc172a05b5f41ebbafea9a78ae364c163f1ba6e841a600a243f63ba4a77f1,
  deposit 400,000 microT, check in interval 1 epoch, deadline epoch
  39553. No check in was made.
- Release of vault B after the lapse, transaction
  a0b2592662db6668d849b387093fc614b274646558e42ca0457cfd1ace2dc840
  (epoch 39553), called by the beneficiary account, not the vault
  creator. The beneficiary balance moved from 90,000 microT to
  486,326 microT: the full 400,000 microT vault balance received, minus
  the 3,674 microT release fee, exact to the unit. Vault B now reads
  state Released on the mainnet indexer and vault A reads Active.

Every transaction above was verified on the mainnet indexer at
indexer-a.ootle.tari.com after it committed.
