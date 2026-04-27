//! One test per `ContentError` validation variant. Each test writes a
//! tempdir that triggers the variant, calls `load_from_dir`, and matches
//! the resulting `Err` by variant.

use std::fs;
use std::path::Path;

use gecko_sim_content::{load_from_dir, ContentError};

fn write_object_type(root: &Path, name: &str, contents: &str) {
    let dir = root.join("object_types");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(name), contents).unwrap();
}

fn write_accessory(root: &Path, name: &str, contents: &str) {
    let dir = root.join("accessories");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(name), contents).unwrap();
}

const FRIDGE_OK: &str = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: { "stocked": Bool(true) },
    advertisements: [
        Advertisement(
            id: AdvertisementId(1),
            display_name: "Eat snack",
            preconditions: [],
            effects: [AgentNeedDelta(Hunger, 0.4)],
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

#[test]
fn parse_error_on_malformed_ron() {
    let tmp = tempfile::tempdir().unwrap();
    write_object_type(tmp.path(), "broken.ron", "this is not RON");
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::Parse { .. }),
        "got {err:?}"
    );
}

#[test]
fn duplicate_object_type_id_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    write_object_type(tmp.path(), "fridge.ron", FRIDGE_OK);
    write_object_type(tmp.path(), "also_fridge.ron", FRIDGE_OK);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    let ContentError::DuplicateObjectTypeId { first, second, .. } = err else {
        panic!("got {err:?}");
    };
    // The two paths must be distinct and both end in their source filenames
    // — the bisection guarantee of this error.
    assert_ne!(first, second);
    let names: std::collections::HashSet<_> = [&first, &second]
        .into_iter()
        .map(|p| p.file_name().unwrap().to_owned())
        .collect();
    assert!(names.contains(std::ffi::OsStr::new("fridge.ron")));
    assert!(names.contains(std::ffi::OsStr::new("also_fridge.ron")));
}

#[test]
fn duplicate_accessory_id_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
Accessory(
    id: AccessoryId(7),
    display_name: "Sunglasses",
    mesh_id: MeshId(101),
    slot: Head,
)
"#;
    write_accessory(tmp.path(), "a.ron", body);
    write_accessory(tmp.path(), "b.ron", body);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::DuplicateAccessoryId { .. }),
        "got {err:?}"
    );
}

#[test]
fn duplicate_advertisement_id_within_object_type_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: {},
    advertisements: [
        Advertisement(
            id: AdvertisementId(1), display_name: "A",
            preconditions: [], effects: [],
            duration_ticks: 1, interrupt_class: Never,
            score_template: ScoreTemplate(
                need_weights: [], personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
        Advertisement(
            id: AdvertisementId(1), display_name: "B",
            preconditions: [], effects: [],
            duration_ticks: 1, interrupt_class: Never,
            score_template: ScoreTemplate(
                need_weights: [], personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
    ],
)
"#;
    write_object_type(tmp.path(), "fridge.ron", body);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::DuplicateAdvertisementId { .. }),
        "got {err:?}"
    );
}

#[test]
fn unknown_object_state_key_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: { "stocked": Bool(true) },
    advertisements: [
        Advertisement(
            id: AdvertisementId(1), display_name: "A",
            preconditions: [ ObjectState("missing", Eq, Bool(true)) ],
            effects: [],
            duration_ticks: 1, interrupt_class: Never,
            score_template: ScoreTemplate(
                need_weights: [], personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
    ],
)
"#;
    write_object_type(tmp.path(), "fridge.ron", body);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::UnknownObjectStateKey { .. }),
        "got {err:?}"
    );
}

#[test]
fn duplicate_need_weight_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: {},
    advertisements: [
        Advertisement(
            id: AdvertisementId(1), display_name: "A",
            preconditions: [], effects: [],
            duration_ticks: 1, interrupt_class: Never,
            score_template: ScoreTemplate(
                need_weights: [(Hunger, 1.0), (Hunger, 0.5)],
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
    write_object_type(tmp.path(), "fridge.ron", body);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::DuplicateNeedWeight { .. }),
        "got {err:?}"
    );
}

#[test]
fn zero_duration_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: {},
    advertisements: [
        Advertisement(
            id: AdvertisementId(1), display_name: "A",
            preconditions: [], effects: [],
            duration_ticks: 0, interrupt_class: Never,
            score_template: ScoreTemplate(
                need_weights: [], personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
    ],
)
"#;
    write_object_type(tmp.path(), "fridge.ron", body);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::ZeroDuration { .. }),
        "got {err:?}"
    );
}
