// frontend/src/catalog/scenes.ts
//
// Static gallery metadata for local scenes. These strings are content, not
// markup; future UI should render them with textContent only.

export type SceneWorldKind = "transit_loop" | "sl1_scenario";
export type SceneDifficulty = "intro" | "easy" | "medium" | "hard";
export type SceneStatus = "ready" | "draft" | "planned";

export interface ScreenshotTarget {
  readonly route: string;
  readonly selector: "#scene";
  readonly wait_for: "first_frame";
  readonly width: number;
  readonly height: number;
}

export interface SceneCatalogEntry {
  readonly id: string;
  readonly title: string;
  readonly subtitle: string;
  readonly world_kind: SceneWorldKind;
  readonly scene_path: string;
  readonly difficulty: SceneDifficulty;
  readonly palette_name: string;
  readonly rules_summary: readonly string[];
  readonly visual_notes: readonly string[];
  readonly screenshot_target: ScreenshotTarget;
  readonly status: SceneStatus;
}

export type SceneCatalogInput<Id extends string> = Omit<
  SceneCatalogEntry,
  "id" | "scene_path" | "screenshot_target"
> & {
  readonly id: Id;
  readonly screenshot_target?: ScreenshotTarget;
};

export const DEFAULT_SCREENSHOT_TARGET = {
  route: "/",
  selector: "#scene",
  wait_for: "first_frame",
  width: 960,
  height: 540,
} as const satisfies ScreenshotTarget;

export function scenePathForId<Id extends string>(id: Id): `games/${Id}.json` {
  return `games/${id}.json`;
}

export function defineScene<const Id extends string>(
  scene: SceneCatalogInput<Id>
): SceneCatalogEntry & { readonly id: Id; readonly scene_path: `games/${Id}.json` } {
  return {
    ...scene,
    scene_path: scenePathForId(scene.id),
    screenshot_target: scene.screenshot_target ?? DEFAULT_SCREENSHOT_TARGET,
  };
}

export function catalogConventionErrors(scenes: readonly SceneCatalogEntry[]): string[] {
  const errors: string[] = [];
  const ids = new Set<string>();

  for (const scene of scenes) {
    if (ids.has(scene.id)) {
      errors.push(`duplicate scene id: ${scene.id}`);
    }
    ids.add(scene.id);

    const expectedPath = scenePathForId(scene.id);
    if (scene.scene_path !== expectedPath) {
      errors.push(`${scene.id}: scene_path must be ${expectedPath}`);
    }
    if (!isLocalScenePath(scene.scene_path)) {
      errors.push(`${scene.id}: scene_path must be a local games/*.json path`);
    }
  }

  return errors;
}

