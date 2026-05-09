"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import type { Snapshot } from "@/types/sim/Snapshot";
import type { WorldLayout } from "@/types/sim/WorldLayout";
import {
  buildWorldSceneModel,
  EMPTY_WORLD_SCENE_MODEL,
} from "@/lib/world-scene/model";
import {
  canCreateWorldSceneRuntime,
  createWorldSceneRuntime,
  type WorldSceneRuntime,
} from "@/lib/world-scene/runtime";

interface WorldSceneProps {
  world: WorldLayout | null;
  snapshot: Snapshot | null;
}

export function WorldScene({ world, snapshot }: WorldSceneProps) {
  const mountRef = useRef<HTMLDivElement | null>(null);
  const runtimeRef = useRef<WorldSceneRuntime | null>(null);
  const [runtimeUnavailable, setRuntimeUnavailable] = useState(false);
  const [selectedAgentId, setSelectedAgentId] = useState<number | null>(null);
  const model = useMemo(() => buildWorldSceneModel(world, snapshot), [world, snapshot]);
  const selectedAgent = useMemo(
    () => snapshot?.agents.find((agent) => agent.id === selectedAgentId) ?? null,
    [snapshot, selectedAgentId],
  );
  const summary =
    snapshot &&
    `tick ${snapshot.tick} | ${model.leaves.length} leaves | ${model.agents.length} agents | ${model.objects.length} objects`;

  const activeSelectedAgentId = selectedAgent?.id ?? null;

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return;
    let fallbackTimer: number | null = null;

    const showUnavailable = () => {
      fallbackTimer = window.setTimeout(() => setRuntimeUnavailable(true), 0);
    };

    if (!canCreateWorldSceneRuntime()) {
      runtimeRef.current = null;
      showUnavailable();
      return () => {
        if (fallbackTimer !== null) {
          window.clearTimeout(fallbackTimer);
        }
      };
    }

    try {
      runtimeRef.current = createWorldSceneRuntime(
        mount,
        EMPTY_WORLD_SCENE_MODEL,
        { onSelectAgent: setSelectedAgentId },
      );
    } catch {
      runtimeRef.current = null;
      showUnavailable();
    }

    return () => {
      if (fallbackTimer !== null) {
        window.clearTimeout(fallbackTimer);
      }
      runtimeRef.current?.dispose();
      runtimeRef.current = null;
    };
  }, []);

  useEffect(() => {
    runtimeRef.current?.update(model, activeSelectedAgentId);
  }, [activeSelectedAgentId, model]);

  return (
    <section
      aria-label="World scene"
      className="space-y-2"
      style={{ width: "calc(100vw - 3rem)", maxWidth: "100%" }}
    >
      <div
        ref={mountRef}
        className="min-h-96 overflow-hidden rounded-lg border border-neutral-300 bg-neutral-950 dark:border-neutral-700"
      />
      {!world || !snapshot ? (
        <p className="text-sm text-neutral-500">Waiting for world data</p>
      ) : (
        <>
          {runtimeUnavailable && (
            <p className="text-sm text-neutral-500">3D view unavailable</p>
          )}
          <p className="text-xs text-neutral-500">{summary}</p>
          {selectedAgent && (
            <aside className="rounded border border-neutral-300 bg-white/90 p-3 text-sm shadow-sm dark:border-neutral-700 dark:bg-neutral-900/90">
              <div className="font-medium">{selectedAgent.name}</div>
              <div className="text-neutral-500">
                {selectedAgent.current_action
                  ? `${selectedAgent.current_action.phase} to ${selectedAgent.current_action.display_name}`
                  : "Idle"}
              </div>
              {selectedAgent.current_action?.target_label && (
                <div className="text-neutral-500">
                  {selectedAgent.current_action.target_label}
                </div>
              )}
              {selectedAgent.current_action && (
                <div className="mt-2 flex items-center gap-2">
                  <div className="h-2 w-32 overflow-hidden rounded bg-neutral-200 dark:bg-neutral-800">
                    <div
                      className="h-full rounded bg-blue-500"
                      style={{
                        width: `${Math.round(
                          selectedAgent.current_action.fraction_complete * 100,
                        )}%`,
                      }}
                    />
                  </div>
                  <span className="font-mono text-xs">
                    {(
                      selectedAgent.current_action.fraction_complete * 100
                    ).toFixed(0)}
                    %
                  </span>
                </div>
              )}
            </aside>
          )}
        </>
      )}
    </section>
  );
}
