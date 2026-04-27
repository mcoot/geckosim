# Generated TypeScript bindings

Files in this directory are **auto-generated** from the Rust wire types in
`crates/core` and `crates/protocol` via `ts-rs`. Do not edit by hand —
edits are clobbered by the next regeneration.

To regenerate:

`pnpm gen-types`

(equivalent to `cargo test -p gecko-sim-core --features export-ts && cargo test -p gecko-sim-protocol --features export-ts` from the workspace root)

The generator is idempotent: re-running on unchanged Rust types produces
zero diff. CI (when wired) gates on this property.
