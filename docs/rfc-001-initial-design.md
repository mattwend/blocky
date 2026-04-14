# RFC-001: Blocky — Simple Blockchain Design

**Status:** Implemented, partially superseded by RFC-002
**Created:** 2026-04-09
**Last reviewed:** 2026-04-14
**Implemented by:** `df27c13`, `c3efb51`, `829d519`, `8d56694`, `7cc4db0`, `d44ce23`, `41c3e3f`

## Goal

A minimal, educational blockchain implementation in Rust. Focus on correctness and clarity over performance or feature completeness.

## Historical role

This RFC established the initial Blocky direction: blocks, transactions, proof-of-work mining, chain validation, and an interactive CLI / REPL experience.

Repository history shows this document was added before the first core implementation and REPL work:

- `df27c13` — `docs(rfc): add blockchain rfc`
- `c3efb51` — `feat(core): add blockchain library and demo binary`
- `829d519` — `feat(repl): add interactive command loop scaffold`
- `8d56694` — `feat(repl): add ratatui terminal interface`
- `7cc4db0` — `feat(repl): improve tui layout and status panels`

That history justifies treating this file as an actual internal RFC rather than a loose design note.

## Core Data Structures

### Block

```rust
struct Block {
    timestamp: u64,
    prev_hash: Hash,
    nonce: u64,
    transactions: Vec<Transaction>,
}
```

- **No `index`** — derived from position in `Blockchain.chain`.
- **No stored `hash`** — computed on demand via `block.compute_hash()`. Chain linkage is established by the next block's `prev_hash`.

### Transaction

Original proposal:

```rust
struct Transaction {
    sender: String,
    receiver: String,
    amount: u64,
    timestamp: u64,
}
```

This shape was sufficient for the initial simple chain design, but it was later replaced by the address + nonce + payload model defined in RFC-002.

### Chain

Original proposal:

```rust
struct Blockchain {
    chain: Vec<Block>,
    pending_transactions: Vec<Transaction>,
    difficulty: u32,
}
```

This remains the conceptual foundation, but the implemented `Blockchain` has since grown additional fields for world state, the VM, and receipts.

## Hashing

- Use **SHA-256** (via the `sha2` crate).
- `Hash` is a `[u8; 32]` (or a newtype wrapper around it).

The initial design described a simple serialized hash input over block fields. The exact serialization details should now be treated as implementation-defined by the current code.

## Proof of Work

Simple difficulty-based PoW:

1. Miner increments `nonce` starting from 0.
2. Block is valid when `hash` has `difficulty` leading zero bits.
3. Initial difficulty is static and caller-configurable.

No difficulty adjustment algorithm in v1.

## Chain Validation

A chain is valid if and only if:

1. The genesis block has `prev_hash == [0u8; 32]`.
2. Every block hash satisfies the configured difficulty requirement.
3. For each non-genesis block, `block.prev_hash == prev.compute_hash()`.

## API (Original Public Interface)

```rust
impl Blockchain {
    fn new(difficulty: u32) -> Self;              // Creates chain with genesis block
    fn add_transaction(&mut self, tx: Transaction) -> Result<()>;
    fn mine_pending(&mut self) -> Result<Block>;  // Mine pending txs into a new block
    fn is_valid(&self) -> bool;                   // Full chain validation
}

impl Block {
    fn new(transactions: Vec<Transaction>, prev_hash: Hash) -> Self;
    fn mine(&mut self, difficulty: u32);           // PoW loop
    fn compute_hash(&self) -> Hash;
}
```

The implemented code still follows this shape at a high level, although later RFCs extended transaction validation and mining semantics.

## Project Layout (Original)

```text
blocky/
  Cargo.toml
  src/
    main.rs          # CLI entry point / demo
    lib.rs           # Re-exports
    block.rs         # Block + hashing + mining
    transaction.rs   # Transaction type
    chain.rs         # Blockchain logic + validation
```

The current repository has grown beyond this initial scope and now also includes app binaries, world state, receipts, VM code, call ABI helpers, and the `blocky-sdk` workspace member.

## Dependencies

Original dependencies proposed here:

| Crate       | Purpose            |
|-------------|--------------------|
| `sha2`      | SHA-256 hashing    |
| `serde`     | Serialization      |
| `serde_json`| Hash input format  |
| `hex`       | Hash display       |
| `chrono`    | Timestamps         |
| `ratatui`   | Terminal UI / REPL |
| `crossterm` | Terminal backend   |

Some of these choices were later extended or revised by RFC-002 and the implementation that followed.

## Out of Scope (original v1)

- Networking / P2P
- Persistent storage
- Wallets / public-key cryptography
- Merkle trees
- Difficulty adjustment
- Consensus between nodes
- Smart contracts

The final item became outdated once RFC-002 was adopted and implemented.

## Decisions

1. **Transaction validation** — No balance tracking in this initial version. Balance enforcement was explicitly deferred.
2. **Serialization format** — Initial proposal used a simple serialization approach suitable for experimentation.
3. **CLI** — Interactive REPL built with `ratatui` + `crossterm`.
4. **Amount type** — `u64` integer units; no floating point.

## Proposed v1 Milestone

A CLI demo that:
1. Creates a blockchain
2. Adds a few transactions
3. Mines blocks
4. Prints the chain
5. Validates the chain and detects tampering

## Implementation Notes

This RFC was implemented in spirit and in sequence, but the repository evolved beyond the exact structures shown above.

Implemented outcomes traceable to this RFC include:

- the block / chain / mining foundation
- chain validity checks based on previous hashes and proof of work
- a demo binary and interactive REPL / TUI workflow

## Accepted Deviations

### Transaction model superseded

The string-based sender / receiver / amount transaction model in this RFC is no longer current. It was superseded by RFC-002's transaction payload model with:

- `Address = [u8; 32]`
- sender nonce tracking
- `Payload::{Transfer, Deploy, Call}`

### Blockchain structure extended

The current `Blockchain` implementation includes additional fields not described here, including world state, receipts, and a Wasm VM engine.

### Smart contracts no longer out of scope

This RFC originally treated smart contracts as out of scope for v1. That decision was superseded by RFC-002 and the subsequent implementation work landing on 2026-04-12 through 2026-04-14.

## Relationship to later RFCs

RFC-002 extends and partially supersedes this RFC by introducing:

- world state
- balance enforcement
- payload-based transactions
- Wasm smart contracts
- execution receipts
- gas metering
- the contract SDK

This RFC should be read as the historical foundation for Blocky's initial architecture, not as a complete description of the current codebase.
