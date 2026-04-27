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
