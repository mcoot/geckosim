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
  facing:       Vec2,           // unit direction the agent is oriented; updated on movement / object interaction

  // Per-system state (numbering matches 0010)

  // (1) Needs
  needs:        Needs { hunger, sleep, social, hygiene, fun, comfort },   // 6 × f32 in [0,1]

  // (2) Personality — Big Five
  personality:  Personality { openness, conscientiousness, extraversion, agreeableness, neuroticism },  // 5 × f32 in [-1,1]

  // (3) Mood
  mood:         Mood { valence, arousal, stress },   // 3 × f32

  // (4) Memory — bounded ring, importance × recency eviction (cap 500; see Memory eviction below)
  memory:       BoundedRing<MemoryEntry, 500>,

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
  current_action:     Option<CommittedAction>,
  pending_interrupts: Vec<Interrupt>,
  recent_actions:     BoundedRing<RecentActionEntry, 16>,    // for recency_penalty in scoring; FIFO eviction
  known_places:       BoundedVec<LeafAreaId, 64>,

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

**Save behavior:** intrinsic appearance is **stored verbatim** in the save, not re-derived from seed on load. This protects existing geckos against changes to the procgen palettes between sim versions — visual identity remains stable across content updates.

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
  source: InterruptSource,               // NeedThreshold | MacroForcedAction | MacroPreconditionFailed | EnvironmentalEvent | AgentTargeted
  urgency: f32,
  payload: InterruptPayload,
}

TargetSpec =
  | Self_                                    // the acting agent
  | OwnerOfObject                            // owner of the smart object
  | OtherAgent { id: AgentId }               // explicit reference
  | NearbyAgent { selector: NearbySelector } // Random | Closest | HighestAffinity | …

SituationalModifier =
  | MoodWeight              { dim: MoodDim, weight: f32 }
  | MacroVarWeight          { var: MacroVar, weight: f32 }
  | TimeOfDayWeight         { peak_tick: u64, falloff: u32 }
  | RelationshipWithTarget  { field: RelField, weight: f32 }

ItemMeta {
  origin:      Option<AgentId>,    // source / previous owner (theft, gift)
  origin_tick: u64,                // when the current owner acquired it
  flags:       ItemFlags,          // bitflags: Stolen | Gift | Contraband | …
}

RecentActionEntry {
  ad_template:    (ObjectTypeId, AdvertisementId),  // template identity across instances
  completed_tick: u64,
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

Where:

```
personality_modifier = max(0.1, 1.0 + sensitivity * dot(agent.personality, ad.score_template.personality_weights))
```

with `sensitivity` ≈ 0.5 (tunable). The clamp keeps the multiplier strictly positive; the linear form means personality biases the score by roughly ±50% rather than flipping its sign. The dot product is well-defined: personality components live in `[-1, 1]` and `personality_weights` likewise — an extraverted agent and an extravert-friendly action align to a positive product.

### Effect application

**Effects apply atomically at end-tick** of an action that completes normally. Cleaner than streaming during duration; matches event-driven decision-making (0004); long-duration actions with intermediate effects can be modeled as multi-step actions chaining shorter advertisements (chaining itself is deferred — see open questions).

### Effect application under interruption

When an action is interrupted (need threshold crossed, macro-precondition failed per 0009, environmental event, agent-targeted), each effect resolves by kind:

| Effect kind                                                                                 | Behavior on interruption                                                  |
| ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| `AgentNeedDelta`, `AgentMoodDelta`, `AgentSkillDelta`, `MoneyDelta`                         | **Pro-rata** by completion fraction (e.g. interrupted at 60% → 60% applies). |
| `InventoryDelta`, `MemoryGenerate`, `RelationshipDelta`, `HealthConditionChange`, `PromotedEvent` | **All-or-nothing** — suppressed entirely on interrupt; applied only on normal completion. |

Rule of thumb: continuous effects scale; discrete events don't. Advertisements that need different semantics (e.g. "rob the bank" shouldn't pay 60% on interruption) should split into multiple shorter actions; a per-effect override can land later if needed.

### `TargetSpec` resolution

A `TargetSpec` resolves to **zero or one** `AgentId` per evaluation, in the context of the acting agent and (where applicable) the smart object the action targets:

| Variant                     | Resolves to                                                      |
| --------------------------- | ---------------------------------------------------------------- |
| `Self_`                     | the acting agent                                                 |
| `OwnerOfObject`             | the smart object's `owner` if it is an `AgentId`; otherwise none (`Household`/`Business` owners do not resolve to an agent) |
| `OtherAgent { id }`         | the named agent (consumers may filter by `alive` / colocation)   |
| `NearbyAgent { selector }`  | at most one agent picked by the selector (`Random`/`Closest`/`HighestAffinity`/…); none if no candidates |

