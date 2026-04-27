//! ECS system: execute. Completes any committed action whose
//! `expected_end_tick` has been reached this tick.
//!
//! For object-targeted actions: looks up the advertisement via the
//! catalog, applies each `Effect` to the agent's components, pushes a
//! `RecentActionEntry` into the agent's recent-actions ring, clears
//! `CurrentAction`.
//!
//! For self-actions (`Idle`/`Wait`): clears `CurrentAction` only — no
//! effects, no ring entry.

use bevy_ecs::system::{Query, Res};

use crate::agent::{Mood, Needs};
use crate::decision::{ActionRef, CurrentAction, RecentActionEntry, RecentActionsRing};
use crate::ids::{AdvertisementId, ObjectId};
use crate::object::{Advertisement, ObjectCatalog, SmartObject};
use crate::systems::decision::effects::{apply as apply_effect, EffectTarget};
use crate::time::CurrentTick;

/// Run the execute phase of the decision runtime: complete any agent
/// whose committed action has reached its `expected_end_tick`.
///
/// For object-targeted actions: applies the ad's effects, pushes a
/// `RecentActionEntry`, clears `CurrentAction`. For self-actions
/// (`Idle`/`Wait`): clears `CurrentAction` only.
#[allow(
    clippy::needless_pass_by_value,
    reason = "bevy_ecs SystemParam: Res must be passed by value"
)]
pub(crate) fn execute(
    catalog: Res<ObjectCatalog>,
    current_tick: Res<CurrentTick>,
    objects: Query<&SmartObject>,
    mut agents: Query<(
        &mut Needs,
        &mut Mood,
        &mut RecentActionsRing,
        &mut CurrentAction,
    )>,
) {
    for (mut needs, mut mood, mut ring, mut current) in &mut agents {
        let Some(action) = &current.0 else {
            continue;
        };
        if current_tick.0 < action.expected_end_tick {
            continue;
        }

        match action.action {
            ActionRef::Object { object, ad } => {
                if let Some((type_id, advertisement)) =
                    lookup_advertisement(&catalog, &objects, object, ad)
                {
                    let mut target = EffectTarget {
                        needs: &mut needs,
                        mood: &mut mood,
                    };
                    for effect in &advertisement.effects {
                        apply_effect(effect, &mut target);
                    }
                    ring.push(RecentActionEntry {
                        ad_template: (type_id, ad),
                        completed_tick: current_tick.0,
                    });
                } else {
                    tracing::warn!(
                        ?object,
                        ?ad,
                        "decision::execute: advertisement not found in catalog; clearing action"
                    );
                }
            }
            ActionRef::SelfAction(_) => {
                // No effects, no ring entry — just clear.
            }
        }
        current.0 = None;
    }
}

