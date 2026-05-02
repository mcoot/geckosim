//! ECS system: walk. Per ADR 0007 spatial pass — agents in
//! `Phase::Walking` step toward their target each tick at
//! `WALK_SPEED_M_PER_TICK`. On arrival the agent transitions to
//! `Phase::Performing` and the action's `expected_end_tick` is computed
//! from `perform_duration_ticks`.

use bevy_ecs::system::{Query, Res};

use crate::agent::{Facing, Position};
use crate::decision::{CurrentAction, Phase};
use crate::time::CurrentTick;
use crate::world::Vec2;

/// Walking speed in meters per sim-tick. ADR 0008 sets 1 tick = 1
/// sim-minute; 80 m/min ≈ 5 km/h, a normal walking pace.
pub const WALK_SPEED_M_PER_TICK: f32 = 80.0;

/// Snap-to-target threshold. 5 cm — well below the 0.5m smart-object grid.
pub const ARRIVE_EPSILON: f32 = 0.05;

/// Step `from` toward `to` by at most `max_dist` meters. Returns the new
/// position and a unit-vector facing for the step (or `Vec2::ZERO` when
/// the caller arrived this tick — caller leaves `Facing` unchanged).
pub(crate) fn step_toward(from: Vec2, to: Vec2, max_dist: f32) -> (Vec2, Vec2) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let dist_sq = dx * dx + dy * dy;
    if dist_sq <= max_dist * max_dist {
        return (to, Vec2::ZERO);
    }
    let dist = dist_sq.sqrt();
    let inv = 1.0 / dist;
    let dir = Vec2 {
        x: dx * inv,
        y: dy * inv,
    };
    let next = Vec2 {
        x: from.x + dir.x * max_dist,
        y: from.y + dir.y * max_dist,
    };
    (next, dir)
}

/// Run the walk phase: advance each `Walking` agent's position toward
/// its `target_position`, transitioning to `Performing` on arrival.
#[allow(
    clippy::needless_pass_by_value,
    reason = "bevy_ecs SystemParam: Res must be passed by value"
)]
pub(crate) fn walk(
    current_tick: Res<CurrentTick>,
    mut agents: Query<(&mut Position, &mut Facing, &mut CurrentAction)>,
) {
    for (mut position, mut facing, mut action) in &mut agents {
        let Some(committed) = action.0.as_mut() else {
            continue;
        };
        if committed.phase != Phase::Walking {
            continue;
        }
        let Some(target) = committed.target_position else {
            tracing::warn!("movement::walk: Walking action with no target_position; skipping");
            continue;
        };

        let (next, new_facing) = step_toward(position.pos, target, WALK_SPEED_M_PER_TICK);
        position.pos = next;
        if new_facing != Vec2::ZERO {
            facing.dir = new_facing;
        }

        let dx = target.x - next.x;
        let dy = target.y - next.y;
        if dx * dx + dy * dy <= ARRIVE_EPSILON * ARRIVE_EPSILON {
            committed.phase = Phase::Performing;
            committed.started_tick = current_tick.0;
            committed.expected_end_tick =
                Some(current_tick.0 + u64::from(committed.perform_duration_ticks));
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;

    use crate::agent::{Facing, Position};
    use crate::decision::{
        ActionRef, CommittedAction, CurrentAction, Phase, SelfActionKind,
    };
    use crate::ids::{AdvertisementId, LeafAreaId, ObjectId};
    use crate::systems::movement::walk;
    use crate::time::CurrentTick;
    use crate::world::Vec2;

    fn build(start: Vec2, target: Vec2, perform: u32) -> (World, bevy_ecs::entity::Entity) {
        let mut world = World::new();
        world.insert_resource(CurrentTick(0));
        let agent = world
            .spawn((
                Position {
                    leaf: LeafAreaId::new(1),
                    pos: start,
                },
                Facing::default(),
                CurrentAction(Some(CommittedAction {
                    action: ActionRef::Object {
                        object: ObjectId::new(0),
                        ad: AdvertisementId::new(1),
                    },
                    started_tick: 0,
                    expected_end_tick: None,
                    phase: Phase::Walking,
                    target_position: Some(target),
                    target_spot: None,
                    target_facing: None,
                    perform_duration_ticks: perform,
                })),
            ))
            .id();
        (world, agent)
    }

    #[test]
    fn agent_walks_three_ticks_then_performs() {
        // distance = 200, speed = 80 → arrive at tick 3.
        let (mut world, agent) = build(Vec2::ZERO, Vec2::new(200.0, 0.0), 5);
        let mut sched = Schedule::default();
        sched.add_systems(walk);
        for tick in 1..=3 {
            *world.resource_mut::<CurrentTick>() = CurrentTick(tick);
            sched.run(&mut world);
        }
        let action = world
            .get::<CurrentAction>(agent)
            .unwrap()
            .0
            .clone()
            .unwrap();
        assert_eq!(action.phase, Phase::Performing);
        assert_eq!(action.started_tick, 3);
        assert_eq!(action.expected_end_tick, Some(3 + 5));
        let pos = world.get::<Position>(agent).unwrap().pos;
        assert!(
            (pos.x - 200.0).abs() < super::ARRIVE_EPSILON,
            "pos.x={}",
            pos.x
        );
    }

    #[test]
    fn arrival_within_one_tick_when_close() {
        let (mut world, agent) = build(Vec2::ZERO, Vec2::new(10.0, 0.0), 4);
        let mut sched = Schedule::default();
        sched.add_systems(walk);
        *world.resource_mut::<CurrentTick>() = CurrentTick(1);
        sched.run(&mut world);
        let action = world
            .get::<CurrentAction>(agent)
            .unwrap()
            .0
            .clone()
            .unwrap();
        assert_eq!(action.phase, Phase::Performing);
        assert_eq!(action.expected_end_tick, Some(1 + 4));
    }

    #[test]
    fn self_action_agent_never_enters_walk() {
        let mut world = World::new();
        world.insert_resource(CurrentTick(1));
        let agent = world
            .spawn((
                Position {
                    leaf: LeafAreaId::new(1),
                    pos: Vec2::ZERO,
                },
                Facing::default(),
                CurrentAction(Some(CommittedAction {
                    action: ActionRef::SelfAction(SelfActionKind::Idle),
                    started_tick: 0,
                    expected_end_tick: Some(5),
                    phase: Phase::Performing,
                    target_position: None,
                    target_spot: None,
                    target_facing: None,
                    perform_duration_ticks: 5,
                })),
            ))
            .id();
        let mut sched = Schedule::default();
        sched.add_systems(walk);
        sched.run(&mut world);
        let action = world
            .get::<CurrentAction>(agent)
            .unwrap()
            .0
            .clone()
            .unwrap();
        assert_eq!(action.phase, Phase::Performing);
        assert_eq!(action.expected_end_tick, Some(5)); // unchanged
    }
}
