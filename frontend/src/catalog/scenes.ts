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
  // ── Complex scenario pack (40 scenes) ──────────────────────────────────────
  defineScene({
    id: "airport-ground-stop",
    title: "Airport Ground Stop",
    subtitle: "An airport ground stop where an AI flow manager must route flight plans through gate assignment, pushback, taxiway sequencing, and runway departure queues while preventing gridlock.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "default_airport_ground_stop",
    rules_summary: [
      "An airport ground stop where an AI flow manager must route flight plans through gate assignment, pushback, taxiway, and runway sequences.",
    ],
    visual_notes: ["Airport Ground Stop topology."],
    status: "draft",
  }),
  defineScene({
    id: "archive-index-table",
    title: "Archive Index Table",
    subtitle: "A document archive where index trolleys travel a three-level tree of stacks and shelves, returning through a shared reading room.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "default_archive_index_table",
    rules_summary: ["Archive Index Table — 3-level tree transit loop."],
    visual_notes: ["Archive Index Table topology."],
    status: "ready",
  }),
  defineScene({
    id: "autonomous-farm-season",
    title: "Autonomous Farm Season",
    subtitle: "An autonomous farm laid out as a 5×3 seasonal grid (spring–harvest rows, west–east columns) with horizontal irrigation lanes, vertical growth channels, and diagonal crop-routing shortcuts.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "default_autonomous_farm_season",
    rules_summary: ["Autonomous Farm Season — 5x3 grid transit loop."],
    visual_notes: ["Autonomous Farm Season topology."],
    status: "ready",
  }),
  defineScene({
    id: "bakery-oven-shift",
    title: "Bakery Oven Shift",
    subtitle: "A commercial bakery where ingredient stores, prep stations, proofing racks, and ovens must keep the sales counter stocked through holiday rushes and oven faults.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "default_bakery_oven_shift",
    rules_summary: ["Bakery Oven Shift — SL1 scenario."],
    visual_notes: ["Bakery Oven Shift topology."],
    status: "draft",
  }),
  defineScene({
    id: "bioreactor-balance",
    title: "Bioreactor Balance",
    subtitle: "A bioprocess plant with dual top/bottom pipelines (inoculation→harvest and media→formulation) bridged by three cross-connectors.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "default_bioreactor_balance",
    rules_summary: ["Bioreactor Balance — dual pipeline transit loop."],
    visual_notes: ["Bioreactor Balance topology."],
    status: "ready",
  }),
  defineScene({
    id: "chip-fab-yield-crisis",
    title: "Chip Fab Yield Crisis",
    subtitle: "A semiconductor fab where wafer lots move through lithography, etching, and inspection to yield analytics during a process-defect crisis demanding rapid lot disposition.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "default_chip_fab_yield_crisis",
    rules_summary: ["Chip Fab Yield Crisis — hard SL1 scenario."],
    visual_notes: ["Chip Fab Yield Crisis topology."],
    status: "draft",
  }),
  defineScene({
    id: "circuit-garden",
    title: "Circuit Garden",
    subtitle: "A printed-circuit-board garden where logic-gate nodes route digital signals through copper-trace paths in a compact grid.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "default_circuit_garden",
    rules_summary: ["Circuit Garden — 3x3 grid transit loop."],
    visual_notes: ["Circuit Garden topology."],
    status: "ready",
  }),
  defineScene({
    id: "city-budget-war-room",
    title: "City Budget War Room",
    subtitle: "A municipal budget network with department request nodes, committee review nodes, and treasury allocation nodes in three cross-linked columns.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "default_city_budget_war_room",
    rules_summary: ["City Budget War Room — 3-group columns transit loop."],
    visual_notes: ["City Budget War Room topology."],
    status: "ready",
  }),
  defineScene({
    id: "clinic-triage-desk",
    title: "Clinic Triage Desk",
    subtitle: "A busy walk-in clinic where triage nurses, assessment rooms, treatment bays, and discharge coordinators must process patient surges without losing anyone in the queue.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "default_clinic_triage_desk",
    rules_summary: ["Clinic Triage Desk — intro SL1 scenario."],
    visual_notes: ["Clinic Triage Desk topology."],
    status: "draft",
  }),
  defineScene({
    id: "crystal-growth-rig",
    title: "Crystal Growth Rig",
    subtitle: "A materials laboratory with a two-row hexagonal lattice of growth columns and solution chambers for crystal synthesis.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "default_crystal_growth_rig",
    rules_summary: ["Crystal Growth Rig — hex lattice transit loop."],
    visual_notes: ["Crystal Growth Rig topology."],
    status: "ready",
  }),
  defineScene({
    id: "datacenter-cooling-surge",
    title: "Datacenter Cooling Surge",
    subtitle: "A datacenter where thermal sensors, chiller plants, and coolant distribution loops must prevent hot-aisle temperature runaway during a compute surge.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "default_datacenter_cooling_surge",
    rules_summary: ["Datacenter Cooling Surge — medium SL1 scenario."],
    visual_notes: ["Datacenter Cooling Surge topology."],
    status: "draft",
  }),
  defineScene({
    id: "deep-sea-habitat-grid",
    title: "Deep Sea Habitat Grid",
    subtitle: "An underwater habitat network with an outer pressure-module octagon, inner life-support pentagon, and central command hubs connected by an ROV dock.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "default_deep_sea_habitat_grid",
    rules_summary: ["Deep Sea Habitat Grid — octagon+pentagon transit loop."],
    visual_notes: ["Deep Sea Habitat Grid topology."],
    status: "ready",
  }),
  defineScene({
    id: "disaster-supply-staging",
    title: "Disaster Supply Staging",
    subtitle: "A disaster-response staging network with three supply hubs forming a triangle, each dispatching relief convoys to two delivery zones.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "default_disaster_supply_staging",
    rules_summary: ["Disaster Supply Staging — 3-hub triangle transit loop."],
    visual_notes: ["Disaster Supply Staging topology."],
    status: "ready",
  }),
  defineScene({
    id: "drone-repair-bay",
    title: "Drone Repair Bay",
    subtitle: "A UAV maintenance bay with a figure-8 layout: one repair loop (inspect/disassemble/reassemble) and one certification loop (test/flash/calibrate).",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "default_drone_repair_bay",
    rules_summary: ["Drone Repair Bay — figure-8 transit loop."],
    visual_notes: ["Drone Repair Bay topology."],
    status: "ready",
  }),
  defineScene({
    id: "fabric-dye-lab",
    title: "Fabric Dye Lab",
    subtitle: "A textile dye laboratory with three parallel dye chains (acid, reactive, vat) whose intake and fix ends are cross-linked for batch routing.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "default_fabric_dye_lab",
    rules_summary: ["Fabric Dye Lab — 3 parallel chains transit loop."],
    visual_notes: ["Fabric Dye Lab topology."],
    status: "ready",
  }),
  defineScene({
    id: "food-bank-allocation",
    title: "Food Bank Allocation",
    subtitle: "A food bank where donation batches are sorted, cold-stored, packed into delivery boxes, and allocated under surge donation pressure.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "default_food_bank_allocation",
    rules_summary: ["Food Bank Allocation — medium SL1 scenario."],
    visual_notes: ["Food Bank Allocation topology."],
    status: "draft",
  }),
  defineScene({
    id: "forge-heat-map",
    title: "Forge Heat Map",
    subtitle: "A metal forge where billets travel an outer heat-map ring with hot shortcuts to inner quench stations.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "default_forge_heat_map",
    rules_summary: ["Forge Heat Map — ring+inner stations transit loop."],
    visual_notes: ["Forge Heat Map topology."],
    status: "ready",
  }),
  defineScene({
    id: "fusion-shot-campaign",
    title: "Fusion Shot Campaign",
    subtitle: "An inertial-confinement fusion campaign where the AI shot director sequences laser charges, target injection, optical alignment, and ignition to maximise yield measurements.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "default_fusion_shot_campaign",
    rules_summary: ["Fusion Shot Campaign — hard SL1 scenario."],
    visual_notes: ["Fusion Shot Campaign topology."],
    status: "draft",
  }),
  defineScene({
    id: "greenhouse-water-watch",
    title: "Greenhouse Water Watch",
    subtitle: "An automated greenhouse where weather sensors, water pumps, soil monitors, and irrigation grids must keep every grow bed watered through drought spells and pump failures.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "default_greenhouse_water_watch",
    rules_summary: ["Greenhouse Water Watch — intro SL1 scenario."],
    visual_notes: ["Greenhouse Water Watch topology."],
    status: "draft",
  }),
  defineScene({
    id: "hospital-bed-command",
    title: "Hospital Bed Command",
    subtitle: "A hospital bed command centre where admission requests flow through ward assignment, transfer orders, and discharge planning to maintain live capacity dashboards under surge.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "default_hospital_bed_command",
    rules_summary: ["Hospital Bed Command — medium SL1 scenario."],
    visual_notes: ["Hospital Bed Command topology."],
    status: "draft",
  }),
  defineScene({
    id: "kitchen-prep-board",
    title: "Kitchen Prep Board",
    subtitle: "A restaurant prep board where ingredient stations, chopping blocks, grills, and plating zones form a branching kitchen flow.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "default_kitchen_prep_board",
    rules_summary: ["Kitchen Prep Board — spine+branches transit loop."],
    visual_notes: ["Kitchen Prep Board topology."],
    status: "ready",
  }),
  defineScene({
    id: "library-reshelving-clock",
    title: "Library Reshelving Clock",
    subtitle: "A public library where a returning tide of books must be sorted, catalogued, shelved, and surfaced as availability reports before the reading-room backlog grows unmanageable.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "default_library_reshelving_clock",
    rules_summary: ["Library Reshelving Clock — intro SL1 scenario."],
    visual_notes: ["Library Reshelving Clock topology."],
    status: "draft",
  }),
  defineScene({
    id: "microgrid-starter",
    title: "Microgrid Starter",
    subtitle: "A rooftop solar microgrid where battery banks, inverters, and local distribution must serve peak loads through cloud cover and inverter thermal limits.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "default_microgrid_starter",
    rules_summary: ["Microgrid Starter — intro SL1 scenario."],
    visual_notes: ["Microgrid Starter topology."],
    status: "draft",
  }),
  defineScene({
    id: "museum-conservation-bench",
    title: "Museum Conservation Bench",
    subtitle: "A conservation studio with a main six-node treatment spine and two side branches (wet cleaning and X-ray stabilisation) that rejoin before documentation.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "default_museum_conservation_bench",
    rules_summary: ["Museum Conservation Bench — spine+branches transit loop."],
    visual_notes: ["Museum Conservation Bench topology."],
    status: "ready",
  }),
  defineScene({
    id: "observatory-night-queue",
    title: "Observatory Night Queue",
    subtitle: "A robotic telescope where target requests flow through mount scheduling, CCD exposure, and reduction pipeline to a science archive across a single observing night.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "default_observatory_night_queue",
    rules_summary: ["Observatory Night Queue — easy SL1 scenario."],
    visual_notes: ["Observatory Night Queue topology."],
    status: "draft",
  }),
  defineScene({
    id: "pandemic-supply-web",
    title: "Pandemic Supply Web",
    subtitle: "A pandemic vaccine supply network where the AI director must manage API shortages, formulation surges, fill-finish bottlenecks, and cold-chain disruptions to hit allocation targets.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "default_pandemic_supply_web",
    rules_summary: ["Pandemic Supply Web — hard SL1 scenario."],
    visual_notes: ["Pandemic Supply Web topology."],
    status: "draft",
  }),
  defineScene({
    id: "planetary-defense-array",
    title: "Planetary Defense Array",
    subtitle: "A planetary-defense sensor network: an outer radar octagon with diagonal threat feeds, an inner interceptor pentagon, and a command hub with launch-authorization paths.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "default_planetary_defense_array",
    rules_summary: ["Planetary Defense Array — large radial+ring transit loop."],
    visual_notes: ["Planetary Defense Array topology."],
    status: "ready",
  }),
  defineScene({
    id: "quantum-control-room",
    title: "Quantum Control Room",
    subtitle: "A cryogenic qubit control room laid out as a dense 4×4 mesh of dilution fridges, FPGA controllers, and readout chains with diagonal cross-links.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "default_quantum_control_room",
    rules_summary: ["Quantum Control Room — 4x4 dense mesh transit loop."],
    visual_notes: ["Quantum Control Room topology."],
    status: "ready",
  }),
  defineScene({
    id: "recycling-sort-floor",
    title: "Recycling Sort Floor",
    subtitle: "A recycling sort floor where mixed waste streams are sorted, quarantined for contaminants, and baled for commodity markets under surge pressure.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "default_recycling_sort_floor",
    rules_summary: ["Recycling Sort Floor — easy SL1 scenario."],
    visual_notes: ["Recycling Sort Floor topology."],
    status: "draft",
  }),
  defineScene({
    id: "reef-nursery",
    title: "Reef Nursery",
    subtitle: "An underwater coral-nursery ring linking frag stations, growth tanks, and reef-placement buoys around a tidal oval.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "default_reef_nursery",
    rules_summary: ["Reef Nursery — oval ring transit loop."],
    visual_notes: ["Reef Nursery topology."],
    status: "ready",
  }),
  defineScene({
    id: "regional-blackstart",
    title: "Regional Blackstart",
    subtitle: "A power-system blackstart restoration where a darkened grid is re-energized step by step — cranking buses, selecting feeders, and restoring load zones — while managing frequency stability.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "default_regional_blackstart",
    rules_summary: ["Regional Blackstart — hard SL1 scenario."],
    visual_notes: ["Regional Blackstart topology."],
    status: "draft",
  }),
  defineScene({
    id: "robot-arm-workbench",
    title: "Robot Arm Workbench",
    subtitle: "An assembly workbench where a central workcell dispatches robot arms to seven tool stations in a spoke layout.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "default_robot_arm_workbench",
    rules_summary: ["Robot Arm Workbench — 7-spoke star transit loop."],
    visual_notes: ["Robot Arm Workbench topology."],
    status: "ready",
  }),
  defineScene({
    id: "satellite-downlink-window",
    title: "Satellite Downlink Window",
    subtitle: "A ground station managing satellite pass windows where RF contacts flow from dish antenna through demodulation and telemetry decoding to a science archive.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "default_satellite_downlink_window",
    rules_summary: ["Satellite Downlink Window — medium SL1 scenario."],
    visual_notes: ["Satellite Downlink Window topology."],
    status: "draft",
  }),
  defineScene({
    id: "security-alert-fusion",
    title: "Security Alert Fusion",
    subtitle: "A security operations centre where raw SIEM events are enriched, correlated, triaged by analysts, and published as incident reports under an alert storm.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "default_security_alert_fusion",
    rules_summary: ["Security Alert Fusion — medium SL1 scenario."],
    visual_notes: ["Security Alert Fusion topology."],
    status: "draft",
  }),
  defineScene({
    id: "seed-bank-vault",
    title: "Seed Bank Vault",
    subtitle: "A global seed bank with mirrored left and right cold-storage arms branching from a central vault and intake/drying gateway.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "default_seed_bank_vault",
    rules_summary: ["Seed Bank Vault — mirrored tree transit loop."],
    visual_notes: ["Seed Bank Vault topology."],
    status: "ready",
  }),
  defineScene({
    id: "sensor-calibration-lab",
    title: "Sensor Calibration Lab",
    subtitle: "A metrological calibration laboratory where raw sensor outputs must be corrected against certified reference standards before temperature drift and equipment wear degrade measurement traceability.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "default_sensor_calibration_lab",
    rules_summary: ["Sensor Calibration Lab — intro SL1 scenario."],
    visual_notes: ["Sensor Calibration Lab topology."],
    status: "draft",
  }),
  defineScene({
    id: "stormwater-pump-room",
    title: "Stormwater Pump Room",
    subtitle: "An urban stormwater facility where debris filters, lift stations, and retention basins must keep streets clear during flash-flood events, all while maintaining discharge compliance.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "default_stormwater_pump_room",
    rules_summary: ["Stormwater Pump Room — easy SL1 scenario."],
    visual_notes: ["Stormwater Pump Room topology."],
    status: "draft",
  }),
  defineScene({
    id: "warehouse-cold-chain",
    title: "Warehouse Cold Chain",
    subtitle: "A cold-chain warehouse where temperature-controlled pallets flow from inbound dock through chilling, pick floor, and QC to certified dispatch under surge pressure.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "default_warehouse_cold_chain",
    rules_summary: ["Warehouse Cold Chain — easy SL1 scenario."],
    visual_notes: ["Warehouse Cold Chain topology."],
    status: "draft",
  }),
  defineScene({
    id: "weather-balloon-yard",
    title: "Weather Balloon Yard",
    subtitle: "A radiosonde yard where a central launch tower dispatches balloons through inner prep stations to outer tracking posts.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "default_weather_balloon_yard",
    rules_summary: ["Weather Balloon Yard — radial+outer posts transit loop."],
    visual_notes: ["Weather Balloon Yard topology."],
    status: "ready",
  }),
  defineScene({
    id: "wildfire-watch-grid",
    title: "Wildfire Watch Grid",
    subtitle: "A fire-monitoring network laid out as a 3×4 irregular grid of sensor towers and incident-command posts with diagonal fire-spread shortcuts.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "default_wildfire_watch_grid",
    rules_summary: ["Wildfire Watch Grid — 3x4 irregular grid transit loop."],
    visual_notes: ["Wildfire Watch Grid topology."],
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