/// Resolve `(ObjectId, AdvertisementId)` to `(ObjectTypeId, &Advertisement)`
/// via the world's smart-object instances and the catalog.
fn lookup_advertisement<'a>(
    catalog: &'a ObjectCatalog,
    objects: &Query<&SmartObject>,
    object_id: ObjectId,
    ad_id: AdvertisementId,
) -> Option<(crate::ids::ObjectTypeId, &'a Advertisement)> {
    let object = objects.iter().find(|o| o.id == object_id)?;
    let object_type = catalog.by_id.get(&object.type_id)?;
    let advertisement = object_type.advertisements.iter().find(|a| a.id == ad_id)?;
    Some((object.type_id, advertisement))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;

    use crate::agent::{Mood, Need, Needs, Personality};
    use crate::decision::{
        ActionRef, CommittedAction, CurrentAction, Phase, RecentActionsRing, SelfActionKind,
    };
    use crate::ids::{AdvertisementId, LeafAreaId, ObjectId, ObjectTypeId};
    use crate::object::{
        Advertisement, Effect, InterruptClass, MeshId, ObjectCatalog, ObjectType, Op, Predicate,
        ScoreTemplate, SmartObject, StateValue,
    };
    use crate::systems::decision::execute::execute;
    use crate::time::CurrentTick;
    use crate::world::Vec2;

    fn fridge_object_type() -> ObjectType {
        let mut state = HashMap::new();
        state.insert("stocked".to_string(), StateValue::Bool(true));
        ObjectType {
            id: ObjectTypeId::new(1),
            display_name: "Fridge".to_string(),
            mesh_id: MeshId(1),
            default_state: state,
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

    fn build_world(
        agent_needs: Needs,
        action: Option<CommittedAction>,
        current_tick: u64,
    ) -> (World, bevy_ecs::entity::Entity) {
        let mut world = World::new();
        let fridge = fridge_object_type();
        let mut object_types = HashMap::new();
        object_types.insert(fridge.id, fridge);
        world.insert_resource(ObjectCatalog { by_id: object_types });
        world.insert_resource(CurrentTick(current_tick));

        // One smart-object instance.
        let mut state = HashMap::new();
        state.insert("stocked".to_string(), StateValue::Bool(true));
        world.spawn(SmartObject {
            id: ObjectId::new(0),
            type_id: ObjectTypeId::new(1),
            location: LeafAreaId::DEFAULT,
            position: Vec2::ZERO,
            owner: None,
            state,
        });

        let agent = world
            .spawn((
                agent_needs,
                Mood::neutral(),
                CurrentAction(action),
                RecentActionsRing::default(),
            ))
            .id();
        (world, agent)
    }

    #[test]
    fn completed_action_applies_effects_and_clears_current_action() {
        let action = CommittedAction {
            action: ActionRef::Object {
                object: ObjectId::new(0),
                ad: AdvertisementId::new(1),
            },
            started_tick: 0,
            expected_end_tick: 10,
            phase: Phase::Performing,
            target_position: None,
        };
        let (mut world, agent) = build_world(
            Needs {
                hunger: 0.3,
                ..Needs::full()
            },
            Some(action),
            10,
        );
        let mut schedule = Schedule::default();
        schedule.add_systems(execute);
        schedule.run(&mut world);

        let needs = world.get::<Needs>(agent).unwrap();
        assert!((needs.hunger - 0.7).abs() < 1e-6, "hunger={}", needs.hunger);
        let current = world.get::<CurrentAction>(agent).unwrap();
        assert!(current.0.is_none(), "current_action should be None");
        let ring = world.get::<RecentActionsRing>(agent).unwrap();
        assert_eq!(ring.entries.len(), 1);
        assert_eq!(
            ring.entries[0].ad_template,
            (ObjectTypeId::new(1), AdvertisementId::new(1))
        );
    }

    #[test]
    fn in_progress_action_does_not_complete() {
        let action = CommittedAction {
            action: ActionRef::Object {
                object: ObjectId::new(0),
                ad: AdvertisementId::new(1),
            },
            started_tick: 0,
            expected_end_tick: 10,
            phase: Phase::Performing,
            target_position: None,
        };
        let (mut world, agent) = build_world(
            Needs {
                hunger: 0.3,
                ..Needs::full()
            },
            Some(action),
            5, // current_tick < expected_end_tick
        );
        let mut schedule = Schedule::default();
        schedule.add_systems(execute);
        schedule.run(&mut world);

        let needs = world.get::<Needs>(agent).unwrap();
        assert!((needs.hunger - 0.3).abs() < 1e-6, "hunger should be unchanged");
        let current = world.get::<CurrentAction>(agent).unwrap();
        assert!(current.0.is_some(), "current_action should still be set");
    }

    #[test]
    fn idle_self_action_clears_without_effects() {
        let action = CommittedAction {
            action: ActionRef::SelfAction(SelfActionKind::Idle),
            started_tick: 0,
            expected_end_tick: 5,
            phase: Phase::Performing,
            target_position: None,
        };
        let (mut world, agent) = build_world(Needs::full(), Some(action), 5);
        let mut schedule = Schedule::default();
        schedule.add_systems(execute);
        schedule.run(&mut world);

        let current = world.get::<CurrentAction>(agent).unwrap();
        assert!(current.0.is_none());
        let ring = world.get::<RecentActionsRing>(agent).unwrap();
        // Idle does NOT add a ring entry.
        assert!(ring.entries.is_empty());
    }

    #[test]
    fn no_action_is_noop() {
        let (mut world, agent) = build_world(Needs::full(), None, 5);
        let mut schedule = Schedule::default();
        schedule.add_systems(execute);
        schedule.run(&mut world);

        let current = world.get::<CurrentAction>(agent).unwrap();
        assert!(current.0.is_none());
    }
}
