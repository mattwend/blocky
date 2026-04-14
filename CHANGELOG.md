# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project is preparing for semantic version tags.

## [1.0.0] - 2026-04-14

Initial open-source release.

### Added

- Educational blockchain core with blocks, transactions, proof-of-work mining, validation, and chain inspection APIs.
- In-memory world state for balances, contract code, and contract storage.
- Wasm smart-contract execution with deterministic host interactions, gas metering, and execution receipts.
- Shared call ABI and a `blocky-sdk` crate for contract-side input decoding, host calls, logging, transfers, and typed storage helpers.
- Interactive terminal REPL for deploying contracts, calling methods, mining blocks, and inspecting state.
- Demo binary that walks through mining and chain validation behavior.
- Example counter contract showing the full Rust-to-Wasm workflow.
- Project documentation including architecture notes, RFCs, contribution guidance, code of conduct, license, and security policy.

### Changed

- Documentation, metadata, and community files were aligned for public open-source release.
- README content was expanded into a newcomer-oriented guide covering architecture, REPL usage, contract calls, and first-contract workflow.

### Fixed

- Mining now applies pending transactions atomically.
- VM and chain execution paths propagate host, receipt, serialization, and block-hash errors instead of falling back silently.
- Contract aborts trap correctly during execution.
- Tests and SDK e2e flows were cleaned up for reproducible contract builds.

## [Unreleased]

- No unreleased changes yet.
