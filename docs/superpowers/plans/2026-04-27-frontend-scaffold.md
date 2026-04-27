# Frontend scaffold v0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up a Next.js client at `apps/web/` that connects to the running sim host via WebSocket, renders the live agent list with their needs, and exposes speed/pause controls. Wire the `ts-rs` toolchain into the Rust workspace so `crates/core` and `crates/protocol` types auto-generate to TypeScript at `apps/web/src/types/sim/` (committed to git).

**Architecture:** Two halves. Rust side: feature-gated `ts-rs` derives on wire types in `crates/core` and `crates/protocol`; `cargo test -p gecko-sim-protocol --features export-ts` writes the typed `.ts` bindings. Frontend side: Next.js 15 App Router + Tailwind v4 + Vitest, with a `<SimConnectionProvider>` Context owning the WebSocket lifecycle, a pure `reduce(state, ServerMessage) → state` exposed via `useSimConnection()`, and three sibling components — `<ConnectionStatus>`, `<AgentList>`, `<Controls>` — composed in `app/page.tsx`. Single route, no Three.js.

**Tech Stack:** Rust 2021 + `ts-rs 10` (build-time only; feature-gated). Node 22 + pnpm 10 + Next.js 15 (App Router) + React 19 + TypeScript 5 + Tailwind v4 + Vitest 2 + jsdom + @testing-library/react.

**Reference:** Spec at [`docs/superpowers/specs/2026-04-27-frontend-scaffold-design.md`](../specs/2026-04-27-frontend-scaffold-design.md). ADRs 0002 (stack), 0012 (architecture), 0013 (transport) are upstream contracts.

**VCS note:** This repo uses `jj` (jujutsu), colocated with git. **Never use raw `git`.** Each task starts with `jj new -m "<task title>"` to create a fresh commit; jj automatically snapshots edits as you work. There is no separate "commit" command.

**Toolchain prereqs (verify first if any task fails):**
- `cargo` (workspace builds today)
- `pnpm --version` ≥ 10 (verified at plan-time: 10.33.2 on this machine)
- `node --version` ≥ 22 (verified at plan-time: v22.17.1)

---

## File Structure

**New files (Rust side):** none — all changes are in-place edits to existing crate files.

**Modified files (Rust side):**
- `Cargo.toml` (workspace) — Task 1
- `crates/core/Cargo.toml` — Task 1
- `crates/core/src/ids.rs` (`id_newtype!` macro) — Task 2
- `crates/core/src/agent/mod.rs` (`Needs`) — Task 2
- `crates/core/src/snapshot.rs` (`Snapshot`, `AgentSnapshot`) — Task 2
- `crates/protocol/Cargo.toml` — Task 1
- `crates/protocol/src/messages.rs` (`WireFormat`, `ServerMessage`, `ClientMessage`, `PlayerInput`) — Task 3

**New files (frontend side):**
- `apps/web/package.json` — Task 4
- `apps/web/pnpm-lock.yaml` — Task 4 (generated)
- `apps/web/tsconfig.json` — Task 4
- `apps/web/next.config.ts` — Task 4
- `apps/web/eslint.config.mjs` — Task 4
- `apps/web/postcss.config.mjs` — Task 4
- `apps/web/vitest.config.ts` — Task 4
- `apps/web/.gitignore` — Task 4 (Next.js scaffold writes one)
- `apps/web/public/` — Task 4 (empty or minimal favicon)
- `apps/web/src/app/layout.tsx` — Task 4
- `apps/web/src/app/page.tsx` — Task 4 (placeholder), then rewritten Task 7
- `apps/web/src/app/globals.css` — Task 4
- `apps/web/src/types/sim/README.md` — Task 4
- `apps/web/src/types/sim/*.ts` — Task 5 (auto-generated, committed)
- `apps/web/src/lib/sim/reducer.ts` — Task 6
- `apps/web/src/lib/sim/reducer.test.ts` — Task 6
- `apps/web/src/lib/sim/connection.tsx` — Task 6
- `apps/web/src/components/ConnectionStatus.tsx` — Task 7
- `apps/web/src/components/AgentList.tsx` — Task 7
- `apps/web/src/components/Controls.tsx` — Task 7

**Modified files (frontend side):**
- `apps/web/README.md` — Task 8 (replaces placeholder)
- `apps/web/src/app/page.tsx` — Task 7 (replaces Task 4 placeholder)

**Modified (top-level):**
- `.gitignore` — Task 4 (adds `apps/web/node_modules/`, `apps/web/.next/`, `apps/web/.turbo/`)

---

## Task 1: Workspace `ts-rs` dep + `export-ts` features on core/protocol

**Files:**
- Modify: `Cargo.toml` (workspace)
- Modify: `crates/core/Cargo.toml`
- Modify: `crates/protocol/Cargo.toml`

This task adds the `ts-rs` dependency machinery without yet wiring any derive. Verification gate is `cargo build` clean for both default and `--features export-ts`.

- [ ] **Step 1.1: Start the task commit**

```bash
jj new -m "Frontend: ts-rs workspace dep + export-ts feature on core/protocol"
```

- [ ] **Step 1.2: Add `ts-rs` to workspace `Cargo.toml`**

