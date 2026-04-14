# Counter contract example

This example is the smallest useful Wasm contract for Blocky. It stores a
single `u64` counter under the `count` storage key and exposes two guest
exports:

- `increment` — loads the counter, adds one, stores it back, and logs the new value
- `get` — logs the current counter value without modifying storage

## Build

From the repository root:

```bash
cargo build --manifest-path examples/contracts/counter/Cargo.toml \
  --target wasm32-unknown-unknown --release
```

The compiled Wasm artifact will be written to:

```text
target/wasm32-unknown-unknown/release/counter_contract.wasm
```

## Try it in the REPL

Start the REPL:

```bash
cargo run --bin repl
```

Then deploy and call the contract:

```text
deploy alice /full/path/to/blocky/target/wasm32-unknown-unknown/release/counter_contract.wasm
mine
call alice <contract-address-hex> increment
mine
call alice <contract-address-hex> get
mine
```

Expected receipt logs:

```text
log: count = 1
log: count = 1
```
