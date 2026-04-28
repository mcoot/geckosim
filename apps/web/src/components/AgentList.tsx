"use client";

import { useSimConnection } from "@/lib/sim/connection";

const NEED_KEYS = ["hunger", "sleep", "social", "hygiene", "fun", "comfort"] as const;
const MOOD_KEYS = ["valence", "arousal", "stress"] as const;
const PERSONALITY_KEYS = [
  "openness",
  "conscientiousness",
  "extraversion",
  "agreeableness",
  "neuroticism",
] as const;
const PERSONALITY_LABELS = {
  openness: "O",
  conscientiousness: "C",
  extraversion: "E",
  agreeableness: "A",
  neuroticism: "N",
} as const;

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
          {MOOD_KEYS.map((k) => (
            <th key={k} className="px-2 py-1 capitalize text-neutral-500">
              {k}
            </th>
          ))}
          {PERSONALITY_KEYS.map((k) => (
            <th key={k} className="px-2 py-1 text-neutral-500" title={k}>
              {PERSONALITY_LABELS[k]}
            </th>
          ))}
          <th className="px-2 py-1">Doing</th>
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
            {MOOD_KEYS.map((k) => (
              <td key={k} className="px-2 py-1 font-mono text-neutral-500">
                {agent.mood[k].toFixed(2)}
              </td>
            ))}
            {PERSONALITY_KEYS.map((k) => (
              <td key={k} className="px-2 py-1 font-mono text-neutral-500">
                {agent.personality[k].toFixed(2)}
              </td>
            ))}
            <td className="px-2 py-1">
              {agent.current_action
                ? `${agent.current_action.display_name} (${(
                    agent.current_action.fraction_complete * 100
                  ).toFixed(0)}%)`
                : "—"}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
