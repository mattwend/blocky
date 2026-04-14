# RFC-002: Smart Contracts via WebAssembly

**Status:** Implemented, with follow-up validation and hardening work still open
**Created:** 2026-04-11
**Depends on:** RFC-001
**Last reviewed:** 2026-04-14
**Implemented by:** `0123cd3`, `e04d852`, `79a960a`, `70b9f37`, `d780125`, `6bee094`, `a85ba6e`, `d434dfa`, `71b275d`, `b08c89f`, `9a2f01f`, `bf35784`, `55aa4b2`, `3d6388d`, `d1b3e59`

## Goal

Add smart contract support to Blocky. Contracts are written in Rust, compiled to Wasm, and executed in a sandboxed runtime. This RFC also introduces world state as a prerequisite.

## Historical role

Repository history shows this RFC was added before the world-state, VM, receipts, SDK, and gas-metering work landed:

- `0123cd3` — `docs(rfc): add smart contract rfc`
- `e04d852` — `feat(state): add world state and payload-based transactions`
- `79a960a` — `feat(vm): add host function state scaffolding`
- `70b9f37` — `feat(vm): wire contract execution into state transitions`
- `d780125` — `feat(chain): add execution receipts and vm error propagation`
- `d434dfa` — `feat(vm): add structured call envelope`
- `71b275d` — `feat(sdk): add call envelope decoding helpers`
- `b08c89f` — `feat(sdk): add host wrappers and typed storage`
- `9a2f01f` — `feat(vm): add gas metering and repl contract commands`
- `bf35784` — `refactor(vm): make transactions deterministic and simplify execution`

This file therefore documents a real architecture phase, not just an aspirational note.

## Design Principles

1. **Determinism** — All nodes must produce identical results. The implementation configures wasmtime for deterministic execution and avoids float-based contract interfaces.
2. **Sandboxing** — Contracts interact with the outside world only through host functions.
3. **Metering** — Contract execution is fuel-metered.
4. **Simplicity** — The initial implementation focuses on a small, single-contract-call execution model.

## World State

State is the layer between transactions and the chain. It is required for balances, contract code, and contract storage.

### Account Model

Implemented shape:

```rust
pub struct AccountState {
    pub balance: u64,
    pub nonce: u64,
    pub code: Option<Vec<u8>>,
    pub storage: BTreeMap<Vec<u8>, Vec<u8>>,
}
```

### State Store

Implemented shape:

```rust
pub struct WorldState {
    accounts: HashMap<Address, AccountState>,
}

pub type Address = [u8; 32];
```

Implemented operations include:

- `get_account`
- `get_or_create`
- `get_balance`
- `set_balance`
- `transfer`
- `apply_block`
- `apply_transaction`
- `apply_transaction_with_vm`

`WorldState` lives on `Blockchain` and is updated during mined block execution.

### Balance Enforcement

`Blockchain::add_transaction` now enforces sender balance constraints, including reserved cost from pending transactions.

## Transaction Types