In the repo root `Cargo.toml`, find the `[workspace.dependencies]` block. After the `tempfile = "3"` line (added in the RON content pass), append:

```toml
ts-rs = { version = "10", features = ["serde-compat"] }
```

The `serde-compat` feature makes `ts-rs` honor `#[serde(tag = "...")]`, `#[serde(rename_all = "...")]`, etc., when generating TS — required for our tagged-union wire shapes.

- [ ] **Step 1.3: Add optional `ts-rs` dep + `export-ts` feature to `crates/core/Cargo.toml`**

In `crates/core/Cargo.toml`, find the `[dependencies]` block. Append:

```toml
ts-rs = { workspace = true, optional = true }
```

After the `[dependencies]` block (and any existing `[dev-dependencies]` block), add:

```toml
[features]
export-ts = ["dep:ts-rs"]
```

If a `[features]` block already exists, append the new line to it instead. (At plan-time, no `[features]` block exists in `crates/core/Cargo.toml`.)

- [ ] **Step 1.4: Add optional `ts-rs` dep + `export-ts` feature to `crates/protocol/Cargo.toml`**

Same shape as core, but the protocol's `export-ts` feature transitively enables core's so a single command exports everything.

In `crates/protocol/Cargo.toml`, append to `[dependencies]`:

```toml
ts-rs = { workspace = true, optional = true }
```

After the `[dependencies]` (and `[dev-dependencies]`) block, add:

```toml
[features]
export-ts = ["dep:ts-rs", "gecko-sim-core/export-ts"]
```

- [ ] **Step 1.5: Verify default-features build still clean**

```bash
cargo build --workspace
```

Expected: clean. No code change yet so this is a sanity gate.

- [ ] **Step 1.6: Verify `--features export-ts` build clean**

```bash
cargo build -p gecko-sim-protocol --features export-ts
```

Expected: clean. The feature pulls `ts-rs` into the dep graph for both crates but no derives use it yet, so the type just sits idle.

- [ ] **Step 1.7: Verify clippy clean for both feature configurations**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p gecko-sim-protocol --all-targets --features export-ts -- -D warnings
```

Expected: both clean.

- [ ] **Step 1.8: Confirm commit scope**

```bash
jj st
jj diff --stat
```

Expected: three files modified (`Cargo.toml`, `crates/core/Cargo.toml`, `crates/protocol/Cargo.toml`); a `Cargo.lock` update auto-snapshotted (acceptable scope expansion since we added a workspace dep).

---

## Task 2: `ts-rs` derives on `core` wire types

**Files:**
- Modify: `crates/core/src/ids.rs` (`id_newtype!` macro)
- Modify: `crates/core/src/agent/mod.rs` (`Needs`)
- Modify: `crates/core/src/snapshot.rs` (`Snapshot`, `AgentSnapshot`)

The export path resolves relative to each crate's `Cargo.toml`, so `../../apps/web/src/types/sim/` from `crates/core/` lands at the workspace's `apps/web/src/types/sim/`. That directory does NOT exist yet — Task 4 creates it. Until then, we **do not run** `cargo test -p gecko-sim-protocol --features export-ts`. The verification gate for Task 2 is build + clippy only.

- [ ] **Step 2.1: Start the task commit**

```bash
jj new -m "Frontend: ts-rs derives on core wire types"
```

- [ ] **Step 2.2: Add TS derive to the `id_newtype!` macro**

In `crates/core/src/ids.rs`, the existing macro is:

```rust
macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        pub struct $name(pub u64);

        impl $name {
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn raw(self) -> u64 {
                self.0
            }
        }
    };
}
```

Replace it with:

```rust
macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
        #[cfg_attr(
            feature = "export-ts",
            ts(export, export_to = "../../apps/web/src/types/sim/")
        )]
        pub struct $name(pub u64);

        impl $name {
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn raw(self) -> u64 {
                self.0
            }
        }
    };
}
```

(No `transparent` here — `ts-rs` will render `pub struct AgentId(pub u64)` as `type AgentId = number` automatically for tuple structs with one field, regardless of the `transparent` flag, because the struct has no other fields. If a future check shows the generated TS instead has `{ "0": number }`, add `#[cfg_attr(feature = "export-ts", ts(transparent))]` to fix.)

- [ ] **Step 2.3: Add TS derive to `Needs` in `crates/core/src/agent/mod.rs`**

Find the existing `Needs` struct (around line 99):

```rust
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Needs {
    pub hunger: f32,
    pub sleep: f32,
    pub social: f32,
    pub hygiene: f32,
    pub fun: f32,
    pub comfort: f32,
}
```

Add two `cfg_attr` lines between the existing `#[derive(...)]` and `pub struct Needs`:

```rust
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct Needs {
    pub hunger: f32,
    pub sleep: f32,
    pub social: f32,
    pub hygiene: f32,
    pub fun: f32,
    pub comfort: f32,
}
```

- [ ] **Step 2.4: Add TS derives to `Snapshot` and `AgentSnapshot`**

