use std::path::PathBuf;

use gecko_sim_core::decision::Phase;
use gecko_sim_core::{Vec2, WorldGraph};
use gecko_sim_host::demo::build_demo_sim;

fn workspace_content_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("content")
}

fn in_living_room(pos: Vec2) -> bool {
    let graph = WorldGraph::seed_v0();
    let living_room = graph
        .leaf(graph.default_spawn_leaf)
        .expect("seed world has a default spawn leaf");
    pos.x >= living_room.bbox.min.x
        && pos.x <= living_room.bbox.max.x
        && pos.y >= living_room.bbox.min.y
        && pos.y <= living_room.bbox.max.y
}

#[test]
fn demo_seed_spreads_agents_and_objects_inside_the_living_room() {
    let content =
        gecko_sim_content::load_from_dir(&workspace_content_dir()).expect("seed content loads");
    let sim = build_demo_sim(content);
    let snap = sim.snapshot();

    assert_eq!(snap.agents.len(), 3);
    assert_eq!(snap.objects.len(), 3);
    assert!(snap.agents.iter().all(|agent| in_living_room(agent.pos)));
    assert!(snap.objects.iter().all(|object| in_living_room(object.pos)));
    assert_eq!(snap.agents[0].pos, Vec2::new(84.0, 84.0));
    assert_eq!(snap.agents[1].pos, Vec2::new(116.0, 84.0));
    assert_eq!(snap.agents[2].pos, Vec2::new(100.0, 116.0));
}

#[test]
fn demo_seed_gives_geckos_distinct_room_targets() {
    let content =
        gecko_sim_content::load_from_dir(&workspace_content_dir()).expect("seed content loads");
    let mut sim = build_demo_sim(content);

    sim.tick();
    let snap = sim.snapshot();
    let walking_targets: Vec<Vec2> = snap
        .agents
        .iter()
        .filter(|agent| agent.action_phase == Some(Phase::Walking))
        .filter_map(|agent| {
            agent
                .current_action
                .as_ref()
                .and_then(|action| action.target_position)
        })
        .collect();

    assert_eq!(walking_targets.len(), 3);
    assert!(walking_targets.contains(&Vec2::new(116.0, 115.0)));
    assert!(walking_targets.contains(&Vec2::new(88.0, 111.25)));
    assert!(walking_targets.contains(&Vec2::new(112.0, 95.25)));
}