The simple transaction form from RFC-001 was replaced with a payload-based model:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct Transaction {
    pub sender: Address,
    pub nonce: u64,
    pub payload: Payload,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum Payload {
    Transfer {
        receiver: Address,
        amount: u64,
    },
    Deploy {
        code: Vec<u8>,
    },
    Call {
        contract: Address,
        method: String,
        args: Vec<u8>,
        deposit: u64,
    },
}
```

### Address Derivation

Implemented behavior:

- **User accounts:** helper support exists for deriving an address as SHA-256 of a name string for local experimentation.
- **Contract accounts:** derived deterministically from `(sender, nonce)` using SHA-256.

## Wasm Runtime

### Engine: wasmtime

The implementation uses `wasmtime` and configures the engine for deterministic execution with fuel consumption enabled.

### Execution Flow

Implemented high-level flow:

```text
Call transaction received
  │
  ▼
Load target contract bytecode from WorldState
  │
  ▼
Prepare a Store with host state and fuel budget
  │
  ▼
Instantiate a cached or newly compiled wasmtime Module
  │
  ▼
Resolve the exported method by name
  │
  ▼
Execute the method with host functions linked under env
  │
  ▼
On success: commit resulting state and emit receipt
On failure or abort: revert changes and emit failure receipt
```

### Module Caching

Implemented.

Compiled `wasmtime::Module` instances are cached by code hash in `VmEngine`.

## Host Functions

Host functions remain the contract's interface to the runtime. They are linked into the Wasm instance through the `env` import module.

The RFC-level interface remains valid in spirit:

| Function | Purpose |
|---|---|
| `storage_read` | Read contract storage |
| `storage_write` | Write contract storage |
| `storage_remove` | Delete contract storage |
| `get_balance` | Read current contract balance |
| `get_caller` | Read caller address |
| `get_deposit` | Read attached deposit |
| `transfer` | Transfer funds from contract balance |
| `log` | Emit receipt logs |
| `abort` | Abort execution and revert |

The exact host import wiring and ABI details should now be treated as authoritative in the current code and SDK.

## Serialization: Borsh

Borsh is now central to the contract boundary and several internal value representations.

Implemented usage includes:

- Borsh serialization for transaction payload types
- Borsh-encoded `CallEnvelope { method, args }` framing for contract call input
- Borsh storage helpers in `blocky-sdk`
- Borsh receipt serialization derives

The original note that internal chain serialization would migrate further over time remains reasonable, but the current codebase already uses Borsh more broadly than this RFC originally described.

## Call Envelope

An important implementation detail added during RFC-002 work is the structured call envelope:

```rust
pub struct CallEnvelope {
    pub method: String,
    pub args: Vec<u8>,
}
```

Contract call payloads are framed as a Borsh-encoded `CallEnvelope` so guest code can decode structured input from `env::read_input()`.

## Gas Metering

Implemented.

The VM uses wasmtime fuel metering and reports gas used per transaction. Current gas constants include:

- base transaction cost
- storage read/write/remove cost
- transfer cost
- deploy cost per byte
- default gas limit

The exact values are defined in `src/vm/gas.rs`.

## Contract SDK

Implemented as the `blocky-sdk` workspace member.

Implemented capabilities include:

- `CallEnvelope` encode/decode helpers
- `input()` and `call_envelope()` accessors
- typed `decode_args<T>()`
- host wrappers for `log`, `balance`, `deposit`, `caller`, and `transfer`
- typed Borsh storage helpers
- Wasm allocator wiring
- panic-to-abort behavior for Wasm usage

## Execution Receipts

Implemented shape:

```rust
pub struct Receipt {
    pub tx_hash: [u8; 32],
    pub success: bool,
    pub gas_used: u64,
    pub logs: Vec<String>,
    pub error: Option<String>,
}
```

Receipts are stored on `Blockchain` as `Vec<Vec<Receipt>>`, one receipt list per mined block.

Receipts are informational in v1 and are not part of block hashing.

## Changes to Existing Types

### Blockchain

Implemented shape:

```rust
pub struct Blockchain {
    pub chain: Vec<Block>,
    pub pending_transactions: Vec<Transaction>,
    pub difficulty: u32,
    pub state: WorldState,
    pub vm: VmEngine,
    pub receipts: Vec<Vec<Receipt>>,
}
```

### Block

No structural smart-contract-specific changes were required. Blocks still contain transactions.

### Validation

The RFC proposed extending `is_valid` to re-execute transactions against fresh state. That has not landed yet.

Current implementation validates:

- genesis `prev_hash`
- proof-of-work difficulty
- previous-hash linkage

State replay validation and state-root style validation remain future work.

## Project Layout

Implemented additions include:

```text
blocky/
  src/
    call_abi.rs       # CallEnvelope re-export and decoding helpers
    receipt.rs        # Execution receipts
    state.rs          # WorldState, AccountState
    transaction.rs    # Address and Payload transaction model
    vm/
      mod.rs          # VmEngine, module cache, execution flow
      host.rs         # Host function implementations
      gas.rs          # Gas constants
  blocky-sdk/
    Cargo.toml
    src/
      lib.rs          # Contract-side helpers and host wrappers
```

## New Dependencies

Key dependencies introduced by this RFC's implementation:

| Crate | Purpose |
|---|---|
| `wasmtime` | Wasm execution engine |
| `borsh` | Deterministic binary serialization |

## Out of Scope (v1 contracts)

Still out of scope unless later RFCs say otherwise:

- Cross-contract calls
- Contract upgrades / proxy patterns
- Events or indexing beyond simple receipt logs
- Public-key signatures on transactions
- State roots / Merkle proofs
- Persistent storage
- Contract standards
- Additional Wasm validation policy beyond runtime checks already in place

## Implementation Order

The proposed order in this RFC closely matched reality:

1. world state and balance tracking
2. payload-based transactions
3. VM scaffold
4. host functions
5. deploy and call execution
6. gas metering
7. receipts
8. SDK crate
9. REPL contract commands

## Implementation Notes

This RFC has been substantially implemented.

Key outcomes that are now present in the codebase:

- world state and account storage
- balance-aware transaction admission
- deterministic contract address derivation
- Wasm execution through wasmtime
- cached modules by code hash
- structured call envelopes
- contract SDK helpers
- gas metering
- execution receipts
- failure propagation and atomic mining behavior

## Accepted Deviations

### Validation is narrower than proposed

The RFC proposed replay-based validation in `is_valid`. Current code does not yet re-execute transactions during validation.

### Receipts live on `Blockchain`

The RFC described receipts as stored alongside blocks. The implemented model stores them on `Blockchain` in a parallel `Vec<Vec<Receipt>>`.

### ABI details are code-defined

The RFC specified host behavior at a conceptual level. The exact ABI, import names, and call-envelope framing are now best understood from `src/vm/host.rs`, `src/call_abi.rs`, and `blocky-sdk/src/lib.rs`.

## Open Questions

These remain good follow-up topics:

1. **Memory limits** — should Wasm linear memory be capped explicitly?
2. **Code size limits** — should deploys enforce a maximum bytecode size?
3. **Storage limits** — should storage be capped beyond gas pricing?
4. **Address format** — should human-readable encodings be added?
5. **Validation semantics** — should `is_valid` grow replay-based state validation?