In `crates/core/src/snapshot.rs`, decorate both structs the same way. For `Snapshot`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct Snapshot {
    pub tick: u64,
    pub agents: Vec<AgentSnapshot>,
}
```

Apply the same `cfg_attr` pair to `AgentSnapshot` immediately below it.

- [ ] **Step 2.5: Verify default-features build still clean**

```bash
cargo build --workspace
cargo test --workspace
```

Expected: both clean. With the feature off, the `cfg_attr` lines compile away.

- [ ] **Step 2.6: Verify `--features export-ts` build clean**

```bash
cargo build -p gecko-sim-core --features export-ts
```

Expected: clean. The derive macros run; without the export directory existing yet, the auto-generated `#[test]` functions live in the test binary but we are NOT running them in this task.

- [ ] **Step 2.7: Verify clippy clean for both configurations**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p gecko-sim-core --all-targets --features export-ts -- -D warnings
```

Expected: both clean.

- [ ] **Step 2.8: Confirm commit scope**

```bash
jj st
```

Expected: 3 files modified — `crates/core/src/{ids,snapshot}.rs` and `crates/core/src/agent/mod.rs`. Plus a `Cargo.lock` update if `ts-rs` had transitive deps not yet resolved (acceptable).

---

## Task 3: `ts-rs` derives on `protocol` wire types

**Files:**
- Modify: `crates/protocol/src/messages.rs`

- [ ] **Step 3.1: Start the task commit**

```bash
jj new -m "Frontend: ts-rs derives on protocol wire types"
```

- [ ] **Step 3.2: Add TS derives to `WireFormat`, `ServerMessage`, `ClientMessage`, `PlayerInput`**

In `crates/protocol/src/messages.rs`, decorate each public enum with the same `cfg_attr` pair. For `WireFormat`:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub enum WireFormat {
    Json,
}
```

For `ServerMessage`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub enum ServerMessage {
    Hello {
        protocol_version: u32,
        format: WireFormat,
    },
    Init {
        current_tick: u64,
        snapshot: Snapshot,
    },
    Snapshot {
        snapshot: Snapshot,
    },
}
```

For `ClientMessage`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub enum ClientMessage {
    ClientHello {
        last_known_tick: Option<u64>,
    },
    PlayerInput(PlayerInput),
}
```

For `PlayerInput`:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub enum PlayerInput {
    SetSpeed { multiplier: f32 },
    TogglePause,
}
```

(Per the spec, `PROTOCOL_VERSION: u32 = 1` is NOT exported via ts-rs — frontend hardcodes the constant. ts-rs doesn't auto-export bare consts and the manual export shim isn't worth the complexity.)

- [ ] **Step 3.3: Verify default-features build still clean**

```bash
cargo build --workspace
cargo test --workspace
```

Expected: clean. Existing `crates/protocol/tests/roundtrip.rs` continues to pass — derives are conditional.

- [ ] **Step 3.4: Verify `--features export-ts` build clean**

```bash
cargo build -p gecko-sim-protocol --features export-ts
```

Expected: clean.

- [ ] **Step 3.5: Verify clippy clean for both configurations**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p gecko-sim-protocol --all-targets --features export-ts -- -D warnings
```

Expected: both clean.

- [ ] **Step 3.6: Confirm commit scope**

```bash
jj st
```

Expected: 1 file modified — `crates/protocol/src/messages.rs`.

---

## Task 4: Scaffold the Next.js app under `apps/web/`

**Files:**
- Modify: `apps/web/README.md` (replaces placeholder)
- Create: `apps/web/{package.json,pnpm-lock.yaml,tsconfig.json,next.config.ts,eslint.config.mjs,postcss.config.mjs,vitest.config.ts,.gitignore}`
- Create: `apps/web/public/` (auto-populated by create-next-app)
- Create: `apps/web/src/app/{layout.tsx,page.tsx,globals.css}`
- Create: `apps/web/src/types/sim/README.md`
- Modify: `.gitignore` (workspace root) — add Next.js paths

Approach: run `pnpm create next-app` non-interactively, then layer on the test setup (Vitest + Testing Library) and the placeholder typed-bindings dir.

- [ ] **Step 4.1: Start the task commit**

```bash
jj new -m "Frontend: scaffold Next.js app under apps/web/ (App Router, TS, Tailwind v4, Vitest)"
```

- [ ] **Step 4.2: Remove the placeholder `apps/web/README.md` so create-next-app's scaffold can proceed**

```bash
rm apps/web/README.md
```

(create-next-app refuses to scaffold into a directory containing files. The directory itself stays.)

- [ ] **Step 4.3: Run create-next-app non-interactively**

```bash
cd apps/web && pnpm create next-app@latest . \
    --ts \
    --tailwind \
    --eslint \
    --app \
    --src-dir \
    --use-pnpm \
    --import-alias "@/*" \
    --turbopack \
    --skip-install \
    --yes
```

This writes `package.json`, `tsconfig.json`, `next.config.ts`, `postcss.config.mjs`, `eslint.config.mjs`, `src/app/{layout,page}.tsx`, `src/app/globals.css`, `public/*`, `.gitignore`, and a fresh `README.md`.

`--skip-install` lets us add the Vitest deps in one install round below. If the flag isn't recognized by your installed `pnpm create next-app`, drop it and accept that pnpm will run `install` twice.

- [ ] **Step 4.4: Add Vitest + Testing Library dev dependencies**

