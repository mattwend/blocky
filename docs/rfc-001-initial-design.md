# RFC-001: Blocky — Simple Blockchain Design

**Status:** Draft
**Date:** 2026-04-09

## Goal

A minimal, educational blockchain implementation in Rust. Focus on correctness and clarity over performance or feature completeness.

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
- **No stored `hash`** — computed on demand via `block.compute_hash()`. Chain linkage is established by the *next* block's `prev_hash`.

### Transaction

```rust
struct Transaction {
    sender: String,
    receiver: String,
    amount: u64,
    timestamp: u64,
}
```

### Chain

```rust
struct Blockchain {
    chain: Vec<Block>,
    pending_transactions: Vec<Transaction>,
    difficulty: u32,
}
```

## Hashing

- Use **SHA-256** (via the `sha2` crate).
- Hash input: `"{timestamp}{prev_hash}{nonce}{transactions_serialized}"`.
- `Hash` is a `[u8; 32]` (or a newtype wrapper around it).

## Proof of Work

Simple difficulty-based PoW:

1. Miner increments `nonce` starting from 0.
2. Block is valid when `hash` has `difficulty` leading zero bits.
3. Initial difficulty: **2** (adjustable later).

No difficulty adjustment algorithm in v1 — just a static value.

## Chain Validation

A chain is valid if and only if:

1. Genesis block (first in `chain`) has `prev_hash == [0u8; 32]`.
2. For each subsequent block:
   - `block.prev_hash == prev.compute_hash()`
   - `block.compute_hash()` satisfies the difficulty requirement.

## API (Public Interface)

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

## Project Layout

```
blocky/
  Cargo.toml
  src/
    main.rs          # CLI entry point / demo
    lib.rs           # Re-exports
    block.rs         # Block + hashing + mining
    transaction.rs   # Transaction type
    chain.rs         # Blockchain logic + validation
```

## Dependencies

| Crate       | Purpose            |
|-------------|--------------------|
| `sha2`      | SHA-256 hashing    |
| `serde`     | Serialization      |
| `serde_json`| Hash input format  |
| `hex`       | Hash display       |
| `chrono`    | Timestamps         |
| `ratatui`   | Terminal UI / REPL |
| `crossterm` | Terminal backend   |

## Out of Scope (v1)

- Networking / P2P
- Persistent storage
- Wallets / public-key cryptography
- Merkle trees
- Difficulty adjustment
- Consensus between nodes
- Smart contracts

## Decisions

1. **Transaction validation** — No balance tracking in v1. Transactions are recorded blindly. Balance enforcement deferred to RFC-002.
2. **Serialization format** — `serde_json` for hash input. Good enough for our purpose.
3. **CLI** — Interactive REPL built with `ratatui` + `crossterm`.
4. **Amount type** — `u64` integer (smallest units). No floating point.

## Proposed v1 Milestone

A CLI demo that:
1. Creates a blockchain
2. Adds a few transactions
3. Mines blocks
4. Prints the chain
5. Validates the chain (and detects tampering if a block is mutated)
