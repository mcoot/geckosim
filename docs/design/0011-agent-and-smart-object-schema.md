# 0011 — Agent and smart-object schema

- **Status:** Accepted
- **Date:** 2026-04-26

## Context

0010 fixed the v0 systems list. This doc defines the concrete data shapes for agents (geckos), smart objects, and the advertisement format that connects them. It is the bridge from design to Rust types — the contract that 0012 (crate architecture) and 0013 (frontend transport) build on.

Field types are sketched in pseudo-Rust; precise representations (`Vec` vs `SmallVec` vs `BoundedVec`, exact bit widths) are an implementation choice as long as the contract holds.

## Decision

### Identifiers

All entities use stable `u64` newtype IDs, preserved across saves:

`AgentId`, `ObjectId`, `ObjectTypeId`, `BuildingId`, `LeafAreaId`, `HousingId`, `EmploymentId`, `HouseholdId`, `BusinessId`, `CrimeIncidentId`, `MemoryEntryId`, `AccessoryId`.

`OwnerRef` is a sum type over `AgentId | HouseholdId | BusinessId` for owned entities (e.g. fridges, registers).

### Agent (gecko) shape

```
Gecko {
  // Identity
  id:           AgentId,
  name:         String,
  age:          u16,            // sim-years
  gender:       Gender,
  birth_tick:   u64,
  alive:        bool,           // false after death; record persists for memory references

  // Appearance (see below)
  appearance:   Appearance,

  // Spatial
  current_leaf: LeafAreaId,
  position:     Vec2,           // within leaf area, 0.5m grid for object-aligned positions

  // Per-system state (numbering matches 0010)

  // (1) Needs
  needs:        Needs { hunger, sleep, social, hygiene, fun, comfort },   // 6 × f32 in [0,1]

  // (2) Personality — Big Five
  personality:  Personality { openness, conscientiousness, extraversion, agreeableness, neuroticism },  // 5 × f32 in [-1,1]

  // (3) Mood
  mood:         Mood { valence, arousal, stress },   // 3 × f32

  // (4) Memory — bounded ring, importance-weighted eviction (cap 100)
  memory:       BoundedRing<MemoryEntry, 100>,

  // (5) Relationships — sparse; only non-trivial pairs stored
  relationships: SparseMap<AgentId, RelationshipEdge>,

  // (6) Skills
  skills:       Skills { social, manual, cognitive, physical, artistic },  // 5 × f32 in [0,1]

  // (7) Money — i64 minor units (cents) for determinism
  money:        i64,
  recent_transactions: BoundedRing<Transaction, 32>,

  // (8) Housing
  residence:    Option<HousingId>,
  household:    Option<HouseholdId>,

  // (9) Employment
  employment:   Option<EmploymentId>,    // schedule lives on the Job entity, not here

  // (10) Health
  health:       HealthState {
    conditions:  Vec<HealthCondition>,
    vitality:    f32,                    // hits 0 → dies
  },

  // (11) Crime
  criminal_record:      Vec<CrimeIncidentId>,
  pending_consequences: Vec<Consequence>,

  // Cross-cutting: inventory (8 slots, typed)
  inventory:    BoundedVec<InventorySlot, 8>,

  // Decision runtime (per 0004)
  current_action:    Option<CommittedAction>,
  pending_interrupts: Vec<Interrupt>,
  known_places:      BoundedVec<LeafAreaId, 64>,

  // Determinism (per 0008)
  rng:          PrngState,                 // seeded sub-stream of world seed
}
```

### Appearance

Two parts. Intrinsic appearance is **procedurally generated from the agent's seed at creation** — deterministic, no authoring, gives a population of distinct-looking geckos for free. Accessories are aesthetic-only at v0 (no theft / no economic value / no scoring effects).

```
Appearance {
  intrinsic: {
    base_color:     Color,
    pattern:        Pattern,         // Solid | Stripes | Spots | Mottled
    pattern_color:  Color,
    eye_color:      Color,
    body_size:      Size,            // Small | Medium | Large
    tail_length:    TailLength,      // Short | Medium | Long
  },
  accessories: BoundedVec<AccessoryId, 4>   // refs into static accessory catalog
}
```

Accessories may graduate to a worn-item system later if/when they need sim mechanics; until then, slot the data and defer the depth (same pattern as deferred vehicles in 0007).

### Supporting types (sketch)

```
MemoryEntry {
  id, kind: MemoryKind, tick: u64,
  participants: BoundedVec<AgentId, 4>,
  location: LeafAreaId,
  valence: f32,         // -1..1
  importance: f32,      // 0..1, decays over time; eviction key
}

RelationshipEdge {
  affinity:    f32,     // -1..1
  trust:       f32,     // 0..1
  familiarity: f32,     // 0..1
  last_interaction_tick: u64,
}

HealthCondition {
  kind: ConditionKind, severity: f32,
  onset_tick: u64, expected_recovery_tick: Option<u64>,
}

InventorySlot {
  item: ItemType,                 // enum: Food, WorkMaterial, StolenGood, Gift, ...
  count: u16,
  metadata: Option<ItemMeta>,
}

CommittedAction {
  ad_ref: (ObjectId, AdvertisementId),  // OR a typed self-action
  started_tick: u64,
  expected_end_tick: u64,
  phase: Phase,                          // Walking | Performing | Completing
  target_position: Option<Vec2>,
}

Interrupt {
  source: InterruptSource,               // NeedThreshold | MacroForcedAction | EnvironmentalEvent | AgentTargeted
  urgency: f32,
  payload: InterruptPayload,
}
```