```bash
cd apps/web && pnpm add -D \
    vitest@^2 \
    @vitejs/plugin-react@^4 \
    jsdom@^25 \
    @testing-library/react@^16 \
    @testing-library/dom@^10 \
    @testing-library/jest-dom@^6
```

(Versions pin to current major releases as of 2026-04. `pnpm install` runs implicitly and writes `pnpm-lock.yaml`.)

- [ ] **Step 4.5: Add the `test` and `gen-types` scripts to `apps/web/package.json`**

In `apps/web/package.json`, find the `"scripts"` block. After the `"lint"` line (added by create-next-app), insert:

```json
"test": "vitest run",
"test:watch": "vitest",
"gen-types": "cd ../.. && cargo test -p gecko-sim-protocol --features export-ts"
```

(Don't add `--turbo` or other flags to existing scripts; leave them as create-next-app wrote them.)

- [ ] **Step 4.6: Create `apps/web/vitest.config.ts`**

```ts
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
  },
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
});
```

- [ ] **Step 4.7: Create `apps/web/src/types/sim/README.md`**

```markdown
# Generated TypeScript bindings

Files in this directory are **auto-generated** from the Rust wire types in
`crates/core` and `crates/protocol` via `ts-rs`. Do not edit by hand —
edits are clobbered by the next regeneration.

To regenerate:

```bash
pnpm gen-types
# or, equivalently, from the workspace root:
cargo test -p gecko-sim-protocol --features export-ts
```

The generator is idempotent: re-running on unchanged Rust types produces
zero diff. CI (when wired) gates on this property.
```

- [ ] **Step 4.8: Create the workspace-root `.gitignore` additions**

Append to the existing `/Users/joseph/src/geckosim/.gitignore`:

```
apps/web/node_modules/
apps/web/.next/
apps/web/.turbo/
```

(create-next-app writes a separate `apps/web/.gitignore` with the same paths but project-relative; the workspace-root entries belt-and-suspenders any tooling that treats the root as the only ignore source.)

- [ ] **Step 4.9: Verify the scaffold builds and lints cleanly**

```bash
cd apps/web && pnpm build
```

Expected: Next.js production build completes; no type errors. The default `src/app/page.tsx` from create-next-app renders.

```bash
cd apps/web && pnpm lint
```

Expected: clean.

- [ ] **Step 4.10: Verify Vitest runs (with no test files yet)**

```bash
cd apps/web && pnpm test
```

Expected: Vitest reports "No test files found" and exits with code 1 — that's expected at this stage. Confirm Vitest itself loads without error (i.e. it's the "no tests" message, not a config error).

If exit code 1 fails the implementer's automation, `cd apps/web && pnpm test --passWithNoTests` is the alternative; but Vitest's default exit-1-on-no-tests is the contract we want once tests exist.

- [ ] **Step 4.11: Confirm commit scope**

```bash
jj st
```

Expected: a large set of files added under `apps/web/` (the entire scaffold), plus the workspace `.gitignore` modification. No Rust-side changes.

---

## Task 5: Run `ts-rs` export and commit the typed bindings

**Files:**
- Create: `apps/web/src/types/sim/AgentId.ts` and others (auto-generated)

The `apps/web/src/types/sim/` directory exists from Task 4 (containing only the README). This task populates it.

- [ ] **Step 5.1: Start the task commit**

```bash
jj new -m "Frontend: commit auto-generated typed wire bindings to apps/web/src/types/sim/"
```

- [ ] **Step 5.2: Run the export**

```bash
cargo test -p gecko-sim-protocol --features export-ts
```

Expected: every `export_bindings_<TypeName>` test runs and writes a `.ts` file under `apps/web/src/types/sim/`. Test output reports them all as passing. Expected files (one per type):

- `AgentId.ts`
- `Needs.ts`
- `Snapshot.ts`
- `AgentSnapshot.ts`
- `WireFormat.ts`
- `ServerMessage.ts`
- `ClientMessage.ts`
- `PlayerInput.ts`
- Plus all other `id_newtype!` IDs (`ObjectId`, `BuildingId`, `LeafAreaId`, `HousingId`, `EmploymentId`, `HouseholdId`, `BusinessId`, `CrimeIncidentId`, `MemoryEntryId`, `AccessoryId`, `AdvertisementId`, `ObjectTypeId`, `PromotedEventId`).

If the test fails because one of the imports inside a generated `.ts` file refers to a type whose Rust source lacks the derive (e.g., `Snapshot.ts` imports `AgentSnapshot` from `./AgentSnapshot` but `AgentSnapshot` was missed), go back to Task 2 and fix.

- [ ] **Step 5.3: Verify idempotence**

```bash
cargo test -p gecko-sim-protocol --features export-ts
jj st
```

Expected: the second run produces no diff. `jj st` shows the same set of added files as before, no modifications.

- [ ] **Step 5.4: Verify the generated TS parses**

```bash
cd apps/web && pnpm tsc --noEmit
```

Expected: clean. Imports between the generated files resolve (e.g. `AgentSnapshot.ts` imports `AgentId.ts` and `Needs.ts`; `Snapshot.ts` imports `AgentSnapshot.ts`).

- [ ] **Step 5.5: Inspect the `AgentId` rendering**

