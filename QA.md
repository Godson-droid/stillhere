# QA gate — StillHere

Checked 2026-11-16 against the verified contest rules in `../RESEARCH.md`
(Tari November Build Contest, theme "MVP", deadline 2026-12-01 00:00 UTC).

## Contest rules vs deliverables

| Rule (from RESEARCH.md) | Status |
| --- | --- |
| New code only, written after the 2026-11-01 theme announcement | PASS — repo created and written 2026-11-16; git history starts today |
| OSI approved open source licence, MIT recommended | PASS — MIT `LICENSE`, Copyright 2026 Godson |
| Rust / Ootle MVP build | PASS — `src/lib.rs`, one template, `tari_template_lib` 0.40.1 |
| Working MVP app link for judges | READY — `sim/index.html`, a self contained simulation of the template logic (see below); hosting is a later step with Godson |
| Public code link | PENDING GODSON — repo is local only in this phase; publishing to GitHub is a separate approved step |
| Submission post in community.tari.com topic 520 (code link, app link, description, Tari payment address, announcement link) | PENDING GODSON — not part of this build phase; the payment address must come from his own Tari wallet |
| Announcement on Bluesky, Reddit, Medium or YouTube (X does not count) | PENDING GODSON — his existing YouTube channel satisfies this with no new account |
| AI / agent development | Allowed explicitly by the Council; nothing to do |
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

- The template is tested in process only. It has not been deployed to a
  live Tari network or wallet in this phase.
- The optional mainnet "First to Launch" bonus was not attempted: it
  requires burning real XTM, which needs Godson's explicit approval and
  funds.
- Submission steps still to come with Godson: publish the repo publicly,
  host the simulation, create/provide his Tari payment address, record or
  post the announcement (YouTube), and post in topic 520 before
  2026-12-01 00:00 UTC.

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
