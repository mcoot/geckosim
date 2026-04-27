# Frontend scaffold v0 — Next.js client + ts-rs typed wire + WS connection + agent list + speed/pause controls

- **Date:** 2026-04-27
- **Status:** Draft
- **Scope:** Fifth implementation pass. Stands up the Next.js / React frontend under `apps/web/`, wires `ts-rs` into the Rust workspace so wire types auto-generate to TypeScript, and ships a connected WebSocket client that renders the live agent list and exposes speed / pause controls.
- **Predecessors:**
  - [`2026-04-27-ws-transport-v0-design.md`](2026-04-27-ws-transport-v0-design.md) — host serves `Hello` → `ClientHello` → `Init` → 30 Hz `Snapshot` flow over JSON-over-WS at `127.0.0.1:9001`. `ServerMessage`/`ClientMessage`/`PlayerInput` already implemented in `crates/protocol`.
  - [`2026-04-27-ron-content-loading-design.md`](2026-04-27-ron-content-loading-design.md) — host loads RON catalogs at startup. The frontend doesn't surface catalog content yet, but the host now starts cleanly with seed content.

## Goal

End state: `pnpm --filter web dev` (or equivalent) inside `apps/web/` boots a Next.js dev server. Opening the page in a browser:

1. Establishes a WebSocket to the running host (`127.0.0.1:9001`).
2. Receives `Hello`, sends `ClientHello { last_known_tick: null }`.
3. Receives `Init` and renders an agent list with three rows (Alice, Bob, Charlie) showing each agent's six need values.
4. Updates the agent list at the host's 30 Hz sample rate as `Snapshot` frames arrive.
5. Exposes seven speed buttons (`0.5×`, `1×`, `2×`, `4×`, `8×`, `16×`, `32×`, `64×`) and a Pause toggle. Clicking any button sends the corresponding `PlayerInput` and the effect is observable in subsequent snapshots' `tick` field within ~200 ms.
6. Shows a connection-status badge (Connecting / Connected / Disconnected). On WS close, the badge flips to "Disconnected — reload to reconnect" and snapshot updates stop. No auto-reconnect.

`ts-rs` derives on `crates/core` and `crates/protocol` wire types; running `cargo test -p gecko-sim-protocol --features export-ts` regenerates the typed `.ts` files at `apps/web/src/types/sim/`. These are committed to git; an upstream check (CI later) keeps them in sync.

This is the slice tagged "frontend scaffold pass" in the WS v0 follow-up list. No Three.js / 3D rendering — the agent list is a plain HTML table. Three.js arrives in a later pass once `Snapshot` carries positional data.

## Non-goals (deferred — see "Deferred items" section)

- **No Three.js / react-three-fiber.** Agents render as table rows with text labels, not as 3D meshes. ADR 0002's stack pin still holds; the 3D pass lands when sim state grows positional data.
- **No catalog UI.** `ObjectType`s and `Accessory`s loaded by the host (per the RON content pass) are not surfaced anywhere in the client yet. They land in `Init` extensions when the spawning pass introduces real instances.
- **No auto-reconnect.** WS close → "reload to reconnect" banner. ADR 0013 explicitly defers smart resync; auto-reconnect loops are a footgun until then.
- **No `Delta` / `PromotedEvent` handling.** Server still streams full `Snapshot` only; the client mirrors that.
- **No save/load UI.** No `RequestSave` / `RequestLoad` / `RequestRestart` — those `PlayerInput` variants don't exist yet.
- **No multi-page navigation.** One route (`/`) renders the entire client.
- **No mobile / responsive polish, no accessibility audit, no theme toggle.** Tailwind defaults; readable on a desktop browser is the bar.
- **No Playwright / browser-level e2e.** Vitest unit tests on the pure reducer + manual browser smoke is the test gate.
- **No CI workflow.** This pass adds no GitHub Actions config; the test commands are run locally. CI lands when the first PR opens (probably as part of "finishing-a-development-branch").
- **No production build deployment.** `pnpm dev` is the only intended command; `pnpm build` should succeed (Next.js validates on build) but no static export, no Docker, no hosting.
- **No backend re-export of catalog over the wire.** ADR 0013 anticipates `Init` carrying the object/accessory catalogs; that wires up when the frontend first needs them.