Consumers map the resolved 0-or-1 result into their own shape:

- `Predicate::AgentRelationship` and `Effect::RelationshipDelta`: a `None` resolution makes the predicate false / the effect a no-op.
- `Effect::MemoryGenerate.participants: TargetSpec` lifts into `MemoryEntry.participants: BoundedVec<AgentId, 4>` as a 0- or 1-element vec.

`MemoryEntry.participants` is bounded at 4 because some memory creation paths beyond `Effect::MemoryGenerate` will record genuine multi-agent participation (e.g. group-event memories, post-v0 multi-target effects). At v0, the only memory-creation path is `MemoryGenerate`, so the participants list always holds 0 or 1 entries — but the cap is sized for the multi-participant case ahead of time.

Multi-target resolution (e.g. "everyone in this leaf area") is deferred. When it lands, the natural extension is either a new `TargetSpec` variant with a list-returning selector or a `Vec<TargetSpec>` field on the consuming effect.

### Promoted-event emission

`Effect::PromotedEvent` allocates a `PromotedEventId` from the sim's monotonic counter and synchronously appends to the promoted-event ring (per 0009). Emission happens during effect application at end-tick — same atomicity as other effects, so promoted events are suppressed when the action is interrupted (they're in the all-or-nothing bucket above).

### Schedules

Schedules live on the `Job` entity (employer's hours), not on the agent. Agent reads its schedule via `employment`. Personal habits at v0 are emergent from needs + personality — no per-agent personal schedule field.

The "virtual need" framing from 0004 is implemented as a **scoring modifier**, not a schema field. The scoring function reads `agent.employment` and `current_tick` directly:

- If the action's location matches `employment.workplace` and `current_tick` is inside `employment.schedule.work_window`, the score receives a positive boost.
- For non-work actions during the work window, the score is penalised proportionally to how overdue the agent is.

No new state on the agent — the forcing function lives in the scoring formula and reads existing employment fields.

### Agent generation (migration arrivals)

At v0, new geckos enter the sim only via macro-driven migration (per 0010). Each arrival is created at a macro tick boundary with:

- **Identity.** Fresh `AgentId`, generated `name`, sampled `age` (adult range), `gender`, `birth_tick = current_tick − age_in_ticks`, `alive: true`.
- **Appearance.** Procedural from the new agent's seed, per the rules above.
- **Personality.** Big Five sampled from a configured prior distribution (default roughly uniform, centered on zero).
- **Skills.** Sampled from an age-appropriate distribution.
- **Mood, needs.** Initialized to neutral values.
- **Memory, relationships, criminal record, pending consequences, inventory.** Empty.
- **Money.** Sampled from a starting distribution (typically tied to age and macro cost-of-living).
- **Housing.** Assigned to a vacant residence; if no shared assignment is available, a single-person `HouseholdId` is created.
- **Employment.** Optionally assigned to an open job (per macro employment rate).
- **RNG.** New seeded sub-stream from the world seed.

When reproduction lands post-v0, in-sim births grow a parallel pipeline that initializes from parental state instead.

### Memory eviction

The per-agent memory ring is capped at 500 entries to support story-emergence over multi-year agent lifetimes (per 0010 / 0005). When the ring fills, eviction picks the entry with the lowest `eviction_score`:

```
eviction_score(memory) = importance × recency_factor(memory.tick, current_tick)
```

`importance` is set at memory creation by the emitting `Effect::MemoryGenerate`; `recency_factor` decays slowly so old-but-important memories survive (the formative-event case). The exact decay curve is content/balancing — defer to first balancing pass.

### Inventory stacking

Items with `metadata: None` may stack within a single `InventorySlot` (`count > 1`). Items with `metadata: Some(...)` always occupy their own slot with `count = 1` — provenance (e.g. "stolen from Alice on tick 4321") is per-item, not per-stack.

Three generic apples = one slot, count 3. Three phones stolen from three different victims = three slots. Three apples gifted by the same friend on the same tick = could collapse into one slot if the metadata is identical, but implementations may also keep them separate; either is acceptable.

### Authoring format

- **Smart-object catalog and advertisements:** **RON** files loaded at startup. Rust-native, enums serialize cleanly, easy to edit by hand. Hot reload deferred.
- **Accessory catalog:** RON files (same loader).
- **Agent state:** Rust types only — agents are not authored content.

## Memory budget

Per-agent worst case ~50 KB (memory ring at 500 entries dominates; relationships and transactions also material). 1000 agents ≈ 50 MB. Comfortable.

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
