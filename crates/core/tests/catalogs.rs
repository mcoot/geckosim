//! Smoke test: the new catalog resources derive `Resource` and can be
//! inserted into a `bevy_ecs::World`.

use std::collections::HashMap;

use bevy_ecs::world::World;
use gecko_sim_core::agent::{Accessory, AccessoryCatalog, AccessorySlot};
use gecko_sim_core::ids::{AccessoryId, ObjectTypeId};
use gecko_sim_core::object::{MeshId, ObjectCatalog, ObjectType};

#[test]
fn object_catalog_resource_inserts() {
    let mut world = World::new();
    let mut by_id = HashMap::new();
    by_id.insert(
        ObjectTypeId::new(1),
        ObjectType {
            id: ObjectTypeId::new(1),
            display_name: "Fridge".into(),
            mesh_id: MeshId(1),
            default_state: HashMap::new(),
            interaction_spots: vec![],
            advertisements: vec![],
        },
    );
    world.insert_resource(ObjectCatalog { by_id });
    let res = world
        .get_resource::<ObjectCatalog>()
        .expect("ObjectCatalog inserted");
    assert_eq!(res.by_id.len(), 1);
}

#[test]
fn accessory_catalog_resource_inserts() {
    let mut world = World::new();
    let mut by_id = HashMap::new();
    by_id.insert(
        AccessoryId::new(1),
        Accessory {
            id: AccessoryId::new(1),
            display_name: "Sunglasses".into(),
            mesh_id: MeshId(101),
            slot: AccessorySlot::Head,
        },
    );
    world.insert_resource(AccessoryCatalog { by_id });
    let res = world
        .get_resource::<AccessoryCatalog>()
        .expect("AccessoryCatalog inserted");
    assert_eq!(res.by_id.len(), 1);
}

#[test]
fn accessory_slot_round_trips_via_ron() {
    let v = AccessorySlot::Neck;
    let s = ron::to_string(&v).expect("serialize");
    let back: AccessorySlot = ron::from_str(&s).expect("deserialize");
    assert_eq!(v, back);
}

#[test]
fn sim_new_inserts_object_and_accessory_catalogs_from_bundle() {
    use gecko_sim_core::{ContentBundle, Sim};

    let mut object_types = HashMap::new();
    object_types.insert(
        ObjectTypeId::new(7),
        ObjectType {
            id: ObjectTypeId::new(7),
            display_name: "Chair".into(),
            mesh_id: MeshId(2),
            default_state: HashMap::new(),
            interaction_spots: vec![],
            advertisements: vec![],
        },
    );
    let mut accessories = HashMap::new();
    accessories.insert(
        AccessoryId::new(9),
        Accessory {
            id: AccessoryId::new(9),
            display_name: "Bow tie".into(),
            mesh_id: MeshId(102),
            slot: AccessorySlot::Neck,
        },
    );
    let bundle = ContentBundle {
        object_types,
        accessories,
    };

    let sim = Sim::new(0, bundle);

    // The catalogs are exposed for tests via dedicated accessors.
    assert_eq!(sim.object_catalog().by_id.len(), 1);
    assert!(sim
        .object_catalog()
        .by_id
        .contains_key(&ObjectTypeId::new(7)));
    assert_eq!(sim.accessory_catalog().by_id.len(), 1);
    assert!(sim
        .accessory_catalog()
        .by_id
        .contains_key(&AccessoryId::new(9)));
}

#[test]
fn sim_new_with_default_bundle_has_empty_catalogs() {
    use gecko_sim_core::{ContentBundle, Sim};

    let sim = Sim::new(0, ContentBundle::default());
    assert!(sim.object_catalog().by_id.is_empty());
    assert!(sim.accessory_catalog().by_id.is_empty());
}