```bash
cat apps/web/src/types/sim/AgentId.ts
```

Expected output (or close to it):

```ts
// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.

export type AgentId = number;
```

If instead the file says `export type AgentId = { 0: number };`, the tuple-struct-with-single-field special case isn't kicking in for your `ts-rs` version. Fix: add `#[cfg_attr(feature = "export-ts", ts(transparent))]` to the `id_newtype!` macro alongside the existing cfg_attr lines, regenerate, recommit.

- [ ] **Step 5.6: Verify clippy + workspace tests still clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: both clean.

- [ ] **Step 5.7: Confirm commit scope**

```bash
jj st
```

Expected: ~17 new `.ts` files added under `apps/web/src/types/sim/`. No source modifications.

---

## Task 6: `SimConnectionProvider` + reducer + Vitest tests

**Files:**
- Create: `apps/web/src/lib/sim/reducer.ts`
- Create: `apps/web/src/lib/sim/reducer.test.ts`
- Create: `apps/web/src/lib/sim/connection.tsx`

- [ ] **Step 6.1: Start the task commit**

```bash
jj new -m "Frontend: SimConnectionProvider + reducer + Vitest tests"
```

- [ ] **Step 6.2: Write the failing reducer tests**

Create `apps/web/src/lib/sim/reducer.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { initialState, reduce, type SimState } from "./reducer";
import type { Snapshot } from "@/types/sim/Snapshot";

const fixtureSnapshot = (tick: number): Snapshot => ({
  tick,
  agents: [
    {
      id: 0,
      name: "Alice",
      needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
    },
  ],
});

describe("reduce", () => {
  it("hello message leaves state unchanged (still connecting)", () => {
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "hello", protocol_version: 1, format: "json" },
    });
    expect(next).toEqual(initialState);
  });

  it("init message transitions to connected and stores snapshot", () => {
    const snap = fixtureSnapshot(0);
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 0, snapshot: snap },
    });
    expect(next.status).toBe("connected");
    expect(next.snapshot).toBe(snap);
    expect(next.lastTick).toBe(0);
  });

  it("snapshot message replaces snapshot and updates lastTick", () => {
    const initSnap = fixtureSnapshot(0);
    const connected: SimState = {
      status: "connected",
      snapshot: initSnap,
      lastTick: 0,
    };
    const newSnap = fixtureSnapshot(5);
    const next = reduce(connected, {
      kind: "server-message",
      msg: { type: "snapshot", snapshot: newSnap },
    });
    expect(next.snapshot).toBe(newSnap);
    expect(next.lastTick).toBe(5);
    expect(next.status).toBe("connected");
  });

  it("ws-close transitions to disconnected, retains snapshot", () => {
    const initSnap = fixtureSnapshot(3);
    const connected: SimState = {
      status: "connected",
      snapshot: initSnap,
      lastTick: 3,
    };
    const next = reduce(connected, { kind: "ws-close" });
    expect(next.status).toBe("disconnected");
    expect(next.snapshot).toBe(initSnap);
  });

  it("ws-error transitions to disconnected", () => {
    const next = reduce(initialState, { kind: "ws-error" });
    expect(next.status).toBe("disconnected");
  });
});
```

- [ ] **Step 6.3: Run the failing test**

```bash
cd apps/web && pnpm test
```

Expected: compile error / module-not-found — `./reducer` does not exist.

- [ ] **Step 6.4: Implement `apps/web/src/lib/sim/reducer.ts`**

```ts
import type { ServerMessage } from "@/types/sim/ServerMessage";
import type { Snapshot } from "@/types/sim/Snapshot";

export type SimStatus = "connecting" | "connected" | "disconnected";

export type SimState = {
  status: SimStatus;
  snapshot: Snapshot | null;
  lastTick: number | null;
};

export const initialState: SimState = {
  status: "connecting",
  snapshot: null,
  lastTick: null,
};

export type Action =
  | { kind: "server-message"; msg: ServerMessage }
  | { kind: "ws-open" }
  | { kind: "ws-close" }
  | { kind: "ws-error" };

export function reduce(state: SimState, action: Action): SimState {
  switch (action.kind) {
    case "ws-open":
      return state.status === "disconnected" ? state : { ...state, status: "connecting" };
    case "ws-close":
    case "ws-error":
      return { ...state, status: "disconnected" };
    case "server-message":
      return reduceServerMessage(state, action.msg);
  }
}

function reduceServerMessage(state: SimState, msg: ServerMessage): SimState {
  switch (msg.type) {
    case "hello":
      // Handshake greeting; transport layer responds with ClientHello.
      // No state change — still "connecting" until Init lands.
      return state;
    case "init":
      return {
        status: "connected",
        snapshot: msg.snapshot,
        lastTick: msg.snapshot.tick,
      };
    case "snapshot":
      return {
        ...state,
        snapshot: msg.snapshot,
        lastTick: msg.snapshot.tick,
      };
  }
}
```

- [ ] **Step 6.5: Run the tests to verify pass**

```bash
cd apps/web && pnpm test
```

Expected: all 5 tests pass.

- [ ] **Step 6.6: Implement `apps/web/src/lib/sim/connection.tsx`**