## Architecture

### Workspace layout

```
geckosim/
├── crates/                       (Rust workspace — unchanged)
├── content/                      (RON seed catalog — unchanged)
└── apps/
    └── web/                      Next.js app — this pass
        ├── package.json
        ├── pnpm-lock.yaml
        ├── tsconfig.json
        ├── next.config.ts
        ├── eslint.config.mjs
        ├── postcss.config.mjs    (loads @tailwindcss/postcss)
        ├── vitest.config.ts
        ├── public/
        └── src/
            ├── app/
            │   ├── layout.tsx
            │   ├── page.tsx
            │   └── globals.css
            ├── components/
            │   ├── AgentList.tsx
            │   ├── ConnectionStatus.tsx
            │   └── Controls.tsx
            ├── lib/sim/
            │   ├── connection.tsx     (Context provider + useSimConnection hook)
            │   ├── reducer.ts         (pure (state, ServerMessage) → state)
            │   └── reducer.test.ts    (Vitest)
            └── types/sim/             (ts-rs auto-generated; committed)
```

`apps/web/` is a self-contained pnpm project. Cargo and Node toolchains stay independent — `apps/web/` is **not** part of the Cargo workspace. The only crossing point is `apps/web/src/types/sim/`, populated by the Rust-side ts-rs export.

### `ts-rs` pipeline

#### Workspace `Cargo.toml`

Add to `[workspace.dependencies]`:

```toml
ts-rs = { version = "10", features = ["serde-compat"] }
```

(Implementation pass picks the latest 10.x patch on first run; subsequent passes track major-version bumps if any ergonomics change matters.)

#### `crates/core/Cargo.toml`

```toml
[dependencies]
# ... existing ...
ts-rs = { workspace = true, optional = true }

[features]
export-ts = ["dep:ts-rs"]
```

#### `crates/protocol/Cargo.toml`

```toml
[dependencies]
# ... existing ...
ts-rs = { workspace = true, optional = true }

[features]
export-ts = ["dep:ts-rs", "gecko-sim-core/export-ts"]
```

The `protocol` feature transitively enables `core`'s feature so a single command (`cargo test -p gecko-sim-protocol --features export-ts`) emits all relevant types.

#### Derives on wire types

The following types gain TS derives, all behind `#[cfg_attr(feature = "export-ts", ...)]`:

**In `crates/core`:**

- `AgentId` (in `ids.rs`) — `#[ts(export, transparent)]` so it renders as `type AgentId = number` rather than `{ 0: number }`.
- `Needs` (in `agent/mod.rs`)
- `Snapshot` and `AgentSnapshot` (in `snapshot.rs`)

The macro for `id_newtype!` in `ids.rs` grows a transparent TS derive guard so future ID newtypes follow the same shape.

**In `crates/protocol`:**

- `WireFormat`, `ServerMessage`, `ClientMessage`, `PlayerInput`, plus the `PROTOCOL_VERSION` constant exported as a generated `.ts` file via `ts-rs` rename rules.

All derives use `#[ts(export, export_to = "../../apps/web/src/types/sim/")]`. The path is workspace-relative (resolved from `crates/<crate>/`); `ts-rs` writes one `.ts` file per type.

#### Generation command

```bash
cargo test -p gecko-sim-protocol --features export-ts
```

`ts-rs`'s derive macro auto-injects `#[test]` functions named `export_bindings_<TypeName>` per type; running `cargo test` with the feature on triggers them. They're idempotent — re-running produces byte-identical files.

For frontend convenience, `apps/web/package.json` exposes a script:

```json
{
  "scripts": {
    "gen-types": "cd ../.. && cargo test -p gecko-sim-protocol --features export-ts"
  }
}
```

Frontend devs without Rust toolchain don't need to run this — types are committed.

#### Generated file convention

Each emitted `.ts` file is **read-only** from the frontend's perspective. They live under `apps/web/src/types/sim/` and are imported via:

```ts
import type { Snapshot } from "@/types/sim/Snapshot";
import type { ServerMessage } from "@/types/sim/ServerMessage";
```

