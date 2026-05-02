# gecko-sim

## Local Dev

Run the Rust host and the Next.js frontend in separate terminals:

```sh
cargo run -p gecko-sim-host
```

```sh
cd apps/web
pnpm install # first time only
pnpm dev
```

Open `http://localhost:3000`. The frontend connects to the host at
`ws://127.0.0.1:9001/` by default; override it with `NEXT_PUBLIC_SIM_WS_URL`
if needed.