```tsx
"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useReducer,
  useRef,
  type ReactNode,
} from "react";
import type { ClientMessage } from "@/types/sim/ClientMessage";
import type { PlayerInput } from "@/types/sim/PlayerInput";
import type { ServerMessage } from "@/types/sim/ServerMessage";
import { initialState, reduce, type SimState } from "./reducer";

const DEFAULT_WS_URL = "ws://127.0.0.1:9001/";
const WS_URL = process.env.NEXT_PUBLIC_SIM_WS_URL ?? DEFAULT_WS_URL;

export interface SimConnectionApi {
  state: SimState;
  sendInput: (input: PlayerInput) => void;
}

const SimConnectionContext = createContext<SimConnectionApi | null>(null);

export function useSimConnection(): SimConnectionApi {
  const ctx = useContext(SimConnectionContext);
  if (!ctx) {
    throw new Error("useSimConnection must be used inside <SimConnectionProvider>");
  }
  return ctx;
}

export function SimConnectionProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(reduce, initialState);
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    const ws = new WebSocket(WS_URL);
    wsRef.current = ws;

    ws.onopen = () => dispatch({ kind: "ws-open" });
    ws.onclose = () => dispatch({ kind: "ws-close" });
    ws.onerror = () => dispatch({ kind: "ws-error" });
    ws.onmessage = (event) => {
      let msg: ServerMessage;
      try {
        msg = JSON.parse(event.data) as ServerMessage;
      } catch {
        console.warn("dropping non-JSON frame", event.data);
        return;
      }
      dispatch({ kind: "server-message", msg });

      // Hello → reply with ClientHello.
      if (msg.type === "hello") {
        const reply: ClientMessage = { type: "client_hello", last_known_tick: null };
        ws.send(JSON.stringify(reply));
      }
    };

    return () => {
      ws.close();
      wsRef.current = null;
    };
  }, []);

  const sendInput = useCallback((input: PlayerInput) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      console.warn("sendInput called while WS not open; ignoring", input);
      return;
    }
    // ClientMessage::PlayerInput(PlayerInput) — serde flattens the inner enum's
    // tag into the outer object via #[serde(tag = "type")] on ClientMessage and
    // #[serde(tag = "kind")] on PlayerInput, so the wire shape is:
    //   { "type": "player_input", "kind": "set_speed", "multiplier": 2.0 }
    const msg = { type: "player_input" as const, ...input };
    ws.send(JSON.stringify(msg));
  }, []);

  return (
    <SimConnectionContext.Provider value={{ state, sendInput }}>
      {children}
    </SimConnectionContext.Provider>
  );
}
```

- [ ] **Step 6.7: Verify build and lint pass**

```bash
cd apps/web && pnpm tsc --noEmit
cd apps/web && pnpm lint
cd apps/web && pnpm test
```

Expected: all clean.

- [ ] **Step 6.8: Confirm commit scope**

```bash
jj st
```

Expected: 3 new files in `apps/web/src/lib/sim/`. Possibly `pnpm-lock.yaml` updated by previous steps; harmless.

---

## Task 7: `<ConnectionStatus>` + `<AgentList>` + `<Controls>` + page composition

**Files:**
- Create: `apps/web/src/components/ConnectionStatus.tsx`
- Create: `apps/web/src/components/AgentList.tsx`
- Create: `apps/web/src/components/Controls.tsx`
- Modify: `apps/web/src/app/page.tsx` (replaces create-next-app placeholder)

- [ ] **Step 7.1: Start the task commit**

```bash
jj new -m "Frontend: ConnectionStatus + AgentList + Controls components + page composition"
```

- [ ] **Step 7.2: Create `apps/web/src/components/ConnectionStatus.tsx`**

```tsx
"use client";

import { useSimConnection } from "@/lib/sim/connection";

const COLORS = {
  connecting: "bg-yellow-500",
  connected: "bg-green-500",
  disconnected: "bg-red-500",
} as const;

export function ConnectionStatus() {
  const { state } = useSimConnection();
  return (
    <span className="inline-flex items-center gap-2 text-sm">
      <span className={`h-2 w-2 rounded-full ${COLORS[state.status]}`} />
      {state.status === "connected"
        ? `tick ${state.lastTick}`
        : state.status === "connecting"
          ? "connecting…"
          : "disconnected — reload to reconnect"}
    </span>
  );
}
```

- [ ] **Step 7.3: Create `apps/web/src/components/Controls.tsx`**

```tsx
"use client";

import { useSimConnection } from "@/lib/sim/connection";

const SPEEDS = [0.5, 1, 2, 4, 8, 16, 32, 64] as const;

export function Controls() {
  const { state, sendInput } = useSimConnection();
  const disabled = state.status !== "connected";

  return (
    <section className="flex flex-wrap items-center gap-2">
      <button
        type="button"
        disabled={disabled}
        onClick={() => sendInput({ kind: "toggle_pause" })}
        className="rounded border border-neutral-400 px-3 py-1 text-sm hover:bg-neutral-100 disabled:opacity-50 dark:hover:bg-neutral-800"
      >
        Pause / Resume
      </button>
      <span className="text-sm text-neutral-500">Speed:</span>
      {SPEEDS.map((multiplier) => (
        <button
          key={multiplier}
          type="button"
          disabled={disabled}
          onClick={() => sendInput({ kind: "set_speed", multiplier })}
          className="rounded border border-neutral-400 px-3 py-1 text-sm hover:bg-neutral-100 disabled:opacity-50 dark:hover:bg-neutral-800"
        >
          {multiplier}×
        </button>
      ))}
    </section>
  );
}
```

