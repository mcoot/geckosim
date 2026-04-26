# 0007 — World / spatial model

- **Status:** Accepted
- **Date:** 2026-04-26

## Context

We need a spatial representation that supports cheap "what's near me" queries (for smart-object discovery from 0004), tractable pathfinding at hundreds-of-agents scale (0003), legible per-area effects (disasters, crime, policy from 0006), and natural mapping to social structure.

## Decision

**Hierarchical spatial graph**, with the same leaf pattern reused for interiors and outdoors.

```
World
└── District (5 in v0)
    ├── Building (~10 per district)
    │   └── Floor (≥1 per building)
    │       └── Room        ← leaf area: continuous 2D, 0.5m object grid
    └── Outdoor Zone        ← leaf area: continuous 2D, 0.5m object grid
        (~15–20 per district; dense urban feel)
```

### Leaf areas (rooms and outdoor zones)

- **Continuous 2D** for agent positions and movement.
- **0.5m grid** for smart-object placement (chairs, fridges, benches, …).
- Rooms and outdoor zones use the same data structure; they differ in tags, not in kind.

### Outdoor zone types (initial taxonomy)

- **Street segment** — one block of road; pedestrian by default. The vehicular side is structurally present (see "Future") but not simulated in v0.
- **Plaza / park** — open public space; can hold many smart objects.
- **Forecourt** — small zone immediately outside a building; bridges to that building's ground-floor entrance(s). A forecourt may be marked **private** (gates which agents can use its advertised actions); residential yards are modeled as oversized private forecourts.
- Extensible: alley, transit stop, parking lot, marketplace, … added when systems demand them.

### Building structure

- A building has one or more **floors**, each a graph of rooms.
- **Vertical connectors** (stairs, elevators) are explicit edges between floors.
- Every building has at least one ground-floor entrance edge to its forecourt.
- All floors are fully simulated regardless of occupancy (justified by 0003 scale).

### Connectivity & pathfinding

- Each level holds a **connectivity graph** of its children (room ↔ room within a floor; floor ↔ floor within a building; outdoor zone ↔ outdoor zone within a district; forecourt ↔ outdoor zones).
- **Hierarchical pathfinding:** A* on the building/district graph for cross-area routes; local navigation within the destination leaf area.
- **Smart-object queries scope by default:** "what can I eat here" walks zone → building → district → known elsewhere. No global spatial scan.

### Inter-district movement

- A sparse **transit graph** connects districts (a few edges per district).
- A gecko crossing district boundaries takes a long-distance travel action measured in ticks; the sim may step it through or fast-forward depending on observation.

### World boundedness

- The world is bounded — no wrap, no infinite scroll. Outside the city is simulated only by the macro layer (0006).

## Scale (v0)

- 5 districts
- ~10 buildings per district (~50 total)
- ~5 rooms per building average (~250 interior rooms)
- ~15–20 outdoor zones per district (~75–100 outdoor zones)
- ~350 leaf areas total

## Future / explicitly out of scope for v0

- **Vehicles.** Street segments structurally include a vehicular side so cars and transit can be added later without re-doing topology, but no driving sim, traffic flow, or parking in v0.
- **More outdoor zone types** (alleys, parking lots, transit stops) — add as needed.
- **World extension** beyond the bounded city — macro-only for now.

## Consequences

- A smart object lives in exactly one leaf area and advertises actions scoped to it. Cross-area discovery is opt-in via an agent's known places.
- Saves are leaf-area granular: per-room / per-zone state plus the world graph.
- Renderer can LOD aggressively by hierarchy level: building exteriors at district zoom; interiors only when zoomed in.
