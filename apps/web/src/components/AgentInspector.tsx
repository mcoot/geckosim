"use client";

import type { AgentSnapshot } from "@/types/sim/AgentSnapshot";

const NEED_KEYS = ["hunger", "sleep", "social", "hygiene", "fun", "comfort"] as const;
const MOOD_KEYS = ["valence", "arousal", "stress"] as const;
const PERSONALITY_KEYS = [
  "openness",
  "conscientiousness",
  "extraversion",
  "agreeableness",
  "neuroticism",
] as const;

interface AgentInspectorProps {
  agent: AgentSnapshot | null;
}

function labelFor(key: string) {
  return key.replace(/_/g, " ").replace(/\b\w/g, (char) => char.toUpperCase());
}

function formatFraction(value: number) {
  return value.toFixed(2);
}

function formatPercent(value: number) {
  return `${Math.round(value * 100)}%`;
}

export function AgentInspector({ agent }: AgentInspectorProps) {
  return (
    <section
      aria-label="Agent inspector"
      className="rounded border border-neutral-300 bg-white p-4 text-sm dark:border-neutral-700 dark:bg-neutral-950"
    >
      {!agent ? (
        <div className="flex min-h-32 items-center justify-center text-neutral-500">
          Select a gecko
        </div>
      ) : (
        <div className="space-y-4">
          <header className="space-y-1">
            <h2 className="text-lg font-semibold">{agent.name}</h2>
            <p className="text-neutral-600 dark:text-neutral-400">
              {agent.current_action
                ? `${agent.current_action.phase} to ${agent.current_action.display_name}`
                : "Idle"}
            </p>
            {agent.current_action?.target_label && (
              <p className="text-neutral-600 dark:text-neutral-400">
                Target: {agent.current_action.target_label}
              </p>
            )}
            {agent.current_action && (
              <p className="font-mono text-xs text-neutral-500">
                Progress: {formatPercent(agent.current_action.fraction_complete)}
              </p>
            )}
            <p className="font-mono text-xs text-neutral-500">
              Leaf {agent.leaf} · ({agent.pos.x.toFixed(1)}, {agent.pos.y.toFixed(1)})
            </p>
          </header>

          <MetricGroup
            title="Needs"
            items={NEED_KEYS.map((key) => ({
              label: labelFor(key),
              value: formatFraction(agent.needs[key]),
            }))}
          />
          <MetricGroup
            title="Mood"
            items={MOOD_KEYS.map((key) => ({
              label: labelFor(key),
              value: formatFraction(agent.mood[key]),
            }))}
          />
          <MetricGroup
            title="Personality"
            items={PERSONALITY_KEYS.map((key) => ({
              label: labelFor(key),
              value: formatPercent(agent.personality[key]),
            }))}
          />
        </div>
      )}
    </section>
  );
}

function MetricGroup({
  title,
  items,
}: {
  title: string;
  items: Array<{ label: string; value: string }>;
}) {
  return (
    <section className="space-y-2" aria-label={title}>
      <h3 className="text-xs font-semibold uppercase text-neutral-500">
        {title}
      </h3>
      <dl className="grid grid-cols-2 gap-x-4 gap-y-1">
        {items.map((item) => (
          <div key={item.label} className="flex items-baseline justify-between gap-3">
            <dt className="text-neutral-600 dark:text-neutral-400">{item.label}</dt>
            <dd className="font-mono text-xs">{item.value}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
