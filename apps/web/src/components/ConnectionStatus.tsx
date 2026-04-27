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
