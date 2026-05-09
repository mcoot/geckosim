"use client";

import { useMemo, useState } from "react";
import { AgentInspector } from "@/components/AgentInspector";
import { ConnectionStatus } from "@/components/ConnectionStatus";
import { Controls } from "@/components/Controls";
import { WorldScene } from "@/components/WorldScene";
import { SimConnectionProvider, useSimConnection } from "@/lib/sim/connection";

export function Dashboard() {
  const { state } = useSimConnection();
  const [selectedAgentId, setSelectedAgentId] = useState<number | null>(null);
  const selectedAgent = useMemo(
    () =>
      state.snapshot?.agents.find((agent) => agent.id === selectedAgentId) ?? null,
    [selectedAgentId, state.snapshot],
  );
  const activeSelectedAgentId = selectedAgent?.id ?? null;

  return (
    <main className="mx-auto box-border flex min-w-0 w-full max-w-6xl flex-1 flex-col gap-4 p-6">
      <header className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">gecko-sim</h1>
        <ConnectionStatus />
      </header>
      <Controls />
      <div className="grid min-w-0 gap-4 lg:grid-cols-[minmax(0,1fr)_20rem]">
        <WorldScene
          world={state.world}
          snapshot={state.snapshot}
          selectedAgentId={activeSelectedAgentId}
          onSelectAgent={setSelectedAgentId}
        />
        <AgentInspector agent={selectedAgent} />
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