- [ ] **Step 7.4: Create `apps/web/src/components/AgentList.tsx`**

```tsx
"use client";

import { useSimConnection } from "@/lib/sim/connection";

const NEED_KEYS = ["hunger", "sleep", "social", "hygiene", "fun", "comfort"] as const;

export function AgentList() {
  const { state } = useSimConnection();
  const snapshot = state.snapshot;

  if (!snapshot) {
    return <p className="text-sm text-neutral-500">No data yet.</p>;
  }
  if (snapshot.agents.length === 0) {
    return <p className="text-sm text-neutral-500">No agents.</p>;
  }

  return (
    <table className="w-full border-collapse text-sm">
      <thead>
        <tr className="border-b border-neutral-300 text-left dark:border-neutral-700">
          <th className="px-2 py-1">ID</th>
          <th className="px-2 py-1">Name</th>
          {NEED_KEYS.map((k) => (
            <th key={k} className="px-2 py-1 capitalize">
              {k}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {snapshot.agents.map((agent) => (
          <tr
            key={agent.id}
            className="border-b border-neutral-200 last:border-0 dark:border-neutral-800"
          >
            <td className="px-2 py-1 font-mono">{agent.id}</td>
            <td className="px-2 py-1">{agent.name}</td>
            {NEED_KEYS.map((k) => (
              <td key={k} className="px-2 py-1 font-mono">
                {agent.needs[k].toFixed(2)}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
```

- [ ] **Step 7.5: Replace `apps/web/src/app/page.tsx`**

Overwrite create-next-app's placeholder with:

```tsx
"use client";

import { AgentList } from "@/components/AgentList";
import { ConnectionStatus } from "@/components/ConnectionStatus";
import { Controls } from "@/components/Controls";
import { SimConnectionProvider } from "@/lib/sim/connection";

export default function Page() {
  return (
    <SimConnectionProvider>
      <main className="mx-auto max-w-4xl space-y-4 p-6">
        <header className="flex items-center justify-between">
          <h1 className="text-xl font-semibold">gecko-sim</h1>
          <ConnectionStatus />
        </header>
        <Controls />
        <AgentList />
      </main>
    </SimConnectionProvider>
  );
}
```

- [ ] **Step 7.6: Run frontend gates**

```bash
cd apps/web && pnpm tsc --noEmit
cd apps/web && pnpm lint
cd apps/web && pnpm test
cd apps/web && pnpm build
```

