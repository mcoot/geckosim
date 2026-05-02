//! ECS system: decide. For each agent without a current action, builds
//! the candidate-advertisement list, filters by predicates, scores the
//! survivors, picks weighted-random from top-N, and commits.
//!
//! Falls back to `SelfAction(Idle)` (with `IDLE_DURATION_TICKS = 5`) when
//! no advertisements survive predicate filtering.

use bevy_ecs::system::{ParamSet, Query, Res, ResMut};
use rand::Rng;

use crate::agent::{Mood, Needs, Personality, Position};
use crate::decision::{
    ActionRef, CommittedAction, CurrentAction, IDLE_DURATION_TICKS, Phase, RecentActionsRing,
    SelfActionKind,
};
use crate::ids::{AdvertisementId, ObjectId, ObjectTypeId};
use crate::object::{Advertisement, ObjectCatalog, SmartObject};
use crate::sim::SimRngResource;
use crate::systems::decision::interaction::{
    OccupiedInteractionSpots, ResolvedInteractionTarget, resolve_interaction_target,
};
use crate::systems::decision::predicates::{EvalContext, evaluate};
use crate::systems::decision::scoring::{
    base_utility, mood_modifier, personality_modifier, recency_penalty, weighted_pick,
};
use crate::systems::movement::ARRIVE_EPSILON;
use crate::time::CurrentTick;

/// Pick the top-N highest-scoring candidates before weighted-pick.
const TOP_N: usize = 3;

/// Noise scale: each candidate's score gets a uniform `[0, NOISE_SCALE)`
/// addition. Per ADR 0011 this lets equal-scoring candidates break ties
/// stochastically.
const NOISE_SCALE: f32 = 0.1;

/// Run the decide phase: for each agent with no current action, choose
/// the next action via the v0 utility-AI scorer.
#[allow(
    clippy::needless_pass_by_value,
    reason = "bevy_ecs SystemParam: Res must be passed by value"
)]
pub(crate) fn decide(
    catalog: Res<ObjectCatalog>,
    current_tick: Res<CurrentTick>,
    mut sim_rng: ResMut<SimRngResource>,
    objects: Query<&SmartObject>,
    mut agents: ParamSet<(
        Query<&CurrentAction>,
        Query<(
            &Needs,
            &Mood,
            &Personality,
            &Position,
            &RecentActionsRing,
            &mut CurrentAction,
        )>,
    )>,
) {
    let mut occupied = collect_occupied_spots(&agents.p0());
    // PrngState wraps Pcg64Mcg in a tuple struct; reach into the inner
    // RNG (which implements rand::Rng via the blanket impl on RngCore).
    let prng = &mut sim_rng.0.0;
    for (needs, mood, personality, position, recent_ring, mut current) in &mut agents.p1() {
        if current.0.is_some() {
            continue;
        }
        let next = pick_next_action(
            needs,
            mood,
            personality,
            position,
            recent_ring,
            &catalog,
            &objects,
            &occupied,
            current_tick.0,
            prng,
        );
        if let (ActionRef::Object { object, .. }, Some(spot)) = (next.action, next.target_spot) {
            occupied.insert(object, spot);
        }
        current.0 = Some(next);
    }
}

