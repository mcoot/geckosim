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
