//! Polished-world quality gate for authored v1 JSON scenes.
//!
//! This is intentionally stricter than the runtime loader: the loader
//! validates safety and compatibility, while this test validates the
//! authoring checklist documented in `docs/world-quality.md`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
#[cfg(not(feature = "llm-live"))]
use simetro_engine::LoadError;
use simetro_engine::{load_scene_str, LoadedScene};

#[test]
fn games_are_polished_v1_worlds() {
    let mut world_slugs = BTreeSet::new();

    for path in world_files() {
        let slug = validate_world(&path);
        assert!(
            world_slugs.insert(slug.clone()),
            "duplicate world slug in games/: {slug}",
        );
    }

    validate_frontend_catalog(&world_slugs);
    validate_tauri_registry(&world_slugs);
}

fn world_files() -> Vec<PathBuf> {
    let dir = repo_root().join("games");

    let mut files = fs::read_dir(&dir)
        .expect("games directory should exist")
        .map(|entry| entry.expect("games entry should be readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<_>>();
    files.sort();
    assert!(
        !files.is_empty(),
        "expected at least one JSON world in games/"
    );
    files
}

fn repo_root() -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop();
    root.pop();
    root
}

fn validate_world(path: &Path) -> String {
    let label = path.display().to_string();
    let src = fs::read_to_string(path).unwrap_or_else(|err| panic!("{label}: read failed: {err}"));
    let value = serde_json::from_str::<Value>(&src)
        .unwrap_or_else(|err| panic!("{label}: JSON parse failed: {err}"));

    assert_eq!(
        value.get("schema_version").and_then(Value::as_u64),
        Some(1),
        "{label}: polished worlds must remain schema_version 1",
    );

    let slug = validate_catalog(path, &value);
    let loaded = match load_scene_str(&src, 42) {
        Ok(loaded) => loaded,
        #[cfg(not(feature = "llm-live"))]
        Err(LoadError::AgentKindRequiresFeature { kind, .. }) if kind == "llm" => return slug,
        Err(err) => panic!("{label}: v1 loader rejected world: {err}"),
    };
    validate_palette(&label, &loaded);

    // SL1 scenes carry their game-bearing structure in
    // `scenario_language_v1` and leave `pieces.{nodes,paths,movers}`
    // intentionally empty (the SL1 frontend renderer ships later).
    // Apply SL1-specific topology checks instead of the legacy
    // node-shape/mover-path validators.
    if value.get("scenario_language_v1").is_some() {
        validate_sl1_topology(&label, &value, &loaded);
    } else {
        validate_layout_silhouette(&label, &loaded);
        validate_node_language(&label, &value, &loaded);
        validate_mover_paths(&label, &loaded);
    }

    slug
}

fn validate_catalog(path: &Path, value: &Value) -> String {
    let label = path.display();
    let catalog = value
        .get("catalog")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{label}: missing catalog object"));

    for field in ["slug", "title", "version", "author", "description"] {
        let text = catalog
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{label}: catalog.{field} must be a string"));
        assert!(
            !text.trim().is_empty(),
            "{label}: catalog.{field} must not be empty",
        );
    }

    let slug = catalog
        .get("slug")
        .and_then(Value::as_str)
        .expect("catalog.slug checked above");
    let file_stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("world filename should be UTF-8");
    assert_eq!(
        slug, file_stem,
        "{label}: catalog.slug should match the world filename",
    );
    assert!(
        is_catalog_slug(slug),
        "{label}: catalog.slug must use lowercase kebab-case/underscore characters",
    );

    let tags = catalog
        .get("tags")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{label}: catalog.tags must be an array"));
    assert!(!tags.is_empty(), "{label}: catalog.tags must not be empty");
    assert!(
        tags.iter()
            .all(|tag| tag.as_str().is_some_and(|text| !text.trim().is_empty())),
        "{label}: catalog.tags must contain only non-empty strings",
    );

    for field in [
        "palette_note",
        "layout_note",
        "node_language_note",
        "mover_path_note",
    ] {
        let note = catalog
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{label}: catalog.{field} must be a string"));
        assert!(
            !note.trim().is_empty(),
            "{label}: catalog.{field} must not be empty",
        );
    }

    let has_review_note = catalog
        .get("review_note")
        .and_then(Value::as_str)
        .is_some_and(|note| !note.trim().is_empty());
    let has_screenshot = catalog
        .get("screenshot")
        .and_then(Value::as_str)
        .is_some_and(|screenshot| !screenshot.trim().is_empty());
    assert!(
        has_review_note || has_screenshot,
        "{label}: catalog needs a screenshot reference or review_note",
    );

    slug.to_string()
}

fn validate_frontend_catalog(world_slugs: &BTreeSet<String>) {
    let path = repo_root().join("frontend/src/catalog/scenes.ts");
    let label = path.display().to_string();
    let source =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("{label}: read failed: {err}"));
    let catalog_ids = extract_ts_catalog_ids(&source);

    assert_eq!(
        &catalog_ids, world_slugs,
        "{label}: SCENE_CATALOG ids must match games/*.json catalog slugs; add one defineScene entry per polished world",
    );
}