#[allow(clippy::too_many_arguments)]
fn pick_next_action<R: Rng + ?Sized>(
    needs: &Needs,
    mood: &Mood,
    personality: &Personality,
    position: &Position,
    recent_ring: &RecentActionsRing,
    catalog: &ObjectCatalog,
    objects: &Query<&SmartObject>,
    occupied: &OccupiedInteractionSpots,
    current_tick: u64,
    prng: &mut R,
) -> CommittedAction {
    // Build candidates filtered by predicates and scored.
    let mut scored: Vec<(
        ObjectId,
        ObjectTypeId,
        AdvertisementId,
        u32,
        ResolvedInteractionTarget,
        f32,
    )> = Vec::new();
    for object in objects.iter() {
        let Some(object_type) = catalog.by_id.get(&object.type_id) else {
            continue;
        };
        for ad in &object_type.advertisements {
            let ctx = EvalContext {
                needs,
                agent_leaf: position.leaf,
                object_state: &object.state,
                object_leaf: object.location,
            };
            if !ad.preconditions.iter().all(|p| evaluate(p, &ctx)) {
                continue;
            }
            let Some(target) =
                resolve_interaction_target(object, object_type, position, occupied, None)
            else {
                continue;
            };
            let score = score_advertisement(
                needs,
                mood,
                personality,
                recent_ring,
                object_type.id,
                ad,
                prng,
            );
            scored.push((
                object.id,
                object_type.id,
                ad.id,
                ad.duration_ticks,
                target,
                score,
            ));
        }
    }

    // Sort descending by score; truncate to top-N.
    scored.sort_by(|a, b| b.5.partial_cmp(&a.5).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(TOP_N);

    if scored.is_empty() {
        return CommittedAction {
            action: ActionRef::SelfAction(SelfActionKind::Idle),
            started_tick: current_tick,
            expected_end_tick: Some(current_tick + u64::from(IDLE_DURATION_TICKS)),
            phase: Phase::Performing,
            target_position: None,
            target_spot: None,
            target_facing: None,
            perform_duration_ticks: IDLE_DURATION_TICKS,
        };
    }

    // weighted_pick returns the index into `scored` so we resolve the
    // full (ObjectId, AdvertisementId, duration) tuple unambiguously —
    // AdvertisementId alone is not unique across object types.
    let weights: Vec<f32> = scored
        .iter()
        .map(|(_, _, _, _, _, score)| *score)
        .collect();
    let picked_idx = weighted_pick(&weights, prng).expect("non-empty after early return");
    let (object_id, _type_id, ad_id, duration_ticks, target, _score) = scored[picked_idx];

    let target_pos = target.position;
    let dx = target_pos.x - position.pos.x;
    let dy = target_pos.y - position.pos.y;
    let already_there = (dx * dx + dy * dy) <= ARRIVE_EPSILON * ARRIVE_EPSILON;

    let (phase, expected_end_tick) = if already_there {
        (
            Phase::Performing,
            Some(current_tick + u64::from(duration_ticks)),
        )
    } else {
        (Phase::Walking, None)
    };

    CommittedAction {
        action: ActionRef::Object {
            object: object_id,
            ad: ad_id,
        },
        started_tick: current_tick,
        expected_end_tick,
        phase,
        target_position: Some(target_pos),
        target_spot: target.spot,
        target_facing: target.facing,
        perform_duration_ticks: duration_ticks,
    }
}

fn collect_occupied_spots(actions: &Query<&CurrentAction>) -> OccupiedInteractionSpots {
    let mut occupied = OccupiedInteractionSpots::default();
    for current in actions {
        let Some(action) = &current.0 else {
            continue;
        };
        let ActionRef::Object { object, .. } = action.action else {
            continue;
        };
        if !matches!(action.phase, Phase::Walking | Phase::Performing) {
            continue;
        }
        if let Some(spot) = action.target_spot {
            occupied.insert(object, spot);
        }
    }
    occupied
}

fn score_advertisement<R: Rng + ?Sized>(
    needs: &Needs,
    mood: &Mood,
    personality: &Personality,
    recent_ring: &RecentActionsRing,
    type_id: ObjectTypeId,
    ad: &Advertisement,
    prng: &mut R,
) -> f32 {
    let base = base_utility(needs, &ad.score_template);
    let pers = personality_modifier(personality, &ad.score_template.personality_weights);
    let md = mood_modifier(mood, &ad.score_template.situational_modifiers);
    let pen = recency_penalty(recent_ring, type_id, ad.id);
    let noise = prng.random::<f32>() * NOISE_SCALE;
    base * pers * md * (1.0 - pen) + noise
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;

    use crate::agent::{Mood, Need, Needs, Personality, Position};
    use crate::decision::{
        ActionRef, CommittedAction, CurrentAction, IDLE_DURATION_TICKS, Phase,
        RecentActionsRing, SelfActionKind,
    };
    use crate::ids::{AdvertisementId, InteractionSpotId, LeafAreaId, ObjectId, ObjectTypeId};
    use crate::object::{
        Advertisement, Effect, InteractionSpot, InterruptClass, MeshId, ObjectCatalog,
        ObjectType, Op, Predicate, ScoreTemplate, SmartObject, StateValue,
    };
    use crate::rng::PrngState;
    use crate::sim::SimRngResource;
    use crate::systems::decision::decide::decide;
    use crate::time::CurrentTick;
    use crate::world::{Vec2, WorldGraph};

    fn fridge_object_type() -> ObjectType {
        let mut state = HashMap::new();
        state.insert("stocked".to_string(), StateValue::Bool(true));
        ObjectType {
            id: ObjectTypeId::new(1),
            display_name: "Fridge".to_string(),
            mesh_id: MeshId(1),
            default_state: state,
            interaction_spots: vec![],
            advertisements: vec![Advertisement {
                id: AdvertisementId::new(1),
                display_name: "Eat snack".to_string(),
                preconditions: vec![Predicate::AgentNeed(Need::Hunger, Op::Lt, 0.6)],
                effects: vec![Effect::AgentNeedDelta(Need::Hunger, 0.4)],
                duration_ticks: 10,
                interrupt_class: InterruptClass::NeedsThresholdOnly,
                score_template: ScoreTemplate {
                    need_weights: vec![(Need::Hunger, 1.0)],
                    personality_weights: Personality::default(),
                    situational_modifiers: vec![],
                },
            }],
        }
    }

    fn fridge_object_type_with_spot() -> ObjectType {
        let mut object_type = fridge_object_type();
        object_type.interaction_spots = vec![InteractionSpot {
            id: InteractionSpotId::new(1),
            offset: Vec2::new(0.0, -1.0),
            facing: Vec2::new(0.0, 1.0),
            label: Some("door".to_string()),
        }];
        object_type
    }

    fn build_world(agent_needs: Needs) -> (World, bevy_ecs::entity::Entity) {
        build_world_with(fridge_object_type(), Vec2::ZERO, Vec2::ZERO, agent_needs)
    }

    fn build_world_with(
        object_type: ObjectType,
        object_position: Vec2,
        agent_position: Vec2,
        agent_needs: Needs,
    ) -> (World, bevy_ecs::entity::Entity) {
        let mut world = World::new();
        let mut object_types = HashMap::new();
        object_types.insert(object_type.id, object_type);
        world.insert_resource(ObjectCatalog { by_id: object_types });
        world.insert_resource(CurrentTick(0));
        world.insert_resource(SimRngResource(PrngState::from_seed(42)));
        world.insert_resource(WorldGraph::seed_v0());

        let mut state = HashMap::new();
        state.insert("stocked".to_string(), StateValue::Bool(true));
        world.spawn(SmartObject {
            id: ObjectId::new(0),
            type_id: ObjectTypeId::new(1),
            location: LeafAreaId::new(0),
            position: object_position,
            owner: None,
            state,
        });

        let agent = world
            .spawn((
                agent_needs,
                Mood::neutral(),
                Personality::default(),
                Position {
                    leaf: LeafAreaId::new(0),
                    pos: agent_position,
                },
                CurrentAction::default(),
                RecentActionsRing::default(),
            ))
            .id();
        (world, agent)
    }

    #[test]
    fn hungry_agent_commits_eat_snack() {
        let (mut world, agent) = build_world(Needs {
            hunger: 0.3,
            ..Needs::full()
        });
        let mut schedule = Schedule::default();
        schedule.add_systems(decide);
        schedule.run(&mut world);

        let current = world.get::<CurrentAction>(agent).unwrap();
        let action = current.0.as_ref().expect("CurrentAction should be set");
        match action.action {
            ActionRef::Object { object, ad } => {
                assert_eq!(object, ObjectId::new(0));
                assert_eq!(ad, AdvertisementId::new(1));
            }
            ActionRef::SelfAction(_) => panic!("expected Object action"),
        }
        assert_eq!(action.started_tick, 0);
        // Agent at (0,0), fridge at (0,0) → already there → Performing
        // with duration_ticks = 10 → expected_end_tick = Some(10).
        assert_eq!(action.expected_end_tick, Some(10));
        assert_eq!(action.phase, Phase::Performing);
        assert_eq!(action.perform_duration_ticks, 10);
    }

    #[test]
    fn object_action_targets_interaction_spot() {
        let (mut world, agent) = build_world_with(
            fridge_object_type_with_spot(),
            Vec2::new(5.0, 5.0),
            Vec2::ZERO,
            Needs {
                hunger: 0.3,
                ..Needs::full()
            },
        );
        let mut schedule = Schedule::default();
        schedule.add_systems(decide);
        schedule.run(&mut world);

        let action = world
            .get::<CurrentAction>(agent)
            .unwrap()
            .0
            .as_ref()
            .expect("CurrentAction should be set");
        assert_eq!(action.target_position, Some(Vec2::new(5.0, 4.0)));
        assert_eq!(action.target_spot, Some(InteractionSpotId::new(1)));
        assert_eq!(action.target_facing, Some(Vec2::new(0.0, 1.0)));
        assert_eq!(action.phase, Phase::Walking);
    }

    #[test]
    fn occupied_single_spot_filters_object_action() {
        let (mut world, agent) = build_world_with(
            fridge_object_type_with_spot(),
            Vec2::new(5.0, 5.0),
            Vec2::ZERO,
            Needs {
                hunger: 0.3,
                ..Needs::full()
            },
        );
        world.spawn((
            Needs::full(),
            Mood::neutral(),
            Personality::default(),
            Position {
                leaf: LeafAreaId::new(0),
                pos: Vec2::new(1.0, 0.0),
            },
            CurrentAction(Some(CommittedAction {
                action: ActionRef::Object {
                    object: ObjectId::new(0),
                    ad: AdvertisementId::new(1),
                },
                started_tick: 0,
                expected_end_tick: None,
                phase: Phase::Walking,
                target_position: Some(Vec2::new(5.0, 4.0)),
                target_spot: Some(InteractionSpotId::new(1)),
                target_facing: Some(Vec2::new(0.0, 1.0)),
                perform_duration_ticks: 10,
            })),
            RecentActionsRing::default(),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(decide);
        schedule.run(&mut world);

        let action = world
            .get::<CurrentAction>(agent)
            .unwrap()
            .0
            .as_ref()
            .expect("CurrentAction should be set");
        assert!(matches!(action.action, ActionRef::SelfAction(SelfActionKind::Idle)));
    }

    #[test]
    fn full_needs_agent_falls_back_to_idle() {
        // hunger = 1.0 > 0.6 → AgentNeed(Hunger, Lt, 0.6) fails → no candidates.
        let (mut world, agent) = build_world(Needs::full());
        let mut schedule = Schedule::default();
        schedule.add_systems(decide);
        schedule.run(&mut world);

        let current = world.get::<CurrentAction>(agent).unwrap();
        let action = current.0.as_ref().expect("CurrentAction should be set");
        match action.action {
            ActionRef::SelfAction(kind) => {
                assert_eq!(kind, SelfActionKind::Idle);
            }
            ActionRef::Object { .. } => {
                panic!("expected SelfAction(Idle), got {:?}", action.action)
            }
        }
        assert_eq!(action.expected_end_tick, Some(u64::from(IDLE_DURATION_TICKS)));
    }

    #[test]
    fn agent_with_existing_action_is_skipped() {
        let (mut world, agent) = build_world(Needs {
            hunger: 0.3,
            ..Needs::full()
        });
        // Pre-commit a different action.
        world
            .get_mut::<CurrentAction>(agent)
            .unwrap()
            .0 = Some(crate::decision::CommittedAction {
                action: ActionRef::SelfAction(SelfActionKind::Wait),
                started_tick: 0,
                expected_end_tick: Some(100),
                phase: Phase::Performing,
                target_position: None,
                target_spot: None,
                target_facing: None,
                perform_duration_ticks: 100,
            });

        let mut schedule = Schedule::default();
        schedule.add_systems(decide);
        schedule.run(&mut world);

        // Action unchanged.
        let action = world.get::<CurrentAction>(agent).unwrap().0.as_ref().unwrap();
        assert!(matches!(
            action.action,
            ActionRef::SelfAction(SelfActionKind::Wait)
        ));
    }
}