### Smart-object shape

Two-level: a static **catalog of object types** loaded from data files at startup, and per-instance state in the world.

```
ObjectType {                         // static, from RON catalog
  id:              ObjectTypeId,
  display_name:    String,
  mesh_id:         MeshId,           // renderer hint
  default_state:   StateMap,
  advertisements:  Vec<Advertisement>,
}

SmartObject {                        // per instance
  id:        ObjectId,
  type_id:   ObjectTypeId,
  location:  LeafAreaId,
  position:  Vec2,                   // 0.5m grid-aligned per 0007
  owner:     Option<OwnerRef>,
  state:     StateMap,               // type-specific instance state
}
```

Advertisements are properties of the type; per-instance filtering happens by evaluating preconditions against the instance's `state`.

### Advertisement format (the key contract)

```
Advertisement {
  id:           AdvertisementId,
  display_name: String,

  preconditions: Vec<Predicate>,     // all must hold
  effects:       Vec<Effect>,        // applied atomically at end-tick
  duration_ticks: u32,
  interrupt_class: InterruptClass,   // Always | NeedsThresholdOnly | Never
  score_template: ScoreTemplate,
}

Predicate =
  | AgentNeed(Need, Op, f32)
  | AgentSkill(Skill, Op, f32)
  | AgentInventory(ItemType, Op, u16)
  | AgentRelationship(TargetSpec, RelField, Op, f32)
  | ObjectState(StateKey, Op, StateValue)
  | MacroState(MacroVar, Op, MacroValue)        // gating per 0009
  | Spatial(SpatialReq)                          // SameLeafArea | AdjacentArea | KnownPlace | ...
  | TimeOfDay(TickRange)

Effect =
  | AgentNeedDelta(Need, f32)
  | AgentMoodDelta(MoodDim, f32)
  | AgentSkillDelta(Skill, f32)
  | MoneyDelta(i64)
  | InventoryDelta(ItemType, i32)
  | MemoryGenerate(MemoryKind, importance: f32, valence: f32, participants: TargetSpec)
  | RelationshipDelta(TargetSpec, RelField, f32)
  | HealthConditionChange(...)
  | PromotedEvent(EventType, Payload)            // per 0009 taxonomy

ScoreTemplate {
  need_weights:        Vec<(Need, f32)>,         // base utility = Σ need_pressure × weight
  personality_weights: Personality,              // dot with agent personality
  situational_modifiers: Vec<SituationalModifier>,  // mood / macro / time-of-day terms
}
```

### Action evaluation contract

```
score(agent, ad, macro_ctx) =
      base_utility(agent.needs, ad.score_template.need_weights)
    × personality_modifier(agent.personality, ad.score_template.personality_weights)
    × situational_modifier(agent.mood, macro_ctx, ad.score_template.situational_modifiers)
    × (1 - recency_penalty(agent.recent_actions, ad.id))
    + noise(agent.rng)
```

Pick **weighted-random from top-N** (per 0004), not strict argmax.

### Effect application

**Effects apply atomically at end-tick** of an action. Cleaner than streaming during duration; matches event-driven decision-making (0004); long-duration actions with intermediate effects can be modeled as multi-step actions chaining shorter advertisements.

### Schedules

Schedules live on the `Job` entity (employer's hours), not on the agent. Agent reads its schedule via `employment`. Personal habits at v0 are emergent from needs + personality — no per-agent personal schedule field.

### Authoring format

- **Smart-object catalog and advertisements:** **RON** files loaded at startup. Rust-native, enums serialize cleanly, easy to edit by hand. Hot reload deferred.
- **Accessory catalog:** RON files (same loader).
- **Agent state:** Rust types only — agents are not authored content.

## Memory budget

Per-agent worst case ~17 KB (memory ring + relationships + transactions dominate). 1000 agents ≈ 17 MB. Comfortable.

## Consequences

- 0012 (crate architecture) implements these types and the loader for the RON catalogs.
- 0013 (frontend transport) defines how a tick snapshot or delta of this state is serialized to the renderer.
- New systems added post-v0 either extend an existing field or add a new top-level field on `Gecko`. Adding a new field is a save-format change; design accordingly.
- The `Predicate` and `Effect` enums are extension points — new variants are added as systems demand them. Existing RON content remains valid as long as variants aren't removed or renamed.

## Open questions

- **Decay-rate location.** Are need-decay rates universal constants, on personality (e.g. neurotic geckos get hungry faster), or modulated by macro (cold weather → faster comfort decay)? Likely all three combined; concrete formula deferred until first balancing pass.
- **Memory importance formula.** Combination of valence magnitude, participants' relationship strength, and event kind. Specifics deferred to first implementation.
- **Action chaining.** "Multi-step actions" mentioned above need a representation — probably a top-level field on `CommittedAction` for `next: Option<...>`, but defer until needed.
- **Procedural appearance palette.** The exact color palettes / pattern weights are content, not architecture. Defer to first content pass.