fn validate_tauri_registry(world_slugs: &BTreeSet<String>) {
    let path = repo_root().join("src-tauri/src/scene_registry.rs");
    let label = path.display().to_string();
    let source =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("{label}: read failed: {err}"));
    let registry_ids = extract_scene_registry_ids(&source);

    assert_eq!(
        &registry_ids, world_slugs,
        "{label}: SCENE_ENTRIES must match games/*.json catalog slugs; add one scene_entry!(\"slug\") per polished world",
    );
}

fn extract_ts_catalog_ids(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let after_prefix = trimmed.strip_prefix("id: \"")?;
            let id = after_prefix.split_once('"')?.0;
            Some(id.to_string())
        })
        .collect()
}

fn extract_scene_registry_ids(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let (_, after_prefix) = line.split_once("scene_entry!(\"")?;
            let id = after_prefix.split_once('"')?.0;
            Some(id.to_string())
        })
        .collect()
}

fn is_catalog_slug(slug: &str) -> bool {
    let mut chars = slug.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
}

fn validate_palette(label: &str, loaded: &LoadedScene) {
    let palette = &loaded.theme.palette;
    assert!(
        palette.len() >= 5,
        "{label}: palette should include background, foreground, and at least three accents",
    );

    let unique = palette
        .iter()
        .map(|color| color.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        unique.len(),
        palette.len(),
        "{label}: palette contains duplicate colors",
    );

    let rgb = palette
        .iter()
        .map(|color| hex_rgb(color))
        .collect::<Vec<_>>();
    for (i, a) in rgb.iter().enumerate() {
        for (j, b) in rgb.iter().enumerate().skip(i + 1) {
            let distance_squared = color_distance_squared(*a, *b);
            assert!(
                distance_squared >= 45 * 45,
                "{label}: palette colors {i} and {j} are too similar",
            );
        }
    }
}

fn validate_layout_silhouette(label: &str, loaded: &LoadedScene) {
    let positions = loaded
        .world
        .nodes
        .values()
        .map(|node| node.pos)
        .collect::<Vec<_>>();
    assert!(
        positions.len() >= 3,
        "{label}: layout needs at least three nodes for a readable silhouette",
    );

    let (min_x, max_x, min_y, max_y) = positions.iter().fold(
        (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), [x, y]| {
            (min_x.min(*x), max_x.max(*x), min_y.min(*y), max_y.max(*y))
        },
    );
    let width = max_x - min_x;
    let height = max_y - min_y;
    assert!(
        width >= 100.0 && height >= 100.0,
        "{label}: layout silhouette is too small or line-like",
    );

    let mut max_triangle_area = 0.0_f32;
    for (i, a) in positions.iter().enumerate() {
        for (j, b) in positions.iter().enumerate().skip(i + 1) {
            for c in positions.iter().skip(j + 1) {
                max_triangle_area = max_triangle_area.max(triangle_area(*a, *b, *c));
            }
        }
    }
    assert!(
        max_triangle_area >= 10_000.0,
        "{label}: nodes do not form a distinct two-dimensional silhouette",
    );
}

fn validate_node_language(label: &str, value: &Value, loaded: &LoadedScene) {
    let nodes = value
        .pointer("/pieces/nodes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{label}: pieces.nodes must be an array"));

    let shapes = nodes
        .iter()
        .filter_map(|node| node.get("shape").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    assert!(
        shapes.len() >= 3,
        "{label}: use at least three node shapes intentionally",
    );

    let colors = loaded
        .world
        .nodes
        .values()
        .map(|node| node.color)
        .collect::<BTreeSet<_>>();
    assert!(
        colors.len() >= 3,
        "{label}: use at least three node colors intentionally",
    );

    assert!(
        !colors.contains(&loaded.theme.background_index),
        "{label}: node colors should not use the background color",
    );
}

