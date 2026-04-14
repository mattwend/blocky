# Blocky

Blocky is a compact Rust blockchain runtime for experimentation. It favors readable code and explicit data flow over production-oriented complexity, and follows a lightweight RFC-driven process to evolve architectural decisions over time.

It explores how to structure a minimal, self-contained execution environment where blockchain state, transaction processing, and smart contract execution remain easy to reason about and evolve. It is intentionally scoped to make system boundaries, execution flow, and state transitions explicit.

## Overview

At a high level, Blocky combines four core subsystems:

- a minimal proof-of-work blockchain
- an in-memory world state for balances, contracts, and storage
- a Wasm smart-contract runtime with gas metering
- a terminal REPL for experimenting with the system interactively

Blocky is designed to be inspected, executed, and extended. If you want a compact codebase where the full execution path—from transaction to state transition—is easy to follow, this repository provides a clear reference.

Example contracts live under [`examples/contracts/`](examples/contracts/) so you can build small Wasm programs and exercise them through the REPL.

## Status

Blocky is ready for its `v1.0.0` open-source release.

Blocky currently covers the following milestones:

- RFC-001: basic blockchain, proof-of-work mining, validation, demo flow, and REPL
- RFC-002: world state, payload-based transactions, Wasm contracts, receipts, gas metering, and a small contract SDK

Blocky uses a lightweight RFC process for substantial design changes; see [`docs/rfc-process.md`](docs/rfc-process.md).

Design notes and implementation history live in:

- [`CHANGELOG.md`](CHANGELOG.md)
- [`docs/rfc-001-initial-design.md`](docs/rfc-001-initial-design.md)
- [`docs/rfc-002-smart-contracts.md`](docs/rfc-002-smart-contracts.md)

## Architecture overview

Workspace packages:

- `blocky` — the main package containing the library crate, shared application logic, and executable targets
- `blocky-sdk` — contract-side helpers for decoding call input, interacting with host functions, and reading/writing typed storage

Runtime flow:

```text
Transaction
  │
  ▼
Blockchain::add_transaction
  │
  ▼
pending_transactions
  │
  ▼
Blockchain::mine_pending
  │
  ├─ builds a Block and mines proof-of-work
  ├─ applies each transaction to WorldState
  ├─ executes Wasm calls through VmEngine when needed
  └─ records Receipt entries for each transaction
  ▼
confirmed chain + updated world state
```

Module map inside `blocky`:

```text
transaction  -> addresses, nonces, payloads
block        -> block structure, hashing, mining helpers
state        -> account balances, contract code, contract storage
chain        -> pending tx queue, mining, receipts, validation
call_abi     -> structured CallEnvelope shared with blocky-sdk
vm           -> Wasmtime engine, host functions, gas accounting
app/demo     -> printable walkthrough demo
app/repl     -> interactive terminal UI
```

## Prerequisites

- Rust toolchain with `cargo`
- `wasm32-unknown-unknown` target for building example contracts and SDK e2e contracts

Install the Wasm target with:

```bash
rustup target add wasm32-unknown-unknown
```

## Getting started

Run the full test suite:

```bash
cargo test --workspace
```

Run the demo binary:

```bash
cargo run --bin demo
```

Run the interactive REPL:

```bash
cargo run --bin repl
```

## Demo and REPL

The `demo` binary creates a tiny chain, queues a few transfers, mines a block, prints the chain, and then tampers with one transaction to demonstrate validation failure.

The `repl` binary opens an interactive terminal UI powered by `ratatui` and `crossterm`.

REPL controls:

- `Enter` executes the current command
- `Up` / `Down` scroll output when the input is empty, or navigate command history when it is not
- `PageUp` / `PageDown` scroll faster
- `Esc` exits

The UI shows:

- chain status
- pending transactions
- recent output
- command history
- a one-line help panel

Think of it as a text dashboard rather than a plain prompt.

Common commands:

- `add <sender> <receiver> <amount>`
- `deploy <sender> <path>`
- `call <sender> <addr> <method> [args|--hex <hex>|--json <json>]`
- `balance <addr>`
- `mine`
- `print`
- `validate`
- `help`
- `quit`

Commands support quoted arguments, for example:

```text
add "alice smith" "bob jones" 42
```

## Smart-contract call format

Contract calls use a Borsh-encoded `CallEnvelope { method, args }` so guest code can decode structured input from `env::read_input()` instead of relying on ad-hoc raw bytes. The host and the SDK share the same envelope shape.

The `blocky-sdk` crate provides helpers to:

- read raw input
- decode a `CallEnvelope`
- decode typed Borsh argument structs
- emit logs
- inspect balance, caller, and deposit
- transfer funds
- read, write, and remove typed storage values

## Writing your first smart contract

A simple first contract is a counter stored under a fixed key.

### 1. Use the included counter example

This repository includes a ready-to-build counter contract at [`examples/contracts/counter/`](examples/contracts/counter/).

### 2. Build it for Wasm

```bash
cargo build --manifest-path examples/contracts/counter/Cargo.toml \
  --target wasm32-unknown-unknown --release
```

The resulting file will be at:

```text
target/wasm32-unknown-unknown/release/counter_contract.wasm
```

### 3. Start the REPL

```bash
cargo run --bin repl
```

### 4. Queue a deployment

Inside the REPL:

```text
deploy alice /full/path/to/target/wasm32-unknown-unknown/release/counter_contract.wasm
mine
```

When queued, the deploy command prints the short derived contract address, for example:

```text
Deploy queued for 1a2b3c4d from alice.
```

Use the `print` command after mining to inspect the full chain output and locate the deployed contract address in context.

### 5. Call the contract

Queue calls against the deployed address:

```text
call alice <contract-address-hex> increment
mine
call alice <contract-address-hex> get
mine
```

The resulting receipts should include log lines like:

```text
log: count = 1
log: count = 1
```

That round-trip demonstrates the full execution path:

- compile a Rust contract to Wasm
- deploy it through a blockchain transaction
- execute it through the VM
- persist storage in world state
- inspect behavior through receipts and REPL output

## Release notes

Project release history starts in [`CHANGELOG.md`](CHANGELOG.md).

## Project scope

Blocky is intentionally small. It does **not** currently aim to provide:

- networking or peer discovery
- persistent on-disk chain/state storage
- mempool gossip
- signatures or cryptographic account authentication
- production consensus or fork choice
- a production-safe contract ABI standard

Those boundaries are deliberate: the code is optimized for experimentation, inspection, and extension, not production deployment.
