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
