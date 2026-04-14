# Blocky

Minimal Rust blockchain demo for learning and experimentation.

## Status

Early scaffold stage. Current target is the RFC-001 v1 milestone: a small CLI
demo that can create a chain, add transactions, mine blocks, print state, and
detect tampering.

## Planned Scope

- SHA-256 block hashing
- Simple proof-of-work mining
- In-memory blockchain validation
- CLI demo flow
- Optional terminal UI / REPL

## Prerequisites

- Rust toolchain with `cargo`

## Getting Started

```bash
cargo test
cargo run --bin demo
cargo run --bin repl
```

Workspace members:
- `blocky` — node, chain, VM, REPL
- `blocky-sdk` — contract-side helpers for decoding `env::read_input()`, calling host wrappers (`log`, `deposit`, `balance`, `caller`, `transfer`), and reading/writing typed Borsh contract storage

The `repl` binary opens an interactive terminal UI powered by `ratatui` and `crossterm`.

Key controls:
- `Enter` executes the current command
- `Up` / `Down` scroll the output pane when the input is empty, or navigate command history when it is not
- `PageUp` / `PageDown` scroll faster
- `Esc` exits

Commands support quoted arguments, for example:
- `add "alice smith" "bob jones" 42`

Smart-contract calls currently frame input as a Borsh-encoded `CallEnvelope { method, args }` so guest code can decode a structured payload from `env::read_input` instead of relying on ad-hoc raw bytes. The `blocky-sdk` crate includes matching helpers to decode the envelope, deserialize method-specific Borsh argument structs, call VM host functions through safe wrappers, and persist typed Borsh values in contract storage.

The UI includes status, pending-transaction, and command-history context in addition to the command output.
