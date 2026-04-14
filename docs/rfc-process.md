# RFC Process

Blocky uses RFCs as lightweight internal architecture proposals for substantial design changes.

## When to write an RFC

Write an RFC for changes that materially affect one or more of:

- core architecture
- execution or validation semantics
- transaction or state formats
- VM or contract interfaces
- public crate APIs
- persistence, networking, or other major subsystem boundaries

Small refactors, bug fixes, and local implementation details do not need an RFC.

## RFC lifecycle

RFCs use the following statuses:

- **Draft** — proposal under discussion; details may change
- **Accepted** — direction agreed; implementation may not be complete yet
- **Implemented** — the RFC has landed in code, possibly with documented deviations
- **Superseded** — a later RFC replaced this one in whole or in part
- **Rejected** — proposal was considered but not adopted

## Required sections

Each RFC should include:

- title and RFC number
- status
- date created
- optional dependencies on earlier RFCs
- goal / motivation
- detailed design
- out-of-scope items
- implementation plan or ordering
- open questions

Once implementation lands, the RFC should also include:

- implementation status notes
- links or references to key commits / PRs
- accepted deviations from the original proposal
- supersession notes when a later RFC changes the design

## Source of truth

RFCs are design records, not immutable standards documents.

- Before implementation, the RFC is the proposed direction.
- After implementation, the current code is authoritative.
- If code intentionally diverges from the RFC, record that in an `Implementation Notes` or `Accepted Deviations` section.

## Repository history

This process reflects how Blocky has already evolved:

- `RFC-001` was added before the initial blockchain and REPL implementation.
- `RFC-002` was added before the world-state, VM, SDK, receipts, and gas-metering work.

Those documents served as real design drivers, but they were not updated after implementation. Going forward, RFCs should be maintained through implementation and supersession.
