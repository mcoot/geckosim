"use client";

import { AgentList } from "@/components/AgentList";
import { ConnectionStatus } from "@/components/ConnectionStatus";
import { Controls } from "@/components/Controls";
import { WorldScene } from "@/components/WorldScene";
import { SimConnectionProvider, useSimConnection } from "@/lib/sim/connection";

function Dashboard() {
  const { state } = useSimConnection();

  return (
    <main className="mx-auto box-border flex min-w-0 w-full max-w-6xl flex-1 flex-col gap-4 p-6">
      <header className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">gecko-sim</h1>
        <ConnectionStatus />
      </header>
      <Controls />
      <WorldScene world={state.world} snapshot={state.snapshot} />
      <div
        className="min-w-0 overflow-x-auto"
        style={{ width: "calc(100vw - 3rem)", maxWidth: "100%" }}
      >
        <AgentList />
      </div>
    </main>
  );
}

export default function Page() {
  return (
    <SimConnectionProvider>
      <Dashboard />
    </SimConnectionProvider>
  );
}
