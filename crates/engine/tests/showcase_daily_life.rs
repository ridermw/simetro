//! PR 15a — SL1 showcase: Daily-life delights.
//!
//! Six (well — seven, we couldn't resist the bicycle shop) bite-sized
//! SL1 scenes that exercise the full grammar in approachable everyday
//! settings. Each scene must:
//!
//! - Load cleanly (strict-schema, registry-backed scene_id).
//! - Tick to its declared `victory_conditions[].survive_until` tick and
//!   reach `GameOutcome::Won` without emitting any faults.
//! - Produce a stable deterministic hash matching its committed baseline
//!   in `tests/baselines/<slug>.hash`.
//!
//! These scenes intentionally use **only** SL1 primitives already
//! shipped through PR 14: places / links / things / transforms / one
//! demand / one pressure / objectives / one failure condition / one
//! victory condition / minimal observability / one mock observer agent
//! / two pressure-lifecycle milestones. They are designed to be
//! viewed-only, agent-operated, and demonstrably winnable on the
//! default deterministic seed.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{hash_run, load_scene_str, GameOutcome, TickRunner};
use simetro_protocol::SimMessage;

struct Showcase {
    slug: &'static str,
    scene_json: &'static str,
    baseline: &'static str,
    /// Tick at which the scene's `survive_until` victory condition
    /// fires. The deterministic hash is captured at this tick so the
    /// baseline is not coupled to post-terminal behavior.
    win_tick: u64,
}

const SEED: u64 = 42;

const SHOWCASES: &[Showcase] = &[
    Showcase {
        slug: "sandwich-shop",
        scene_json: include_str!("../../../games/sandwich-shop.json"),
        baseline: include_str!("../../../tests/baselines/sandwich-shop.hash"),
        win_tick: 1200,
    },
    Showcase {
        slug: "theme-park-day",
        scene_json: include_str!("../../../games/theme-park-day.json"),
        baseline: include_str!("../../../tests/baselines/theme-park-day.hash"),
        win_tick: 1800,
    },
    Showcase {
        slug: "school-lunch-line",
        scene_json: include_str!("../../../games/school-lunch-line.json"),
        baseline: include_str!("../../../tests/baselines/school-lunch-line.hash"),
        win_tick: 1320,
    },
    Showcase {
        slug: "coffee-roastery",
        scene_json: include_str!("../../../games/coffee-roastery.json"),
        baseline: include_str!("../../../tests/baselines/coffee-roastery.hash"),
        win_tick: 2400,
    },
    Showcase {
        slug: "library-checkout",
        scene_json: include_str!("../../../games/library-checkout.json"),
        baseline: include_str!("../../../tests/baselines/library-checkout.hash"),
        win_tick: 1800,
    },
    Showcase {
        slug: "farmers-market",
        scene_json: include_str!("../../../games/farmers-market.json"),
        baseline: include_str!("../../../tests/baselines/farmers-market.hash"),
        win_tick: 1500,
    },
    Showcase {
        slug: "bicycle-repair-shop",
        scene_json: include_str!("../../../games/bicycle-repair-shop.json"),
        baseline: include_str!("../../../tests/baselines/bicycle-repair-shop.hash"),
        win_tick: 1800,
    },
];

