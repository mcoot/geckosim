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
  const model = useMemo(() => buildWorldSceneModel(world, snapshot), [world, snapshot]);
  const summary =
    snapshot &&
    `tick ${snapshot.tick} | ${model.leaves.length} leaves | ${model.agents.length} agents | ${model.objects.length} objects`;

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
      runtimeRef.current = createWorldSceneRuntime(mount, EMPTY_WORLD_SCENE_MODEL);
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
    runtimeRef.current?.update(model);
  }, [model]);

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
        </>
      )}
    </section>
  );
}