fn validate_mover_paths(label: &str, loaded: &LoadedScene) {
    assert!(
        !loaded.world.movers.is_empty(),
        "{label}: polished worlds should include at least one mover",
    );

    let node_colors = loaded
        .world
        .nodes
        .values()
        .map(|node| node.color)
        .collect::<BTreeSet<_>>();
    let mut home_paths = BTreeSet::new();

    for mover in loaded.world.movers.values() {
        home_paths.insert(mover.home_path);
        let path = loaded
            .world
            .paths
            .get(&mover.home_path)
            .unwrap_or_else(|| panic!("{label}: mover references a missing home path"));
        let from = loaded
            .world
            .nodes
            .get(&path.from)
            .unwrap_or_else(|| panic!("{label}: path references a missing from-node"));
        let to = loaded
            .world
            .nodes
            .get(&path.to)
            .unwrap_or_else(|| panic!("{label}: path references a missing to-node"));

        assert!(
            path_length(from.pos, to.pos) >= 50.0,
            "{label}: mover home path {:?} is too short to be visually meaningful",
            path.id,
        );
        assert_ne!(
            path.color, loaded.theme.background_index,
            "{label}: mover home path {:?} uses the background color",
            path.id,
        );
        assert!(
            node_colors.contains(&path.color),
            "{label}: mover home path {:?} should share an intentional node accent color",
            path.id,
        );
    }

    assert!(
        home_paths.len() >= loaded.world.movers.len().min(3),
        "{label}: first mover paths should demonstrate at least three distinct routes",
    );
}

fn hex_rgb(color: &str) -> [i32; 3] {
    [
        i32::from_str_radix(&color[1..3], 16).expect("loader validated hex color"),
        i32::from_str_radix(&color[3..5], 16).expect("loader validated hex color"),
        i32::from_str_radix(&color[5..7], 16).expect("loader validated hex color"),
    ]
}

fn color_distance_squared(a: [i32; 3], b: [i32; 3]) -> i32 {
    let dr = a[0] - b[0];
    let dg = a[1] - b[1];
    let db = a[2] - b[2];
    dr * dr + dg * dg + db * db
}

fn triangle_area(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5
}

fn path_length(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    (dx * dx + dy * dy).sqrt()
}

/// SL1 polished-world validator. Replaces the legacy node/mover
/// validators for scenes that carry their game-bearing structure in
/// `scenario_language_v1`. Loader-level reference validation is NOT
/// duplicated here (see `crates/engine/src/scenario_language_v1.rs`);
/// these assertions only enforce authored topology quality.
fn validate_sl1_topology(label: &str, value: &Value, loaded: &LoadedScene) {
    let sl1 = loaded.world.sl1.as_ref().unwrap_or_else(|| {
        panic!("{label}: scenario_language_v1 present in source but missing from loaded scene")
    });

    // SL1 scenes intentionally leave the legacy pieces arrays empty so
    // the legacy renderer does not draw stale geometry. Lock that in.
    for legacy in ["nodes", "paths", "movers"] {
        let arr = value
            .pointer(&format!("/pieces/{legacy}"))
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{label}: pieces.{legacy} must be an array"));
        assert!(
            arr.is_empty(),
            "{label}: SL1 scenes must leave pieces.{legacy} empty (SL1 rendering ships separately)",
        );
    }

    assert!(
        sl1.places.len() >= 3,
        "{label}: SL1 scenes need at least three places for a readable topology",
    );

    let positions: Vec<[f32; 2]> = sl1.places.iter().map(|p| [p.pos[0], p.pos[1]]).collect();
    let (min_x, max_x, min_y, max_y) = positions.iter().fold(
        (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), [x, y]| {
            (min_x.min(*x), max_x.max(*x), min_y.min(*y), max_y.max(*y))
        },
    );
    let width = max_x - min_x;
    let height = max_y - min_y;
    assert!(
        width >= 100.0 && height >= 100.0,
        "{label}: SL1 place silhouette is too small or line-like",
    );

    let mut max_triangle_area = 0.0_f32;
    for (i, a) in positions.iter().enumerate() {
        for (j, b) in positions.iter().enumerate().skip(i + 1) {
            for c in positions.iter().skip(j + 1) {
                max_triangle_area = max_triangle_area.max(triangle_area(*a, *b, *c));
            }
        }
    }
    assert!(
        max_triangle_area >= 10_000.0,
        "{label}: SL1 places do not form a distinct two-dimensional silhouette",
    );

    let roles: BTreeSet<&str> = sl1.places.iter().map(|p| p.role.as_str()).collect();
    assert!(
        roles.len() >= 2,
        "{label}: SL1 places should use at least two distinct roles for visual language",
    );
    assert!(
        sl1.places.iter().all(|p| !p.role.trim().is_empty()),
        "{label}: every SL1 place must declare a non-empty role",
    );

    assert!(
        sl1.links.len() >= 2,
        "{label}: SL1 scenes need at least two links to communicate topology",
    );

    assert!(
        !sl1.things.is_empty(),
        "{label}: SL1 scenes need at least one declared thing",
    );
    assert!(
        !sl1.transforms.is_empty(),
        "{label}: SL1 scenes need at least one transform to drive deterministic behavior",
    );
    assert!(
        !sl1.demand.is_empty(),
        "{label}: SL1 scenes need at least one demand entry so the run has observable stakes",
    );
}
