# apps/web — gecko-sim frontend

Next.js 16 (App Router) client for the gecko-sim Rust host. Connects via
WebSocket to `127.0.0.1:9001`, renders the live world scene and agent list,
and exposes speed / pause controls. See ADR 0013 ("Frontend data flow and
transport") for the wire contract.

## Develop

In one terminal, run the host:

```
cargo run -p gecko-sim-host
```

In another:

```
cd apps/web
pnpm install   # first time only
pnpm dev       # opens http://localhost:3000
```

Override the WebSocket URL via `NEXT_PUBLIC_SIM_WS_URL` (default
`ws://127.0.0.1:9001/`).

## Test

```
pnpm test       # Vitest run, exits when done
pnpm test:watch # Vitest watch mode
pnpm tsc --noEmit
pnpm lint
pnpm build
```

## Regenerating typed wire bindings

The TypeScript types under `src/types/sim/` are auto-generated from Rust
via `ts-rs`. Regenerate after changing wire types:

```
pnpm gen-types
```

(equivalent to `cargo test -p gecko-sim-core --features export-ts && cargo test -p gecko-sim-protocol --features export-ts` from the workspace root)

The generator is idempotent; CI gates on the generated files matching
what's committed.

## Architecture

- `src/lib/sim/connection.tsx` — `<SimConnectionProvider>` owns the
  WebSocket lifecycle, exposes `useSimConnection()`.
- `src/lib/sim/reducer.ts` — pure `(state, ServerMessage) -> state`.
- `src/components/{ConnectionStatus,AgentList,Controls,WorldScene}.tsx` —
  presentation components that subscribe via `useSimConnection()`.
- `src/lib/world-scene/` — pure render-model projection, Three.js scene graph
  adapter, and browser runtime for the world scene.
- `src/types/sim/*.ts` — generated wire types. Do not edit.

## Out of scope at v0

- Full free-camera UX, interpolation between snapshots, authored meshes /
  textures, and click-to-inspect panels.
- Auto-reconnect — close shows a "reload to reconnect" banner.
- Catalog / save / load / inspection UI — future passes per ADR 0013.
