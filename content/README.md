# Content

RON catalogs loaded by `gecko-sim-content::load_from_dir`. See ADR 0011
("Authoring format") for the schema and ADR 0012 ("crates/content") for
the loader's place in the architecture.

## Layout

```
content/
├── object_types/   one ObjectType per *.ron file
└── accessories/    one Accessory per *.ron file
```

Files within each subdirectory are loaded in lexicographic filename order
for deterministic results across platforms. Each file holds exactly one
top-level value of the appropriate type.

## Validation

The loader rejects:
- duplicate `ObjectTypeId` or `AccessoryId` across files
- duplicate `AdvertisementId` within a single `ObjectType`
- `Predicate::ObjectState` keys not present in the type's `default_state`
- duplicate `Need` entries in a `ScoreTemplate.need_weights`
- `duration_ticks: 0`
