//! End-to-end happy-path test: write a tempdir of valid RON files, call
//! `load_from_dir`, assert the bundle has the expected entries.

use std::fs;

use gecko_sim_content::load_from_dir;
use gecko_sim_core::ids::{AccessoryId, ObjectTypeId};

const FRIDGE_RON: &str = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: { "stocked": Bool(true) },
    advertisements: [
        Advertisement(
            id: AdvertisementId(1),
            display_name: "Eat snack",
            preconditions: [
                ObjectState("stocked", Eq, Bool(true)),
                AgentNeed(Hunger, Lt, 0.6),
            ],
            effects: [
                AgentNeedDelta(Hunger, 0.4),
            ],
            duration_ticks: 10,
            interrupt_class: NeedsThresholdOnly,
            score_template: ScoreTemplate(
                need_weights: [(Hunger, 1.0)],
                personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
    ],
)
"#;

const SUNGLASSES_RON: &str = r#"
Accessory(
    id: AccessoryId(1),
    display_name: "Sunglasses",
    mesh_id: MeshId(101),
    slot: Head,
)
"#;

#[test]
fn load_from_dir_happy_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let object_types_dir = tmp.path().join("object_types");
    let accessories_dir = tmp.path().join("accessories");
    fs::create_dir(&object_types_dir).unwrap();
    fs::create_dir(&accessories_dir).unwrap();
    fs::write(object_types_dir.join("fridge.ron"), FRIDGE_RON).unwrap();
    fs::write(accessories_dir.join("sunglasses.ron"), SUNGLASSES_RON).unwrap();

    let bundle = load_from_dir(tmp.path()).expect("load");

    assert_eq!(bundle.object_types.len(), 1);
    assert!(bundle.object_types.contains_key(&ObjectTypeId::new(1)));
    assert_eq!(
        bundle.object_types[&ObjectTypeId::new(1)].display_name,
        "Fridge"
    );

    assert_eq!(bundle.accessories.len(), 1);
    assert!(bundle.accessories.contains_key(&AccessoryId::new(1)));
}

#[test]
fn load_from_dir_with_missing_subdirs_returns_empty_bundle() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bundle = load_from_dir(tmp.path()).expect("load empty");
    assert!(bundle.object_types.is_empty());
    assert!(bundle.accessories.is_empty());
}

#[test]
fn load_from_dir_skips_non_ron_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let object_types_dir = tmp.path().join("object_types");
    fs::create_dir(&object_types_dir).unwrap();
    fs::write(object_types_dir.join("fridge.ron"), FRIDGE_RON).unwrap();
    fs::write(object_types_dir.join("README.md"), "ignore me").unwrap();
    fs::write(object_types_dir.join(".DS_Store"), "junk").unwrap();

    let bundle = load_from_dir(tmp.path()).expect("load");
    assert_eq!(bundle.object_types.len(), 1);
}