A `apps/web/src/types/sim/README.md` notes that the directory is auto-generated. Edits are clobbered by the next `pnpm gen-types`.

### WS connection lifecycle (frontend)

`SimConnectionProvider` (in `lib/sim/connection.tsx`) owns the WebSocket and the reducer state. Mount lifecycle:

1. On mount, `new WebSocket("ws://127.0.0.1:9001/")`. State: `{ status: "connecting", snapshot: null, lastTick: null }`.
2. On `onmessage(Hello)`, send `ClientMessage::ClientHello { last_known_tick: null }`. State stays `connecting`.
3. On `onmessage(Init)`, store the snapshot. State: `{ status: "connected", snapshot, lastTick: snapshot.tick }`.
4. On `onmessage(Snapshot)`, replace the snapshot. Update `lastTick`.
5. On `onclose` or `onerror`, state: `{ status: "disconnected", snapshot: <last>, lastTick: <last> }`. Show banner.
6. On unmount, close the socket.

The reducer in `reducer.ts` is a pure function `reduce(state, ServerMessage) → state` so message handling is testable. The provider's `useEffect` wires it up:

```tsx
const [state, dispatch] = useReducer(reduce, initialState);
useEffect(() => {
  const ws = new WebSocket(WS_URL);
  ws.onmessage = (e) => dispatch({ kind: "server-message", msg: JSON.parse(e.data) });
  ws.onopen = () => dispatch({ kind: "ws-open" });
  ws.onclose = () => dispatch({ kind: "ws-close" });
  ws.onerror = () => dispatch({ kind: "ws-error" });
  return () => ws.close();
}, []);
```

The `send` function for outbound `PlayerInput` is a callback that wraps the `ws.send(JSON.stringify(...))` call; it lives in the same provider and is exposed via context.

### Context shape

```ts
interface SimConnectionState {
  status: "connecting" | "connected" | "disconnected";
  snapshot: Snapshot | null;          // typed via ts-rs
  lastTick: number | null;
}

interface SimConnectionApi {
  state: SimConnectionState;
  sendInput: (input: PlayerInput) => void;
}

const SimConnectionContext = createContext<SimConnectionApi | null>(null);

export function useSimConnection(): SimConnectionApi {
  const ctx = useContext(SimConnectionContext);
  if (!ctx) throw new Error("useSimConnection must be inside SimConnectionProvider");
  return ctx;
}
```

`sendInput` no-ops when `status !== "connected"` (with a `console.warn`); a richer queue is deferred.

### Page layout

`src/app/page.tsx` is the single route. Composition:

```tsx
"use client";
import { SimConnectionProvider } from "@/lib/sim/connection";
import { ConnectionStatus } from "@/components/ConnectionStatus";
import { Controls } from "@/components/Controls";
import { AgentList } from "@/components/AgentList";

export default function Page() {
  return (
    <SimConnectionProvider>
      <main className="p-6 max-w-4xl mx-auto space-y-4">
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

(`"use client"` because of WebSocket / hooks.)

### Components

#### `ConnectionStatus`

```tsx
const COLORS = { connecting: "bg-yellow-500", connected: "bg-green-500", disconnected: "bg-red-500" } as const;