#[test]
fn each_showcase_scene_loads_with_full_sl1_grammar() {
    for case in SHOWCASES {
        let loaded = load_scene_str(case.scene_json, SEED)
            .unwrap_or_else(|err| panic!("{} should load: {err:?}", case.slug));

        let sl1 = loaded
            .world
            .sl1
            .as_ref()
            .unwrap_or_else(|| panic!("{} is an SL1 scene", case.slug));

        assert!(!sl1.places.is_empty(), "{} declares places", case.slug);
        assert!(!sl1.links.is_empty(), "{} declares links", case.slug);
        assert!(!sl1.things.is_empty(), "{} declares things", case.slug);
        assert!(
            !sl1.transforms.is_empty(),
            "{} declares transforms",
            case.slug
        );
        assert!(
            !sl1.demand.is_empty(),
            "{} declares at least one demand",
            case.slug
        );
        assert!(
            !sl1.pressure.is_empty(),
            "{} declares at least one pressure event",
            case.slug
        );
        assert!(
            !sl1.objectives.is_empty(),
            "{} declares at least one objective",
            case.slug
        );
        assert!(
            !sl1.failure_conditions.is_empty(),
            "{} declares at least one failure condition",
            case.slug
        );
        assert!(
            !sl1.victory_conditions.is_empty(),
            "{} declares at least one victory condition",
            case.slug
        );
        let obs = sl1
            .observability
            .as_ref()
            .unwrap_or_else(|| panic!("{} declares observability", case.slug));
        assert!(
            !obs.metrics.is_empty(),
            "{} declares at least one metric",
            case.slug
        );
        assert!(
            !obs.dashboards.is_empty(),
            "{} declares at least one dashboard",
            case.slug
        );
        assert!(
            !obs.alerts.is_empty(),
            "{} declares at least one alert",
            case.slug
        );
        assert!(
            !sl1.agents.is_empty(),
            "{} declares at least one agent",
            case.slug
        );
        assert!(
            !sl1.milestones.is_empty(),
            "{} declares at least one milestone",
            case.slug
        );
    }
}

#[test]
fn each_showcase_scene_reaches_won_at_victory_tick() {
    for case in SHOWCASES {
        let mut scene = load_scene_str(case.scene_json, SEED)
            .unwrap_or_else(|err| panic!("{} should load: {err:?}", case.slug));
        let mut world = std::mem::take(&mut scene.world);
        let mut runner = TickRunner::new();
        let mut faults: Vec<String> = Vec::new();
        let mut terminal_tick: Option<u64> = None;

        // Run a generous budget beyond the declared win tick so a
        // late-firing failure condition cannot disguise itself as a
        // win.
        let budget = case.win_tick + 200;
        for tick in 1..=budget {
            runner.tick_once(&mut world);
            for msg in runner.messages() {
                if let SimMessage::Fault(payload) = msg {
                    faults.push(format!("{payload:?}"));
                }
            }
            if world.sl1_outcome().is_terminal() {
                terminal_tick = Some(tick);
                break;
            }
        }

        assert!(
            faults.is_empty(),
            "{} should not emit faults on the winning path, got: {faults:#?}",
            case.slug,
        );
        assert_eq!(
            world.sl1_outcome(),
            GameOutcome::Won,
            "{} should reach GameOutcome::Won within {budget} ticks",
            case.slug,
        );
        assert_eq!(
            terminal_tick,
            Some(case.win_tick),
            "{} should reach GameOutcome::Won exactly at its declared survive_until tick",
            case.slug,
        );
    }
}

#[test]
fn each_showcase_scene_matches_deterministic_hash_baseline() {
    for case in SHOWCASES {
        let mut scene = load_scene_str(case.scene_json, SEED)
            .unwrap_or_else(|err| panic!("{} should load: {err:?}", case.slug));
        let mut world = std::mem::take(&mut scene.world);
        let mut runner = TickRunner::new();
        let hash = hash_run(&mut world, &mut runner, case.win_tick);
        let expected = case.baseline.trim();
        assert_eq!(
            hash, expected,
            "{} deterministic hash drifted; if intentional, update tests/baselines/{}.hash",
            case.slug, case.slug,
        );
    }
}

#[test]
fn each_showcase_scene_is_stable_across_two_runs() {
    for case in SHOWCASES {
        let mut scene1 = load_scene_str(case.scene_json, SEED)
            .unwrap_or_else(|err| panic!("{} loads: {err:?}", case.slug));
        let mut world1 = std::mem::take(&mut scene1.world);
        let mut runner1 = TickRunner::new();
        let h1 = hash_run(&mut world1, &mut runner1, case.win_tick);

        let mut scene2 = load_scene_str(case.scene_json, SEED)
            .unwrap_or_else(|err| panic!("{} loads: {err:?}", case.slug));
        let mut world2 = std::mem::take(&mut scene2.world);
        let mut runner2 = TickRunner::new();
        let h2 = hash_run(&mut world2, &mut runner2, case.win_tick);

        assert_eq!(
            h1, h2,
            "{} deterministic hash should be stable across two identical runs",
            case.slug,
        );
    }
}