Expected: all clean. `pnpm build` outputs the production bundle and reports the route `/` as a static page (or dynamic, depending on Next.js's analysis — either is fine).

- [ ] **Step 7.7: Manual end-to-end smoke**

In one terminal:

```bash
cargo run -p gecko-sim-host
```

In another:

```bash
cd apps/web && pnpm dev
```

Open http://localhost:3000/ in a browser. Verify:

1. Status badge briefly shows "connecting…", then "connected — tick N" with N incrementing about once per second at default speed.
2. Agent list shows three rows (Alice / Bob / Charlie) with all six need columns at decimal values that decrement over time.
3. Click `Pause / Resume`. The tick number stops advancing. Click again — resumes.
4. Click `4×`. The tick number visibly accelerates.
5. Click `0.5×`. Slows down.
6. Stop the host (Ctrl-C in the cargo terminal). Within ~1 second the status badge flips to "disconnected — reload to reconnect".
7. Reload the browser tab (after restarting the host). Reconnects cleanly.

Stop the dev server and the host once verified.

- [ ] **Step 7.8: Confirm commit scope**

```bash
jj st
```

Expected: 3 new component files; `apps/web/src/app/page.tsx` modified.

---

## Task 8: README + final polish

**Files:**
- Modify: `apps/web/README.md` (replaces what create-next-app wrote)
- Modify: `.gitignore` (workspace) — already covered in Task 4; this task is a final-polish gate

- [ ] **Step 8.1: Start the task commit**

```bash
jj new -m "Frontend: README + scripts + final polish"
```

- [ ] **Step 8.2: Replace `apps/web/README.md`**

create-next-app's auto-written README is generic. Replace with a project-specific one:

```markdown
# apps/web — gecko-sim frontend

Next.js 15 (App Router) client for the gecko-sim Rust host. Connects via
WebSocket to `127.0.0.1:9001`, renders the live agent list, and exposes
speed / pause controls. See ADR 0013 ("Frontend data flow and transport")
for the wire contract.

## Develop

In one terminal, run the host:

```bash
cargo run -p gecko-sim-host
```

In another:

```bash
cd apps/web
pnpm install   # first time only
pnpm dev       # opens http://localhost:3000
```

Override the WebSocket URL via `NEXT_PUBLIC_SIM_WS_URL` (default
`ws://127.0.0.1:9001/`).

## Test

```bash
pnpm test       # Vitest run, exits when done
pnpm test:watch # Vitest watch mode
pnpm tsc --noEmit
pnpm lint
pnpm build
```

## Regenerating typed wire bindings

The TypeScript types under `src/types/sim/` are auto-generated from Rust
via `ts-rs`. Regenerate after changing wire types:

```bash
pnpm gen-types
# equivalent to:
#   cargo test -p gecko-sim-protocol --features export-ts
```

The generator is idempotent; CI gates on the generated files matching
what's committed.

## Architecture

- `src/lib/sim/connection.tsx` — `<SimConnectionProvider>` owns the
  WebSocket lifecycle, exposes `useSimConnection()`.
- `src/lib/sim/reducer.ts` — pure `(state, ServerMessage) → state`.
- `src/components/{ConnectionStatus,AgentList,Controls}.tsx` —
  presentation components that subscribe via `useSimConnection()`.
- `src/types/sim/*.ts` — generated wire types. Do not edit.

## Out of scope at v0

- Three.js / 3D rendering — a future "rendering" pass once `Snapshot`
  carries position data.
- Auto-reconnect — close → "reload to reconnect" banner; reload to retry.
- Catalog / save / load / inspection UI — future passes per ADR 0013.
```

- [ ] **Step 8.3: Verify workspace `.gitignore` already has the right entries**

```bash
cat .gitignore
```

Expected: includes `apps/web/node_modules/`, `apps/web/.next/`, `apps/web/.turbo/` from Task 4. If any are missing, add them.

- [ ] **Step 8.4: Final workspace verification**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p gecko-sim-protocol --features export-ts  # idempotent — should produce zero diff
jj st  # confirm zero diff after re-running the export
```

Expected: all green; `jj st` shows the working copy still as the polish commit with only the README edit (the export run produces no diff).

- [ ] **Step 8.5: Final frontend verification**

```bash
cd apps/web && pnpm install --frozen-lockfile
cd apps/web && pnpm test
cd apps/web && pnpm tsc --noEmit
cd apps/web && pnpm lint
cd apps/web && pnpm build
```

Expected: all clean.

- [ ] **Step 8.6: Final commit log**

```bash
jj log -r 'ancestors(@, 9)' --no-graph -T 'change_id.short(8) ++ " " ++ description.first_line() ++ "\n"'
```

Expected: 8 task commits + the spec commit, in order:

```
<task-8>   Frontend: README + scripts + final polish
<task-7>   Frontend: ConnectionStatus + AgentList + Controls components + page composition
<task-6>   Frontend: SimConnectionProvider + reducer + Vitest tests
<task-5>   Frontend: commit auto-generated typed wire bindings to apps/web/src/types/sim/
<task-4>   Frontend: scaffold Next.js app under apps/web/ (App Router, TS, Tailwind v4, Vitest)
<task-3>   Frontend: ts-rs derives on protocol wire types
<task-2>   Frontend: ts-rs derives on core wire types
<task-1>   Frontend: ts-rs workspace dep + export-ts feature on core/protocol
ywpuxvsp   Frontend scaffold v0 spec: …
```

---

## Definition of done (rolled-up gate)

After Task 8 lands:

- `cargo build --workspace` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean (default features).
- `cargo clippy -p gecko-sim-protocol --all-targets --features export-ts -- -D warnings` clean.
- `cargo test --workspace` — all existing tests pass.
- `cargo test -p gecko-sim-protocol --features export-ts` — idempotent regeneration; running twice produces zero diff.
- `cd apps/web && pnpm install --frozen-lockfile && pnpm test && pnpm tsc --noEmit && pnpm lint && pnpm build` — all clean.
- Manual end-to-end smoke (host + dev server + browser): connection badge cycles connecting → connected → disconnected; agent list updates at 30 Hz; Pause / SetSpeed buttons control the tick rate observably; reload reconnects cleanly.
- 8 atomic jj commits matching the commit-strategy section of the spec; the spec commit (`ywpuxvsp`) sits below them.

## Notes for the implementer

- **Do NOT run `cargo test --features export-ts` in Tasks 1–3.** The export tries to write to `apps/web/src/types/sim/` which doesn't exist until Task 4. Tasks 1–3 verify with `cargo build` only.
- **Tailwind v4 has no separate `tailwind.config.ts` file.** create-next-app may still write one for v3-compat — if so, leave it alone (it's harmless). Tailwind v4 reads CSS-first config from the `@import "tailwindcss";` line in `globals.css`.
- **`pnpm create next-app` prompts on first run with telemetry/analytics.** Pass `--yes` (or set `NEXT_TELEMETRY_DISABLED=1`) to silence. The `--yes` flag accepts all defaults that aren't covered by explicit flags.
- **The wire-shape of `ClientMessage::PlayerInput(PlayerInput)` is non-obvious.** Serde flattens the inner enum's tag into the outer object — verified by `crates/protocol/tests/roundtrip.rs`. The `sendInput` helper mirrors this with `{ type: "player_input", ...input }`.
- **`AgentSnapshot.id` is `AgentId` in Rust** (a `u64` newtype). With ts-rs's transparent rendering, it shows up in the generated `AgentSnapshot.ts` as `id: AgentId` where `AgentId = number`. No serialization wrapping; an agent ID `0` is wire-encoded as JSON number `0`.