export const SCENE_CATALOG = [
  defineScene({
    id: "demo-paths",
    title: "Demo Paths",
    subtitle: "A compact loop that proves the renderer, movement, and inspector pipeline.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "simetro_dark",
    rules_summary: [
      "Three movers continuously circulate around a triangular route.",
      "The built-in speed tuner emits periodic decisions for the inspector.",
      "Reloading the scene re-reads the local JSON file without contacting a provider.",
    ],
    visual_notes: [
      "Dark canvas with high-contrast blue, purple, and green route accents.",
      "Circle, square, and triangle nodes make piece identity legible in thumbnails.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "metro-pulse",
    title: "Metro Pulse",
    subtitle: "A heartbeat-shaped metro map with polished station language and vivid route bands.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "metro_pulse_night",
    rules_summary: [
      "Four movers circulate around a single directed transit loop.",
      "The speed tuner stays local and only adjusts pacing for simple visual motion.",
      "The scene is static v1 JSON loaded from games/metro-pulse.json with no provider dependency.",
    ],
    visual_notes: [
      "Stations form a wide ECG pulse silhouette across the 960x540 screenshot target.",
      "Magenta, cyan, amber, mint, violet, and orange accents separate route segments on a navy canvas.",
      "Circles, squares, diamonds, a triangle, and a hexagon distinguish terminals, neighborhoods, transfers, peak, and hub.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "cargo-loom",
    title: "Cargo Loom",
    subtitle:
      "An industrial blueprint where cargo threads through gantries, sorters, cranes, and docks.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "blueprint_cargo",
    rules_summary: [
      "Five movers begin on separate logistics strands and circulate through the loom.",
      "The scene stays schema v1 so the v2-capable loader auto-upgrades it without live providers.",
      "Local scene switching resolves the registered scene id to games/cargo-loom.json.",
    ],
    visual_notes: [
      "Blueprint midnight background with amber freight, cyan conveyor, magenta sorter, blue steel, and green return accents.",
      "The left intake, central hex loom, lower return, and right dock form a wide industrial loom silhouette.",
      "Squares, circles, diamonds, triangles, and hexagons separate freight handling roles in thumbnails.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "factory-line-seeds",
    title: "Factory Line Seeds",
    subtitle:
      "A shape-factory conveyor fantasy where seed packets are cut, painted, stacked, inspected, and recycled.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "factory_line_seed_belts",
    rules_summary: [
      "Seven resource movers begin on extractor, seed, cutter, painter, stacker, warehouse, and recycle belts.",
      "The world stays schema v1 JSON while remaining compatible with the v2-capable loader.",
      "Local scene switching resolves the registered scene id to games/factory-line-seeds.json without live providers or arbitrary paths.",
    ],
    visual_notes: [
      "Graphite factory floor with conveyor blue, raw-resource amber, cutter magenta, painter cyan, seed green, warning orange, and stacker violet accents.",
      "Nodes form a stepped factory-line silhouette with left resource patches, an upper machine row, a right warehouse, and a low scrap-return belt.",
      "Triangles, circles, diamonds, squares, and hexagons distinguish extractors/chutes, resources, mergers/sorters, workstations, and stacker hubs.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "garden-pollinators",
    title: "Garden Pollinators",
    subtitle:
      "A pastel solarpunk garden where pollinators braid nectar, pollen, rain, and seeds around a hive hub.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "pastel_pollinator_garden",
    rules_summary: [
      "Five pollinator movers start on different garden wings and circulate through the main bloom loop.",
      "The scene remains schema v1 JSON while staying compatible with the v2-capable local loader.",
      "Scene switching resolves the registered scene id to games/garden-pollinators.json without arbitrary paths or live providers.",
    ],
    visual_notes: [
      "Sunwashed cream and ink support blossom pink, pollen gold, leaf mint, sky blue, lavender, terracotta, and teal water accents.",
      "Nodes form a butterfly-garden silhouette with a left nectar wing, central hive, right orchard wing, and low rain-meadow return.",
      "Circles, squares, diamonds, triangles, and hexagons distinguish blooms, beds, crossings, trellises, and hive/water infrastructure.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "data-packet-city",
    title: "Data Packet City",
    subtitle:
      "A dark terminal skyline where packets traverse gateways, firewalls, routers, and uplinks.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "terminal_packet_city",
    rules_summary: [
      "Six packet movers begin on separate network districts and circulate through the city loop.",
      "The scene remains schema v1 JSON while staying compatible with the v2-capable loader.",
      "Local scene switching resolves the registered scene id to games/data-packet-city.json.",
    ],
    visual_notes: [
      "Black terminal canvas with green packets, cyan fiber, magenta firewall scans, amber uplinks, violet edges, and red alerts.",
      "Nodes create a skyline silhouette with ingress streets, cache blocks, a central router plaza, firewall spire, and egress tower.",
      "Squares, circles, diamonds, triangles, and hexagons distinguish blocks, gateways, switches/firewalls, towers, and hubs.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "emergency-dispatch",
    title: "Emergency Dispatch",
    subtitle:
      "A high-contrast civic map where dispatchers coordinate sirens, triage, river rescue, and public works.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "civic_dispatch_night",
    rules_summary: [
      "Six emergency movers begin on separate ambulance, bridge, river, fire, evacuation, and triage routes.",
      "The scene remains schema v1 JSON while staying compatible with the v2-capable loader.",
      "Local scene switching resolves the registered scene id to games/emergency-dispatch.json without live providers or arbitrary paths.",
    ],
    visual_notes: [
      "Midnight civic canvas with white labels, dispatch cyan, emergency red, amber alerts, triage green, command violet, fire orange, and river blue accents.",
      "Nodes form a shield-and-siren silhouette around central dispatch, hospital, firehouse, bridge checkpoint, flood command, and river rescue.",
      "Hexagons, squares, triangles, diamonds, and circles distinguish command desks, stations, alarms, intersections, and care destinations.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "power-grid-balancer",
    title: "Power Grid Balancer",
    subtitle:
      "An electric-grid fantasy where operators balance generators, batteries, substations, and overloaded city loads.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "electric_grid_overload",
    rules_summary: [
      "Seven voltage-flow movers begin on separate hydro, wind, solar, transformer, overload, battery, and downtown feeder routes.",
      "The world stays schema v1 JSON while remaining compatible with the v2-capable loader.",
      "Local scene switching resolves the registered scene id to games/power-grid-balancer.json without live providers or arbitrary paths.",
    ],
    visual_notes: [
      "Near-black control-room canvas with readout cream, electric yellow busbars, cyan arcs, magenta overloads, orange heat, green relief, blue generation, and violet load accents.",
      "Nodes form a jagged lightning-bolt grid from left-side generation through a central breaker spine into right-side overloaded city loads and low storage return.",
      "Triangles, hexagons, diamonds, squares, and circles distinguish generation towers, balancers, transformers, substations/storage, and load sinks.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "river-ferries",
    title: "River Ferries",
    subtitle:
      "A watercolor river fantasy where ferries weave through quays, islands, marshes, and harbor lights.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "watercolor_river_ferries",
    rules_summary: [
      "Six ferry movers begin on distinct river crossings and circulate through the main S-curve loop.",
      "The world stays schema v1 JSON while remaining compatible with the v2-capable loader.",
      "Local scene switching resolves the registered scene id to games/river-ferries.json without live providers or arbitrary paths.",
    ],
    visual_notes: [
      "Warm watercolor paper with ink, river blue, teal current, lantern gold, rose quay, willow green, dusk violet, and terracotta hull accents.",
      "Nodes form a wide S-curved river silhouette from fog bank to willow island, market bend, lighthouse reach, and harbor-marsh return.",
      "Squares, circles, diamonds, triangles, and hexagons distinguish landings, islands/marshes, markets/yards, beacons, and river infrastructure.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "night-market-runners",
    title: "Night Market Runners",
    subtitle:
      "A lantern-lit courier fantasy where runners braid warm market alleys, rooftop tea routes, and moon-post deliveries.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "lantern_night_market",
    rules_summary: [
      "Six courier movers begin on distinct gate, spice, noodle, rooftop, east alley, and moon-post runs.",
      "The world stays schema v1 JSON while remaining compatible with the v2-capable loader.",
      "Local scene switching resolves the registered scene id to games/night-market-runners.json without live providers or arbitrary paths.",
    ],
    visual_notes: [
      "Deep plum night canvas with cream text, lantern gold, chili coral, noodle amber, jade teal, silk violet, blossom pink, and courier mint accents.",
      "Nodes form a hanging-lantern night-market silhouette with an arcing stall canopy and lower courier alley return.",
      "Squares, diamonds, triangles, circles, and hexagons distinguish gates/posts, stalls, rooftop lanterns, courts, and clock/drum landmarks.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "orbital-transfers",
    title: "Orbital Transfers",
    subtitle:
      "A starfield transfer-control fantasy where shuttles hop between orbital yards, station hubs, lunar windows, and slingshot beacons.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "starfield_orbital_transfers",
    rules_summary: [
      "Seven shuttle movers begin on separate station, burn, lunar, slingshot, cargo, solar, and hazard-watch transfer segments.",
      "The world stays schema v1 JSON while remaining compatible with the v2-capable loader.",
      "Local scene switching resolves the registered scene id to games/orbital-transfers.json without live providers or arbitrary paths.",
    ],
    visual_notes: [
      "Near-black starfield canvas with white foreground, cyan orbital lanes, orange burns, magenta windows, violet stations, solar gold, support green, lunar blue, and red hazards.",
      "Nodes form a tilted orbital-transfer silhouette with an inner station diamond, outer ellipse, lunar gate, right slingshot arc, and low cargo-return orbit.",
      "Hexagons, circles, diamonds, triangles, and squares distinguish command hubs, docks, windows, burn beacons, solar arrays, and cargo yards.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "gpu-launch-week",
    title: "GPU Launch Week",
    subtitle:
      "An HPC cluster keeps GPU jobs and health dashboards stable through telemetry, fact-building, and report-refresh pressure. SL1 scene v0 — visuals land in a later PR.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "gpu_launch_week_midnight",
    rules_summary: [
      "The scene is the first scenario_language_v1 world: places, links, things, transforms, and demand replace the legacy mover loop.",
      "All transforms colocate on the gpu-platform place: heartbeats are normalized, normalized signals become uptime facts, and facts refresh the dashboard report.",
      "A critical executive dashboard demand fires every 60 ticks starting at tick 120 and observes the dashboard_result thing at the gpu-platform target.",
      "Pressure events, visible objectives, observability, agents, and a winnable/losable HUD are explicitly scheduled for later PRs in the roadmap.",
    ],
    visual_notes: [
      "Marked status=draft because the SL1 canvas renderer is not yet wired; selecting the scene loads it and shows catalog metadata in the scene browser.",
      "Four SL1 places form a wide pipeline silhouette: source telemetry upper-left, central gpu-platform cluster, kusto dashboard upper-right, incident-room operator overlay below.",
      "Place roles (source / compute_cluster / dashboard / operator) act as the SL1 node language and will drive future render hints.",
    ],
    status: "draft",
  }),
defineScene({
    id: "clinic-triage-desk",
    title: "Clinic Triage Desk",
    subtitle: "A busy walk-in clinic where triage nurses, assessment rooms, and discharge coordinators must process patient surges without losing anyone in the queue.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "clinical_triage",
    rules_summary: [
      "Six SL1 places span intake-to-discharge: intake bay, triage desk, assessment room, treatment bay, discharge hub, nurse station.",
      "SL1 links carry patient flow; no legacy movers.",
      "Demand fires when the discharge hub is overloaded; agents balance throughput against staff capacity limits.",
    ],
    visual_notes: [
      "Warm clinical palette: dark background with blue intake, red alert, green discharge, and amber vitals accents.",
      "Place roles: source=intake-bay, processor=triage/treatment, buffer=assessment, dashboard=discharge, operator=nurse.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "greenhouse-water-watch",
    title: "Greenhouse Water Watch",
    subtitle: "An automated greenhouse where weather sensors, water pumps, and soil monitors must keep every grow bed within safe moisture limits through variable cloud cover.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "greenhouse_green",
    rules_summary: [
      "Six SL1 places span sensor-to-nursery: outdoor weather sensor, pump room, soil monitor, irrigation grid, growth nursery, and operator cabin.",
      "SL1 links carry control and data flow; no legacy movers.",
      "Demand fires on the irrigation grid when soil moisture drops; agents balance pump cycles against forecast rainfall.",
    ],
    visual_notes: [
      "Natural green and earth palette: amber pipelines, teal soil monitors, coral drought alerts on a dark background.",
      "Place roles: source=weather-sensor, processor=water-pump/irrigation-grid, buffer=soil-monitor, dashboard=growth-nursery.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "library-reshelving-clock",
    title: "Library Reshelving Clock",
    subtitle: "A public library where a returning tide of books must be sorted, catalogued, shelved, and surfaced as availability reports before patron queues grow.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "library_warm",
    rules_summary: [
      "Six SL1 places span returns-to-reading: return desk, sort station, catalog database, shelf zone, reading room, head librarian station.",
      "SL1 links carry item flows; no legacy movers.",
      "Demand fires when the reading room availability report is stale; agents must keep the catalog pipeline moving.",
    ],
    visual_notes: [
      "Warm library palette: dark background, sand foreground, blue intake, amber sort, green catalog, teal shelf accents.",
      "Place roles: source=return-desk, processor=sort-station/shelf-zone, buffer=catalog-db, dashboard=reading-room.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "microgrid-starter",
    title: "Microgrid Starter",
    subtitle: "A rooftop solar microgrid where battery banks, inverters, and local distribution must serve peak loads through cloud cover and demand spikes.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "electric_microgrid",
    rules_summary: [
      "Six SL1 places span generation-to-metering: solar array, battery bank, inverter hub, local grid, metering dashboard, and grid operator.",
      "SL1 links carry power and telemetry; no legacy movers.",
      "Demand fires when local grid draw exceeds battery reserve; agents balance discharge rate against solar forecast.",
    ],
    visual_notes: [
      "Electric energy palette: dark background, amber solar, cyan battery, green inverter, blue distribution, violet operator accents.",
      "Place roles: source=solar-array, buffer=battery-bank, processor=inverter-hub/local-grid, dashboard=metering.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "sensor-calibration-lab",
    title: "Sensor Calibration Lab",
    subtitle: "A metrological calibration lab where raw sensor outputs must be corrected against certified reference standards before shipping to field operators.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "metrology_precision",
    rules_summary: [
      "Six SL1 places span bench-to-log: test bench, calibration rack, reference standards vault, validation suite, certification log, and lab supervisor.",
      "SL1 links carry sample and result flows; no legacy movers.",
      "Demand fires when the certification log accumulates un-validated readings; agents must keep the calibration pipeline unblocked.",
    ],
    visual_notes: [
      "Precision metrology palette: dark background, steel foreground, blue raw inputs, amber offsets, green validated, violet certified accents.",
      "Place roles: source=test-bench, processor=calibration-rack/validation-suite, buffer=reference-standards, dashboard=certification-log.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "stormwater-pump-room",
    title: "Stormwater Pump Room",
    subtitle: "An urban stormwater facility where debris filters, lift stations, and retention basins must keep streets clear during flash-flood events.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "stormwater_flood",
    rules_summary: [
      "Six SL1 places span collection-to-outfall: rain collector, debris filter, pump station, retention basin, outfall monitor, and flood coordinator.",
      "SL1 links carry fluid flows; no legacy movers.",
      "Pressure events simulate surge rainfall; agents balance filter cleaning cycles against rising basin levels.",
    ],
    visual_notes: [
      "Water and flood palette: dark background, deep blue stormwater, teal filtered, cyan pumped, green relief, orange surge warning accents.",
      "Place roles: source=rain-collector, processor=debris-filter/pump-station, buffer=retention-basin, dashboard=outfall-monitor.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "bakery-oven-shift",
    title: "Bakery Oven Shift",
    subtitle: "A commercial bakery where ingredient stores, prep stations, proofing racks, and ovens must keep the sales counter stocked across a twelve-hour shift.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "bakery_warm",
    rules_summary: [
      "Six SL1 places span store-to-counter: ingredient store, prep station, proofing rack, main oven, display counter, and shift supervisor.",
      "SL1 links carry food flows; no legacy movers.",
      "Demand fires when the display counter falls below minimum stock; agents balance batch size against oven capacity limits.",
    ],
    visual_notes: [
      "Warm bakery palette: dark background, cream foreground, amber flour/dough, orange bake, golden product, violet supervisor accents.",
      "Place roles: source=ingredient-store, processor=prep-station/main-oven, buffer=proofing-rack, dashboard=display-counter.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "warehouse-cold-chain",
    title: "Warehouse Cold Chain",
    subtitle: "A refrigerated distribution warehouse where temperature sensors, chill zones, and pick operations must keep cold-chain integrity during high-volume dispatch windows.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "cold_chain_logistics",
    rules_summary: [
      "Six SL1 places span receiving-to-dispatch: cold dock, chill zone, temperature monitor, pick floor, dispatch hub, and cold-chain coordinator.",
      "SL1 links carry product and telemetry flows; no legacy movers.",
      "Pressure events simulate temperature excursions; agents must reroute product before exceedance thresholds are breached.",
    ],
    visual_notes: [
      "Cold-chain palette: dark background with blue chill zones, cyan temperature accents, green compliant, and amber warning states.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "observatory-night-queue",
    title: "Observatory Night Queue",
    subtitle: "A hilltop observatory where telescopes, mount controllers, CCD cameras, and image pipelines must complete a full night queue before astronomical twilight.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "observatory_night",
    rules_summary: [
      "Six SL1 places span queue-to-archive: scheduler, mount controller, CCD camera, image pipeline, science archive, and night observer.",
      "SL1 links carry control and data flows; no legacy movers.",
      "Demand fires when the image pipeline backs up; agents prioritize targets by scientific value and remaining dark time.",
    ],
    visual_notes: [
      "Night sky palette: dark background with violet telescope accents, cyan mount, amber CCD, green archive, and white star foreground.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "recycling-sort-floor",
    title: "Recycling Sort Floor",
    subtitle: "A materials recovery facility where conveyor belts, sort robots, and balers must convert mixed waste into clean commodity streams before market close.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "recycling_green",
    rules_summary: [
      "Six SL1 places span intake-to-bale: intake conveyor, sort robot station, contaminant quarantine, baling press, commodity store, and floor supervisor.",
      "SL1 links carry material flows; no legacy movers.",
      "Pressure events simulate contamination spikes; agents must reroute streams before the commodity store fills with off-spec bales.",
    ],
    visual_notes: [
      "Recovery palette: dark background with green clean stream, amber sorted, red contaminant, cyan baled, and earth commodity accents.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "datacenter-cooling-surge",
    title: "Datacenter Cooling Surge",
    subtitle: "A hyperscale datacenter cooling plant where chillers, cooling towers, and distribution loops must suppress rack temperatures during unexpected compute surges.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "datacenter_midnight",
    rules_summary: [
      "Six SL1 places span intake-to-rack: chiller plant, cooling tower, distribution loop, hot-aisle sensor, rack-temperature dashboard, and facility engineer.",
      "SL1 links carry coolant and telemetry flows; no legacy movers.",
      "Pressure events simulate GPU workload spikes; agents must adjust chiller setpoints before thermal shutdown thresholds are breached.",
    ],
    visual_notes: [
      "Datacenter midnight palette: dark background with cyan coolant, blue chiller, amber hot aisle, red thermal alarm, and green safe-zone accents.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "hospital-bed-command",
    title: "Hospital Bed Command",
    subtitle: "A hospital command center that coordinates admissions, bed assignments, transfers, and discharges to prevent overcrowding during a multi-ward surge.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "hospital_command",
    rules_summary: [
      "Six SL1 places span intake-to-discharge: admissions office, bed assignment desk, ward cluster, transfer coordinator, discharge planner, and hospital director.",
      "SL1 links carry patient-flow signals; no legacy movers.",
      "Demand fires when ward occupancy exceeds safe thresholds; agents must pipeline discharges and transfers to create bed capacity.",
    ],
    visual_notes: [
      "Hospital command palette: dark background with blue admission flow, amber waiting, red overcapacity, green discharge, and violet executive overlay accents.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "food-bank-allocation",
    title: "Food Bank Allocation",
    subtitle: "A regional food bank where donation trucks, sort volunteers, packing lines, and delivery schedulers must maximize household reach during a surge donation week.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "food_bank_amber",
    rules_summary: [
      "Six SL1 places span donation-to-delivery: donation dock, sort floor, cold storage, packing line, allocation dashboard, and logistics coordinator.",
      "SL1 links carry food product flows; no legacy movers.",
      "Demand fires when allocation requests spike; agents balance perishable turnover against cold-storage capacity limits.",
    ],
    visual_notes: [
      "Food bank palette: dark background with amber donation stream, teal sorted, green packed, blue cold-chain, and orange urgency accents.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "security-alert-fusion",
    title: "Security Alert Fusion",
    subtitle: "A security operations center where SIEM alerts, threat intelligence enrichment, and correlation engines must clear analyst queues before escalation windows close.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "security_ops_dark",
    rules_summary: [
      "Six SL1 places span ingest-to-close: SIEM ingest, enrichment engine, correlation platform, analyst queue, incident dashboard, and SOC manager.",
      "SL1 links carry alert and evidence flows; no legacy movers.",
      "Pressure events simulate alert storms; agents must tune correlation rules before the analyst queue overflows into unreviewed escalations.",
    ],
    visual_notes: [
      "SOC dark palette: dark background with red alert ingest, amber enrichment, cyan correlated, green resolved, and violet management overlay accents.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "satellite-downlink-window",
    title: "Satellite Downlink Window",
    subtitle: "A ground station that must capture satellite passes, decode telemetry, and archive science data during narrow downlink windows before orbital geometry closes.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "satellite_space",
    rules_summary: [
      "Six SL1 places span antenna-to-archive: dish antenna, demodulator, telemetry decoder, science archive, pass-schedule dashboard, and ground controller.",
      "SL1 links carry RF and data flows; no legacy movers.",
      "Demand fires at pass start; agents must pre-configure the pipeline fast enough to capture the full data volume before the window closes.",
    ],
    visual_notes: [
      "Satellite palette: dark background with cyan antenna beam, violet demodulation, amber telemetry, green archived, and white orbital-window countdown accents.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "chip-fab-yield-crisis",
    title: "Chip Fab Yield Crisis",
    subtitle: "A semiconductor fab in yield crisis where lithography, etch, and inspection modules must isolate defect root causes before the next production tape-out deadline.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "fab_yield_crisis",
    rules_summary: [
      "Six SL1 places span wafer-to-output: wafer input station, lithography module, etch chamber, inspection bay, yield analytics dashboard, and fab process engineer.",
      "SL1 links carry wafer and metrology flows; no legacy movers.",
      "Pressure events simulate defect surges across process nodes; agents must route wafers to isolate the defect source before yield floor breaches the tape-out gate.",
    ],
    visual_notes: [
      "Fab yield palette: dark background with violet lithography, cyan etch, amber inspection, red defect alarm, and green yield-recovered accents.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "regional-blackstart",
    title: "Regional Blackstart",
    subtitle: "A grid operator rebuilding a region after a cascading blackout, cranking blackstart units, picking up feeder blocks, and restoring interconnects under strict frequency limits.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "power_grid_dark",
    rules_summary: [
      "Six SL1 places span cranking-to-restoration: blackstart unit, cranking bus, feeder selector, zone energization controller, frequency dashboard, and system operator.",
      "SL1 links carry power and control flows; no legacy movers.",
      "Pressure events simulate frequency deviations; agents must sequence energization to keep frequency within recovery corridor while expanding the restored area.",
    ],
    visual_notes: [
      "Power restoration palette: dark background with yellow cranking bus, cyan energized feeder, green restored zone, red frequency excursion, and amber stabilizing accents.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "airport-ground-stop",
    title: "Airport Ground Stop",
    subtitle: "An airport traffic control operation during a ground stop, managing gate conflicts, pushback sequencing, taxiway deconfliction, and departure prioritization under flow constraints.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "airport_atc_dark",
    rules_summary: [
      "Six SL1 places span gate-to-runway: gate assignment desk, pushback coordinator, taxiway controller, runway sequencer, departure dashboard, and traffic flow manager.",
      "SL1 links carry aircraft movement signals; no legacy movers.",
      "Pressure events simulate gate-conflict cascades; agents must sequence pushback and taxiway clearances before slot penalties accumulate.",
    ],
    visual_notes: [
      "ATC dark palette: dark background with blue taxi flows, amber gate hold, green cleared-for-pushback, red conflict alarm, and white runway accent.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "pandemic-supply-web",
    title: "Pandemic Supply Web",
    subtitle: "A pharmaceutical emergency supply network coordinating API sourcing, bulk formulation, fill-and-finish, cold-chain packaging, and last-mile delivery during a surge.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "pharma_supply_dark",
    rules_summary: [
      "Six SL1 places span source-to-delivery: API sourcing hub, bulk formulation plant, fill-and-finish line, cold-chain packager, distribution dashboard, and supply chain director.",
      "SL1 links carry material and logistics flows; no legacy movers.",
      "Pressure events simulate input shortages and demand spikes; agents must rebalance supplier allocations before the cold-chain buffer depletes.",
    ],
    visual_notes: [
      "Pharma supply palette: dark background with blue API stream, cyan formulation, amber fill-finish, green packaged, and red shortage alarm accents.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "fusion-shot-campaign",
    title: "Fusion Shot Campaign",
    subtitle: "An inertial confinement fusion facility executing a multi-shot ignition campaign where laser charge, target alignment, and diagnostic windows must converge for every shot.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "fusion_energy_dark",
    rules_summary: [
      "Six SL1 places span charge-to-record: laser charge system, target injection bay, alignment optics, ignition chamber, diagnostics recorder, and shot director.",
      "SL1 links carry energy and control flows; no legacy movers.",
      "Pressure events simulate laser-energy variance and alignment drift; agents must converge all systems within shot readiness window before campaign clock expires.",
    ],
    visual_notes: [
      "Fusion energy palette: dark background with violet laser charge, cyan alignment beam, orange ignition flash, amber diagnostics, and green shot-success accents.",
      "Place roles: source, processor×2, buffer, dashboard, operator — shaped by role.",
      "Marked draft: SL1 canvas renderer not yet wired; loading succeeds and schema validates.",
    ],
    status: "draft",
  }),

  defineScene({
    id: "circuit-garden",
    title: "Circuit Garden",
    subtitle: "A printed-circuit-board garden where logic-gate nodes route signals through copper-trace paths.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "circuit_garden_neon",
    rules_summary: [
      "Twelve nodes form a ring (n0–n9) with two central hubs; fourteen paths create a complete directed loop with hub spokes.",
      "Six movers circulate at varying speeds; each starts on a different ring segment.",
      "No scenario_language_v1 block; the built-in speed tuner adjusts pacing.",
    ],
    visual_notes: [
      "All five node shapes — circle, square, hexagon, diamond, triangle — appear across the ring.",
      "Neon circuit palette: dark background with copper, teal logic, amber signal, magenta gate, and green output accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "kitchen-prep-board",
    title: "Kitchen Prep Board",
    subtitle: "A restaurant prep board where ingredient stations feed chopping blocks, grills, and plating zones.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "kitchen_prep_warm",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the full prep circuit.",
      "Six movers represent ingredient carts circulating through the prep stations.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Warm kitchen palette: dark background with amber stove, green produce, red meat, blue plating, and cream foreground accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "archive-index-table",
    title: "Archive Index Table",
    subtitle: "A document archive where index trolleys shuttle between filing cabinets and reading desks.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "archive_sepia",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths complete the index circuit.",
      "Six movers represent document trolleys circulating at measured archival speeds.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Sepia archive palette: dark background with sand foreground, brown cabinet, amber index, teal reading desk, and sage accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "reef-nursery",
    title: "Reef Nursery",
    subtitle: "An underwater coral-nursery network linking frag stations, growth tanks, and reef-placement sites.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "reef_ocean_blue",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the coral circuit.",
      "Six movers represent coral fragments carried by currents and nursery staff.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Ocean blue palette: dark background with cyan current, coral pink, teal growth, amber reef, and green healthy accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "robot-arm-workbench",
    title: "Robot Arm Workbench",
    subtitle: "An assembly workbench where robot arms, part feeders, and inspection stations form a cellular manufacturing loop.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "robot_industrial",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the assembly circuit.",
      "Six movers represent part carriers and assembly subassemblies moving through the cell.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Industrial palette: dark background with steel blue robot, amber conveyor, green inspected, magenta rejected, and cyan assembled accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "forge-heat-map",
    title: "Forge Heat Map",
    subtitle: "A metal forge where billets travel through heating zones, hammering presses, and quench tanks on a production heat map.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "forge_heat_glow",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the forge circuit.",
      "Six movers represent billets at different temperature stages moving through the forge.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Forge heat palette: dark background with red hot zone, amber press, cyan quench, orange forming, and yellow temper accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "seed-bank-vault",
    title: "Seed Bank Vault",
    subtitle: "A global seed bank where collection trays, drying racks, cold-store vaults, and catalog readers form a preservation circuit.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "seed_vault_arctic",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the seed-preservation circuit.",
      "Six movers represent seed trays moving through intake, drying, vaulting, and retrieval.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Arctic vault palette: dark background with white frost, blue cold-store, teal drying, amber catalog, and green seed accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "drone-repair-bay",
    title: "Drone Repair Bay",
    subtitle: "A drone maintenance bay where inspection rigs, soldering stations, firmware flashers, and test pads keep the fleet airworthy.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "drone_repair_dark",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the repair circuit.",
      "Six movers represent drones in various stages of disassembly, repair, and recertification.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Repair bay palette: dark background with cyan diagnostic, amber solder, violet firmware, green certified, and red grounded accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "weather-balloon-yard",
    title: "Weather Balloon Yard",
    subtitle: "A meteorological launch yard where weather balloons are filled, calibrated, launched, and tracked until signal loss.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "weather_sky_blue",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the balloon circuit.",
      "Six movers represent balloons at different ascent phases moving through fill, launch, ascent, and recovery stages.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Sky palette: dark background with blue fill-station, cyan ascent path, amber sensor calibration, green data-good, and white cloud accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "crystal-growth-rig",
    title: "Crystal Growth Rig",
    subtitle: "A crystal growth laboratory where seed crystals travel through solution chambers, growth columns, and annealing ovens.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "crystal_lab_violet",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the crystal-growth circuit.",
      "Six movers represent crystal samples at different growth stages.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Crystal lab palette: dark background with violet growth column, cyan solution, amber anneal, teal seed, and white facet accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "bioreactor-balance",
    title: "Bioreactor Balance",
    subtitle: "A bioprocess plant where inoculation stations, bioreactors, centrifuges, and formulation suites must hit expression targets.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "bioreactor_bio",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the bioprocess circuit.",
      "Six movers represent culture batches at different stages of fermentation and downstream processing.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Bioprocess palette: dark background with green culture, cyan centrifuge, amber formulation, violet purification, and teal harvest accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "disaster-supply-staging",
    title: "Disaster Supply Staging",
    subtitle: "A disaster-response staging yard where relief supplies, medical kits, water tankers, and rescue teams are pre-positioned for rapid dispatch.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "disaster_staging_orange",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the staging circuit.",
      "Six movers represent supply convoys and response teams circulating through intake, staging, pre-position, and dispatch.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Emergency staging palette: dark background with orange dispatch, red urgent, blue water, green medical, and amber staging accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "fabric-dye-lab",
    title: "Fabric Dye Lab",
    subtitle: "A textile dye laboratory where fabric bolts pass through mordanting, dyeing vats, fixing chambers, and drying racks.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "dye_lab_vibrant",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the dye-process circuit.",
      "Six movers represent fabric bolts at different stages of the dyeing process.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Vibrant dye palette: dark background with magenta mordant, cyan dye vat, amber fixing, violet finished, and green dried accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "museum-conservation-bench",
    title: "Museum Conservation Bench",
    subtitle: "A conservation workshop where artifacts move through cleaning, consolidation, documentation, and climate-controlled storage.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "museum_conservation_warm",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the conservation circuit.",
      "Six movers represent artifacts at different stages of treatment and documentation.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Conservation palette: dark background with sand foreground, amber artifact, teal cleaning, blue documentation, and violet climate-store accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "wildfire-watch-grid",
    title: "Wildfire Watch Grid",
    subtitle: "A wildfire monitoring network where remote sensors, aerial scouts, and incident command posts coordinate containment across a fire grid.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "wildfire_watch_red",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the watch-grid circuit.",
      "Six movers represent sensor reports, scout flights, and crew dispatches moving through the incident network.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Wildfire palette: dark background with red fire front, orange ember, amber sensor, green containment, and cyan aerial-scout accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "quantum-control-room",
    title: "Quantum Control Room",
    subtitle: "A cryogenic qubit control room where dilution fridges, microwave lines, FPGA controllers, and error-correction modules must maintain qubit coherence.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "quantum_cryo_violet",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the qubit control circuit.",
      "Six movers represent control pulses and calibration sweeps at different stages of the quantum stack.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Quantum cryo palette: dark background with violet qubit lines, cyan cryo, amber calibration, green coherent, and white decoherence accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "deep-sea-habitat-grid",
    title: "Deep Sea Habitat Grid",
    subtitle: "A deep-sea habitat network where pressurized modules, life-support loops, ROV docking bays, and surface comms must sustain a saturation diving operation.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "deep_sea_dark_blue",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the habitat circuit.",
      "Six movers represent supply drops, crew transfers, and ROV missions circulating through the habitat grid.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Deep-sea palette: dark background with deep blue pressure hull, teal life-support, cyan ROV, amber surface comms, and red emergency accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "city-budget-war-room",
    title: "City Budget War Room",
    subtitle: "A city budget war room where department requests, revenue forecasts, council votes, and emergency reserves must balance before the fiscal deadline.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "city_budget_civic",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the budget circuit.",
      "Six movers represent budget line items, supplemental requests, and reserve transfers moving through the review process.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Civic budget palette: dark background with blue revenue, amber spending, green surplus, red deficit, and violet council-vote accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "planetary-defense-array",
    title: "Planetary Defense Array",
    subtitle: "A planetary defense coordination network where radar tracks, threat models, interceptor assignments, and detonation windows must converge before impact.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "planetary_defense_space",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the defense circuit.",
      "Six movers represent tracking data, threat assessments, and interceptor telemetry moving through the command network.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Defense space palette: dark background with cyan radar track, red threat, amber interceptor, violet detonation window, and green deflected accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),

  defineScene({
    id: "autonomous-farm-season",
    title: "Autonomous Farm Season",
    subtitle: "An autonomous farm management system where planting robots, irrigation networks, pest-control drones, and harvest combines must complete a full growing season.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "farm_season_green",
    rules_summary: [
      "Twelve nodes form a ring with two central hubs; fourteen directed paths create the farm season circuit.",
      "Six movers represent farm operations — planting, irrigation, pest control, monitoring, harvesting, and post-harvest processing — circulating through the season.",
      "No scenario_language_v1 block; the built-in speed tuner handles pacing.",
    ],
    visual_notes: [
      "All five node shapes appear across the ring.",
      "Farm season palette: dark background with green planting, blue irrigation, amber harvest, red pest-alert, and teal monitoring accents.",
      "Marked draft pending visual polish pass.",
    ],
    status: "ready",
  }),
] as const satisfies readonly SceneCatalogEntry[];

export type SceneCatalogId = (typeof SCENE_CATALOG)[number]["id"];

export function findSceneById(id: string): SceneCatalogEntry | undefined {
  return SCENE_CATALOG.find((scene) => scene.id === id);
}

export function isLocalScenePath(path: string): boolean {
  return /^games\/[a-z0-9][a-z0-9_-]*\.json$/.test(path);
}
