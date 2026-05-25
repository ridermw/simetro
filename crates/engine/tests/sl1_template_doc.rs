//! Integration test: the SL1 author template (`docs/sl1-template.jsonc`)
//! always parses and loads.
//!
//! The template is the canonical author surface for scenario_language_v1.
//! If it drifts from the strict schema, every author copying it will hit
//! the load-rejection paths instead of getting a runnable scene. This
//! test strips `//` comments, parses as JSON, and feeds the result
//! through the engine's `load_scene_str` to catch drift at CI time
//! rather than at author-copy time.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::load_scene_str;

/// Strip `//`-to-end-of-line comments from a JSONC string. Respects
/// string literals (does not strip `//` inside `"..."`) and escape
/// sequences. Does not strip `/* ... */` block comments — the SL1
/// template only uses line comments. Trailing commas are not produced
/// by the template, so they are not handled here.
fn strip_line_comments(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut in_str = false;
    let mut esc = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            out.push(c as char);
            if esc {
                esc = false;
            } else if c == b'\\' {
                esc = true;
            } else if c == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == b'"' {
            in_str = true;
            out.push('"');
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        out.push(c as char);
        i += 1;
    }
    out
}

#[test]
fn sl1_template_jsonc_loads_with_engine_loader() {
    let jsonc = include_str!("../../../docs/sl1-template.jsonc");
    let stripped = strip_line_comments(jsonc);
    // Smoke-check that the strip removed at least one comment line so
    // the test would notice if the file ever went 100% JSON (in which
    // case the strip pass would be a no-op but we still want to load
    // it; this assertion just catches accidental refactors).
    assert!(
        jsonc.contains("//"),
        "docs/sl1-template.jsonc is expected to be JSONC with `//` comments"
    );
    assert!(
        !stripped.contains("// "),
        "strip_line_comments left behind a `// ` token; comment stripper regressed"
    );

    let loaded = load_scene_str(&stripped, 42).expect("docs/sl1-template.jsonc must load cleanly");
    assert_eq!(loaded.name, "sl1-template");

    let sl1 = loaded
        .sl1
        .as_ref()
        .expect("template must include a scenario_language_v1 block");
    assert!(!sl1.places.is_empty(), "template should declare places");
    assert!(!sl1.links.is_empty(), "template should declare links");
    assert!(!sl1.things.is_empty(), "template should declare things");
    assert!(
        !sl1.transforms.is_empty(),
        "template should declare transforms"
    );
    assert!(!sl1.demand.is_empty(), "template should declare demand");
    assert!(!sl1.pressure.is_empty(), "template should declare pressure");
    assert!(
        !sl1.objectives.is_empty(),
        "template should declare objectives"
    );
    assert!(
        !sl1.failure_conditions.is_empty(),
        "template should declare failure_conditions"
    );
    assert!(
        !sl1.victory_conditions.is_empty(),
        "template should declare victory_conditions"
    );
    assert!(
        sl1.observability.is_some(),
        "template should declare an observability block"
    );
    assert!(!sl1.agents.is_empty(), "template should declare agents");
    assert!(
        !sl1.milestones.is_empty(),
        "template should declare milestones"
    );
}
