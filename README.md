# StillHere

A dead-man's-switch vault on **Tari Ootle**, built for the Tari November Build Contest (theme: MVP).

You deposit TARI, name a beneficiary, and set a heartbeat window. As long as you check in at least once
per window, the vault stays yours. If you ever stop — the window lapses — **anyone** can trigger the
release, and the vault pays its full balance directly to your beneficiary. No lawyer, no custodian,
no server watching a clock: the chain itself is the heartbeat monitor.

Inheritance and "if something happens to me" transfers are the obvious use. The same primitive covers
dormant-account recovery, dead-hand escrow, and team treasuries that must not die with one keyholder.

## How it works

| Method | Who can call it | What it does |
|---|---|---|
| `new(deposit, beneficiary, interval_epochs)` | Anyone (the creator) | Opens a vault with an initial deposit, mints the single **owner badge** to the creator, sets the first deadline to `current_epoch + interval`. |
| `check_in()` | Owner badge holder | The heartbeat. Resets the deadline to `current_epoch + interval`. A late check-in before anyone releases revives the vault. |
| `deposit(bucket)` | Anyone | Tops up the vault. Does **not** reset the heartbeat. |
| `withdraw(amount)` | Owner badge holder | Takes funds back out. Does **not** reset the heartbeat. |
| `set_beneficiary(addr)` | Owner badge holder | Redirects a future release to a new beneficiary. |
| `close()` | Owner badge holder | Terminal. Returns the full balance to the owner; the vault can never be released afterwards. |
| `release()` | **Anyone**, only at/after the deadline | Terminal. Pays the **entire balance directly into the beneficiary's account** — the vault itself makes the deposit, so the caller can never divert the funds. |
| `status()`, `balance()`, `deadline_epoch()`, … | Anyone | Read-only views. |

Ownership is a **badge**, not an address: exactly one owner badge (resource symbol `SHOWN`) is minted at
creation, its mint/burn rules are locked to deny-all forever, and the component's access rules require a
proof of that badge for every owner-only method. Hand the badge to someone else and they become the
operator — ownership transfer is just an NFT transfer. The template emits an event for every state
change (`still_here.created`, `.deposit`, `.check_in`, `.withdraw`, `.beneficiary_changed`,
`.closed`, `.released`).

Safety properties, all enforced on chain and covered by tests:

- Release is impossible one epoch before the deadline and possible exactly at it.
- Release pays the beneficiary the exact accumulated balance, to the unit, and pays the caller nothing.
- A second release is refused; a closed vault can never be released; terminal vaults refuse deposits,
  check-ins, and withdrawals.
- Zero-value deposits, zero withdrawals, over-balance withdrawals, and a zero check-in interval are
  all refused with explicit messages.
- The component has no owner (`OwnerRule::None`). In the Ootle engine a component owner can call any
  method and rewrite the access rules, so the vault is created ownerless on purpose: the badge rules
  are the only authority, they can never be changed after creation, and transferring the badge fully
  transfers control (the previous holder's access ends with the transfer, which the tests verify).

## Run the tests

The test suite runs the real Ootle engine in-process (no node, no wallet, no funds) using
`tari_template_test_tooling`. Time is controlled by injecting the current-epoch virtual substate,
which is what `Consensus::current_epoch()` reads inside the template.

```sh
cargo test
```

35 tests cover the full lifecycle: creation terms and guards, public top ups, check in extension,
late check in revival, release before, exactly at and after the deadline, exact beneficiary payment
with a third party caller, double release refusal, non owner refusal for every owner only method,
badge transfer ownership including the old holder losing every owner only action and the new holder
gaining them, a check in exactly at the deadline epoch, beneficiary change after the window has
lapsed, release triggered by the beneficiary and by the owner, partial withdrawal, beneficiary
redirection, close semantics, and terminal state refusals.

## Try the simulation

`sim/index.html` is a walletless browser simulation of the template: create a vault, deposit, check
in, advance epochs, let it lapse, release. Every rule and every refusal message mirrors `src/lib.rs`.
Open it from disk or serve the folder with any static host:

```sh
python3 -m http.server --directory sim 8000
# then visit http://localhost:8000
```

How the simulation maps to the template:

| Simulation | Template |
|---|---|
| Epoch counter + advance buttons | `Consensus::current_epoch()` (epochs pass on their own on chain) |
| Owner / Beneficiary / Stranger actor switch | Transaction signer + proofs; owner-only methods require an authorized proof of the owner badge |
| Create form | `new(deposit, beneficiary, interval_epochs)` |
| Deposit / Withdraw buttons | `deposit(bucket)` / `withdraw(amount)` |
| Check in / Close / Release buttons | `check_in()` / `close()` / `release()` |
| Event log | The `still_here.*` events the template emits |
| Refusal toasts | The template's assertion and access-rule rejection messages, verbatim |

One deliberate simplification: the simulation uses plain numbers for balances, while the template
moves real buckets through a vault. Amount semantics (exact payment, no rounding) are identical.

## Build the WASM template

```sh
cargo build-wasm   # alias for: cargo build --target=wasm32-unknown-unknown
```

## Deployment

The template is published on the Esmeralda testnet.

Template address: template_e7164ec19c5c7ea0e84c8d0cce06c874565cd44e97109088bf3eedfdd5a1664d

Publish transaction: 217ce22b3f7ee37a4df0efb5c728d255dfe546dd30deb93e45b9163cdf766e4b

The publish was confirmed in epoch 13234 on 19 November 2026 and can be checked on the public
Esmeralda indexer. The published binary is the exact build of the code in this repository,
147,558 bytes of WASM. Anyone can now create a StillHere vault on Esmeralda by calling the
template constructor.

## Layout

- `src/lib.rs` — the StillHere template (single file, heavily documented).
- `tests/test.rs` — the engine test suite.
- `sim/` — the walletless browser simulation.
- `QA.md` — the pre-submission quality gate: every check run against this repo and its result.

## Licence

MIT. See `LICENSE`.