export function ConnectionStatus() {
  const { state } = useSimConnection();
  return (
    <span className={`inline-flex items-center gap-2 text-sm`}>
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

#### `Controls`

Eight speed buttons (0.5×, 1×, 2×, 4×, 8×, 16×, 32×, 64×) and a `Pause` toggle. Each button sends `PlayerInput::SetSpeed { multiplier }`; the toggle sends `PlayerInput::TogglePause`. Disabled when `status !== "connected"`.

```tsx
const SPEEDS = [0.5, 1, 2, 4, 8, 16, 32, 64] as const;
```

The active speed is unknowable from the wire (the host doesn't echo it back at v0), so buttons are stateless click-to-send. A future "Init carries current speed" payload extension can light up the active button.

#### `AgentList`

A plain HTML `<table>` with columns: ID, Name, Hunger, Sleep, Social, Hygiene, Fun, Comfort. Each numeric value formatted to 2 decimal places. Renders in `Snapshot.agents` order (already deterministic by `AgentId` from the host).

When `state.snapshot` is `null`, render "No data yet" placeholder.

### Reducer (testable core)

```ts
// reducer.ts
import type { ServerMessage } from "@/types/sim/ServerMessage";
import type { Snapshot } from "@/types/sim/Snapshot";

export type SimState = {
  status: "connecting" | "connected" | "disconnected";
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
  // ts-rs renders ServerMessage as a tagged union with `type` discriminant.
  switch (msg.type) {
    case "hello":
      // Handshake greeting; transport layer responds with ClientHello.
      // No state change — still "connecting" until Init lands.
      return state;
    case "init":
      return { status: "connected", snapshot: msg.snapshot, lastTick: msg.snapshot.tick };
    case "snapshot":
      return { ...state, snapshot: msg.snapshot, lastTick: msg.snapshot.tick };
  }
}
```

`reduceServerMessage` is the chunk that Vitest tests pin. Three test cases cover the three `ServerMessage` variants; one tests the `ws-close` transition; one tests the pause-style scenario where `Snapshot` arrives in `connected` state.

### Outbound input

`sendInput` serializes the `PlayerInput` and writes it on the socket. The wire shape (already locked in `protocol::messages`) is:

```json
{ "type": "player_input", "kind": "set_speed", "multiplier": 2.0 }
{ "type": "player_input", "kind": "toggle_pause" }
```

The frontend builds these by hand (since `PlayerInput` is a flat `{ kind, ... }` shape after ts-rs export). Wrapping in the outer `ClientMessage::PlayerInput(_)` envelope is done by the helper:

```ts
function sendInput(ws: WebSocket, input: PlayerInput) {
  const msg: ClientMessage = { type: "player_input", ...input };
  ws.send(JSON.stringify(msg));
}
```

(There's a slight wire-shape subtlety: `ClientMessage::PlayerInput(PlayerInput)` is a tuple variant in Rust, but with `#[serde(tag = "type", rename_all = "snake_case")]` and `PlayerInput` itself tagged with `#[serde(tag = "kind", ...)]`, serde flattens the inner enum's tag into the outer object. The generated TS will reflect that flattened shape — the helper's spread does the same on the way out.)

### Connection target

Hard-coded `ws://127.0.0.1:9001/` in `lib/sim/connection.tsx` for v0. Override via env var `NEXT_PUBLIC_SIM_WS_URL` if set:

```ts
const WS_URL = process.env.NEXT_PUBLIC_SIM_WS_URL ?? "ws://127.0.0.1:9001/";
```

Lets a deployed setup point at a non-loopback host without code edits. Default keeps `pnpm dev` zero-config.

### Tailwind + globals

Tailwind v4 (CSS-first config):

```css
/* src/app/globals.css */
@import "tailwindcss";
```

Tailwind v4 doesn't require a separate `tailwind.config.ts` — content sources auto-detect from the importing CSS file's project. `postcss.config.mjs` includes `@tailwindcss/postcss` per the v4 setup.

### Vitest setup

`vitest.config.ts`:

```ts
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
  },
  resolve: {
    alias: { "@": "/src" },
  },
});
```

Tests live next to the unit (`reducer.test.ts` next to `reducer.ts`). One `package.json` script: `"test": "vitest run"`.

## Module changes by crate / directory

### `gecko-sim-core`

- **Modified:** `Cargo.toml` adds optional `ts-rs` dep + `export-ts` feature. `src/ids.rs`'s `id_newtype!` macro grows a `cfg_attr(feature = "export-ts")` block adding the `TS` derive with `#[ts(transparent, export, export_to = "../../apps/web/src/types/sim/")]`. `src/agent/mod.rs` (Needs), `src/snapshot.rs` (Snapshot, AgentSnapshot) gain the same `cfg_attr` derives.
- **Untouched:** all behavior code.

### `gecko-sim-protocol`

- **Modified:** `Cargo.toml` adds optional `ts-rs` dep + `export-ts` feature that transitively enables `gecko-sim-core/export-ts`. `src/messages.rs` gains TS derives on `WireFormat`, `ServerMessage`, `ClientMessage`, `PlayerInput`. The `PROTOCOL_VERSION: u32` constant is exported as a `.ts` file via a small standalone `#[cfg(feature = "export-ts")] #[test]` that writes `PROTOCOL_VERSION.ts` (since `ts-rs` doesn't auto-export bare `const`s — minor manual handling).
- **Existing tests:** `tests/roundtrip.rs` continues to pass; ts-rs feature off by default so the JSON roundtrip suite is unaffected.

### Workspace `Cargo.toml`

- **Modified:** add `ts-rs = { version = "10", features = ["serde-compat"] }` to `[workspace.dependencies]`.

### `apps/web/`

- **Removed:** `apps/web/README.md` (replaced).
- **New:** the entire Next.js project — `package.json`, `pnpm-lock.yaml`, `tsconfig.json`, `next.config.ts`, `eslint.config.mjs`, `postcss.config.mjs`, `tailwind.config.ts`, `vitest.config.ts`, `public/`, `src/app/{layout,page}.tsx`, `src/app/globals.css`, `src/components/{AgentList,ConnectionStatus,Controls}.tsx`, `src/lib/sim/{connection,reducer}.tsx/.ts`, `src/lib/sim/reducer.test.ts`, `src/types/sim/*.ts` (auto-generated), `src/types/sim/README.md`. A new `apps/web/README.md` describes the dev / build / test commands.

### `.gitignore`

- **Modified:** add `apps/web/.next/`, `apps/web/node_modules/`, `apps/web/.turbo/` (defensive — Next.js sometimes spawns it). The generated `apps/web/src/types/sim/*.ts` files are **committed**, so no ignore there.

## Tests

### Rust side

- **`cargo test --workspace`** continues to pass. ts-rs derives are feature-gated, so default-feature builds are unchanged.
- **`cargo test -p gecko-sim-protocol --features export-ts`** runs the auto-generated export tests and writes the `.ts` files to `apps/web/src/types/sim/`. Re-running is idempotent.
- **`cargo clippy --workspace --all-targets -- -D warnings`** continues to pass for both default-features and `--features export-ts` configurations.

### Frontend side

- **`pnpm --filter web test`** (or `cd apps/web && pnpm test`) runs Vitest:
  - `reduce(initialState, { kind: "server-message", msg: { type: "hello", ... } }) === initialState` — Hello is informational; no state change.
  - `reduce(initialState, { kind: "server-message", msg: { type: "init", current_tick: 0, snapshot: <fixture> } })` → `{ status: "connected", snapshot: <fixture>, lastTick: 0 }`.
  - `reduce(connectedState, { kind: "server-message", msg: { type: "snapshot", snapshot: <fixture-tick-5> } })` → snapshot replaced; `lastTick: 5`.
  - `reduce(connectedState, { kind: "ws-close" })` → `{ ...state, status: "disconnected" }` (snapshot retained).
  - `reduce(connectingState, { kind: "ws-error" })` → `disconnected`.
- **`pnpm --filter web build`** runs `next build` and exits clean. Catches type errors and any router misconfiguration. Run as part of the verification gate but not in CI yet.

### Manual smoke

After the implementation pass:

1. Start the host: `cargo run -p gecko-sim-host` (in one terminal).
2. Start the frontend: `cd apps/web && pnpm dev` (in another).
3. Open `http://localhost:3000` in a browser.
4. Confirm: status badge shows "connecting" briefly, then "connected — tick N" with N incrementing.
5. Confirm: agent list shows three rows (Alice, Bob, Charlie) with needs values that decrement over time as `needs::decay` fires.
6. Click `Pause` — tick stops advancing. Click again — resumes.
7. Click `4×` — tick rate visibly accelerates.
8. Stop the host (`Ctrl-C` in the cargo terminal) — frontend badge flips to "disconnected — reload to reconnect" within ~1 second.

## Definition of done

- `cargo build --workspace` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace` clean.
- `cargo test -p gecko-sim-protocol --features export-ts` regenerates the typed `.ts` files; the regenerated files match what's committed (i.e., re-running produces zero diff).
- `cd apps/web && pnpm install && pnpm test && pnpm build` all clean.
- Manual smoke (above) passes end-to-end.
- One topic chain of jj commits, ordered per the commit strategy below.

## Commit strategy

Eight commits, each independently green:

1. `Frontend: ts-rs workspace dep + export-ts feature on core/protocol`
2. `Frontend: ts-rs derives on core wire types (AgentId, Needs, Snapshot, AgentSnapshot)`
3. `Frontend: ts-rs derives on protocol wire types (Hello, Init, Snapshot, ClientHello, PlayerInput, WireFormat)`
4. `Frontend: scaffold Next.js app under apps/web/ (App Router, TS, Tailwind, ESLint, Vitest)`
5. `Frontend: commit auto-generated typed wire bindings to apps/web/src/types/sim/`
6. `Frontend: SimConnectionProvider + reducer + Vitest tests`
7. `Frontend: ConnectionStatus + AgentList + Controls components + page composition`
8. `Frontend: README + .gitignore entries + pnpm scripts`

Plan-author may merge commits 2 and 3 if the diffs are small; commits 4 and 5 must stay separate so the typed-bindings commit is a clean checkpoint of "wire types match what Rust thinks they are".

## Trace to ADRs

- **ADR 0002 (tech stack):** Next.js + React + Three.js. This pass lands Next.js + React; Three.js explicitly deferred to its own pass (per "Non-goals").
- **ADR 0012 (architecture):** `protocol` crate emits TS bindings via `ts-rs`. The `core` types touched are wire types per ADR 0011 — adding `ts-rs` derives behind a feature does not violate the "core has zero I/O dependencies" rule (ts-rs is a build-time tool, off by default, no runtime presence).
- **ADR 0013 (transport):** the `Hello` → `ClientHello` → `Init` → `Snapshot` lifecycle is exactly as specified. JSON encoding via `WireFormat::Json`. 30 Hz sample stream consumed without modification. `last_known_tick: null` per the "fresh Init on every reconnect" v0 rule.

## Deferred items (carry forward to later passes)

| Item | Triggers landing | Lives in |
|---|---|---|
| Three.js / react-three-fiber rendering | When `Snapshot` carries positional data | `apps/web/src/components/Scene.tsx` (new) + dep additions |
| Catalog UI (smart objects, accessories) | Init payload extension | `apps/web/src/components/Catalog.tsx` (new) |
| `Delta` and `PromotedEvent` stream handling | When the wire grows those messages | `apps/web/src/lib/sim/reducer.ts` |
| Save / load UI | RequestSave/Load PlayerInput variants | New `apps/web/src/components/SaveControls.tsx` |
| Auto-reconnect with smart resync | Delta cache lands server-side | `apps/web/src/lib/sim/connection.tsx` |
| Inspection UI (click agent → see state) | Inspection PlayerInput variant | `apps/web/src/components/InspectionPanel.tsx` |
| MessagePack / postcard wire formats | Bandwidth pressure | `WireFormat::MessagePack` in protocol; client-side decoder |
| CI workflow (lint + test + ts-rs sync check) | First PR | `.github/workflows/` |
| Production build deployment | Multi-machine demo | Hosting decision |
| Mobile / responsive polish | Real users | Tailwind config + component tweaks |
| Playwright e2e | When manual smoke gets onerous | `apps/web/tests/e2e/` |
| Active-speed echo from server | When users want a "current speed" indicator | Init payload extension |
| Tick-rate display, sim-time clock, FPS HUD | Polish pass | Components |
| Renderer interpolation between snapshots | Three.js pass | `apps/web/src/lib/sim/interpolation.ts` |

## What this pass enables next

Three independent next passes are unblocked:

1. **Second sim system pass (`mood`).** Snapshot grows a `mood` field; the wire shape evolves; ts-rs regenerates; the agent list grows columns. Establishes the "extending the wire" workflow.
2. **Three.js / 3D scene pass.** Once `Snapshot` carries positions (system: `pathing`/`movement`), the 3D rendering pass replaces / supplements the table view.
3. **Inspection pass.** First sim-bound `PlayerInput` (NudgeAgent or Inspect) — wires up the round-trip from the client to the sim's apply_input path that ADR 0012 anticipates.
