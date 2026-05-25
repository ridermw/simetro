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
    palette_name: "airport_ground_stop",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Airport Ground Stop is a hard AI-operated simulation.",
      "An AI flow manager routes flight plans through gate assignment, pushback, taxiway, and runway sequences while preventing gridlock.",
      "Win by clearing the ground stop before runway queue saturation triggers a loss condition.",
    ],
    visual_notes: [
      "Airport Ground Stop topology: gate nodes, taxiway segments, and runway threshold laid in a hub-and-channel arrangement.",
      "Dark background with amber and blue accent paths marking active taxi routes.",
      "Node shapes distinguish gate types from runway hold positions.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "archive-index-table",
    title: "Archive Index Table",
    subtitle: "A document archive where index trolleys travel a three-level tree of stacks and shelves, returning through a shared reading room.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "archive_index_table",
    rules_summary: [
      "Rendered by the current renderer: Archive Index Table is a 3-level tree transit loop.",
      "Index trolleys travel down a three-level hierarchy of stacks and shelves and return via a shared reading room.",
      "Five movers on distinct routes keep the archive circuit running without collisions.",
    ],
    visual_notes: [
      "Archive Index Table topology: 8 nodes in a symmetric hierarchical tree.",
      "Warm off-white nodes on a dark background with teal and coral accent paths.",
      "Square and triangle shapes distinguish stack nodes from shelf and reading nodes.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "autonomous-farm-season",
    title: "Autonomous Farm Season",
    subtitle: "An autonomous farm laid out as a 5×3 seasonal grid (spring–harvest rows, west–east columns) with horizontal irrigation lanes, vertical growth channels, and diagonal crop-routing shortcuts.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "autonomous_farm_season",
    rules_summary: [
      "Rendered by the current renderer: Autonomous Farm Season is a 5x3 seasonal grid transit loop.",
      "Horizontal irrigation lanes, vertical growth channels, and diagonal shortcuts cross-link the seasonal grid.",
      "Movers traverse harvest and planting routes across the full 15-node canvas.",
    ],
    visual_notes: [
      "Autonomous Farm Season topology: 15 nodes in a 5 by 3 grid with diagonal shortcut paths.",
      "Earthy green and amber accents on a dark background mark spring, summer, and harvest zones.",
      "Circle and hexagon shapes distinguish irrigation hubs from crop-routing waypoints.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "bakery-oven-shift",
    title: "Bakery Oven Shift",
    subtitle: "A commercial bakery where ingredient stores, prep stations, proofing racks, and ovens must keep the sales counter stocked through holiday rushes and oven faults.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "bakery_oven_shift",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Bakery Oven Shift is an easy AI-operated simulation.",
      "Ingredient stores, prep stations, proofing racks, and ovens must keep the sales counter stocked through holiday rushes and oven faults.",
      "Win by maintaining counter stock above the minimum threshold before the shift ends.",
    ],
    visual_notes: [
      "Bakery Oven Shift topology: supply, prep, proof, oven, and sales nodes in a linear kitchen flow.",
      "Warm amber and rose accent paths on a dark background highlight active batch routing.",
      "Circle and square nodes distinguish ingredient stores from process stations.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "bioreactor-balance",
    title: "Bioreactor Balance",
    subtitle: "A bioprocess plant with dual top/bottom pipelines (inoculation→harvest and media→formulation) bridged by three cross-connectors.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "bioreactor_balance",
    rules_summary: [
      "Rendered by the current renderer: Bioreactor Balance is a dual pipeline transit loop.",
      "A top inoculation-to-harvest pipeline and a bottom media-to-formulation pipeline run in parallel with three cross-connectors.",
      "Movers traverse both pipelines and the connectors to demonstrate full circuit coverage.",
    ],
    visual_notes: [
      "Bioreactor Balance topology: two parallel 5-node pipelines bridged by three vertical cross-links.",
      "Teal and magenta accent paths on a dark background distinguish the two pipeline directions.",
      "Diamond and hexagon shapes mark formulation endpoints; circle nodes mark input and harvest stations.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "chip-fab-yield-crisis",
    title: "Chip Fab Yield Crisis",
    subtitle: "A semiconductor fab where wafer lots move through lithography, etching, and inspection to yield analytics during a process-defect crisis demanding rapid lot disposition.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "chip_fab_yield_crisis",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Chip Fab Yield Crisis is a hard AI-operated simulation.",
      "Wafer lots move through lithography, etching, and inspection to yield analytics during a process-defect crisis demanding rapid lot disposition.",
      "Win by recovering yield above the compliance threshold before the defect escalation terminates the run.",
    ],
    visual_notes: [
      "Chip Fab Yield Crisis topology: fab stage nodes connected in a process-flow sequence with inspection branches.",
      "Cool blue and violet accent paths on a dark background mark wafer routing lanes.",
      "Hexagon and diamond shapes distinguish process chambers from inspection and analytics nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "circuit-garden",
    title: "Circuit Garden",
    subtitle: "A printed-circuit-board garden where logic-gate nodes route digital signals through copper-trace paths in a compact grid.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "circuit_garden",
    rules_summary: [
      "Rendered by the current renderer: Circuit Garden is a 3x3 grid transit loop.",
      "Logic-gate nodes are connected by copper-trace paths in a compact printed-circuit topology.",
      "Five movers at varied speeds circulate across the grid to verify all route branches.",
    ],
    visual_notes: [
      "Circuit Garden topology: 8 nodes in a near-3x3 grid with horizontal, vertical, and diagonal trace paths.",
      "Dark background with blue, red, and green accent paths echoing PCB copper-trace colours.",
      "Square, circle, hexagon, diamond, and triangle shapes distinguish gate roles across the grid.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "city-budget-war-room",
    title: "City Budget War Room",
    subtitle: "A municipal budget network with department request nodes, committee review nodes, and treasury allocation nodes in three cross-linked columns.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "city_budget_war_room",
    rules_summary: [
      "Rendered by the current renderer: City Budget War Room is a 3-group columns transit loop.",
      "Department request nodes, committee review nodes, and treasury allocation nodes form three cross-linked columns.",
      "Movers trace the full approval cycle from request to allocation and back.",
    ],
    visual_notes: [
      "City Budget War Room topology: three columns of nodes with horizontal cross-links between review stages.",
      "Amber and teal accent paths on a dark background distinguish request flow from allocation return.",
      "Circle and square shapes separate request nodes from treasury and committee nodes.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "clinic-triage-desk",
    title: "Clinic Triage Desk",
    subtitle: "A busy walk-in clinic where triage nurses, assessment rooms, treatment bays, and discharge coordinators must process patient surges without losing anyone in the queue.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "clinic_triage_desk",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Clinic Triage Desk is an intro AI-operated simulation.",
      "Triage nurses, assessment rooms, treatment bays, and discharge coordinators must process patient surges without losing anyone in the queue.",
      "Win by discharging all patients before the waiting-room capacity breach triggers a loss condition.",
    ],
    visual_notes: [
      "Clinic Triage Desk topology: intake, triage, assessment, treatment, and discharge nodes in a branching patient flow.",
      "Soft blue and coral accent paths on a dark background mark patient routing through care stages.",
      "Circle nodes mark triage points; diamond nodes mark discharge coordination stages.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "crystal-growth-rig",
    title: "Crystal Growth Rig",
    subtitle: "A materials laboratory with a two-row hexagonal lattice of growth columns and solution chambers for crystal synthesis.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "crystal_growth_rig",
    rules_summary: [
      "Rendered by the current renderer: Crystal Growth Rig is a hex lattice transit loop.",
      "Two rows of hexagonal growth columns and solution chambers form a lattice for crystal synthesis routing.",
      "Movers traverse the lattice to simulate crystal batch flow across all chamber connections.",
    ],
    visual_notes: [
      "Crystal Growth Rig topology: two offset rows of nodes forming a hexagonal lattice.",
      "Cool violet and teal accent paths on a dark background mark solution and growth routing lanes.",
      "Hexagon and circle shapes distinguish growth columns from solution-feed chambers.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "datacenter-cooling-surge",
    title: "Datacenter Cooling Surge",
    subtitle: "A datacenter where thermal sensors, chiller plants, and coolant distribution loops must prevent hot-aisle temperature runaway during a compute surge.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "datacenter_cooling_surge",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Datacenter Cooling Surge is a medium AI-operated simulation.",
      "Thermal sensors, chiller plants, and coolant distribution loops must prevent hot-aisle temperature runaway during a compute surge.",
      "Win by keeping all rack temperatures below the critical threshold before thermal runaway triggers a facility fault.",
    ],
    visual_notes: [
      "Datacenter Cooling Surge topology: server rack nodes, chiller nodes, and coolant loop connections.",
      "Cool blue and red accent paths on a dark background contrast active cooling with heat load.",
      "Square nodes mark racks; hexagon nodes mark chiller plants; triangle nodes mark distribution points.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "deep-sea-habitat-grid",
    title: "Deep Sea Habitat Grid",
    subtitle: "An underwater habitat network with an outer pressure-module octagon, inner life-support pentagon, and central command hubs connected by an ROV dock.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "deep_sea_habitat_grid",
    rules_summary: [
      "Rendered by the current renderer: Deep Sea Habitat Grid is an octagon and pentagon transit loop.",
      "An outer pressure-module octagon, inner life-support pentagon, and central command hubs are connected by an ROV dock.",
      "Movers traverse the full ring-and-inner structure simulating habitat resupply circuits.",
    ],
    visual_notes: [
      "Deep Sea Habitat Grid topology: 8-node outer ring, 5-node inner ring, and 2 central hubs with spoke connections.",
      "Deep teal and violet accent paths on a dark ocean-floor background mark pressure and life-support routes.",
      "Circle and diamond shapes distinguish life-support modules from command hubs.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "disaster-supply-staging",
    title: "Disaster Supply Staging",
    subtitle: "A disaster-response staging network with three supply hubs forming a triangle, each dispatching relief convoys to two delivery zones.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "disaster_supply_staging",
    rules_summary: [
      "Rendered by the current renderer: Disaster Supply Staging is a 3-hub triangle transit loop.",
      "Three supply hubs form a triangle, each dispatching relief convoys to two delivery zones.",
      "Movers traverse all three hub-to-zone spokes and the cross-hub ring to simulate full supply routing.",
    ],
    visual_notes: [
      "Disaster Supply Staging topology: 3 hub nodes in a triangle with 6 delivery-zone spoke nodes.",
      "Amber and teal accent paths on a dark background distinguish inbound supply from outbound relief.",
      "Hexagon hubs and circle delivery-zone nodes make the hub-spoke structure legible at a glance.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "drone-repair-bay",
    title: "Drone Repair Bay",
    subtitle: "A UAV maintenance bay with a figure-8 layout: one repair loop (inspect/disassemble/reassemble) and one certification loop (test/flash/calibrate).",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "drone_repair_bay",
    rules_summary: [
      "Rendered by the current renderer: Drone Repair Bay is a figure-8 transit loop.",
      "One repair loop covers inspect, disassemble, and reassemble stations; one certification loop covers test, flash, and calibrate stations.",
      "Movers traverse both loops and the shared crossover node to demonstrate the full figure-8 circuit.",
    ],
    visual_notes: [
      "Drone Repair Bay topology: two interlocking 3-node loops sharing a central crossover node.",
      "Green and violet accent paths on a dark background distinguish the repair and certification loops.",
      "Square nodes mark repair stations; diamond nodes mark certification stages.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "fabric-dye-lab",
    title: "Fabric Dye Lab",
    subtitle: "A textile dye laboratory with three parallel dye chains (acid, reactive, vat) whose intake and fix ends are cross-linked for batch routing.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "fabric_dye_lab",
    rules_summary: [
      "Rendered by the current renderer: Fabric Dye Lab is a 3 parallel chains transit loop.",
      "Acid, reactive, and vat dye chains run in parallel with intake and fix ends cross-linked for batch routing.",
      "Movers traverse all three chains and the cross-links to simulate dye-batch balancing.",
    ],
    visual_notes: [
      "Fabric Dye Lab topology: three parallel 4-node chains with cross-links at both ends.",
      "Magenta, teal, and amber accent paths on a dark background distinguish the three dye chemistry lines.",
      "Square intake nodes and circle fix nodes bookend each chain with distinct shapes.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "food-bank-allocation",
    title: "Food Bank Allocation",
    subtitle: "A food bank where donation batches are sorted, cold-stored, packed into delivery boxes, and allocated under surge donation pressure.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "food_bank_allocation",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Food Bank Allocation is a medium AI-operated simulation.",
      "Donation batches are sorted, cold-stored, packed into delivery boxes, and allocated under surge donation pressure.",
      "Win by delivering the required allocation volume before cold-storage overflow causes a spoilage fault.",
    ],
    visual_notes: [
      "Food Bank Allocation topology: intake, sort, cold-store, pack, and dispatch nodes in a branching allocation flow.",
      "Warm amber and green accent paths on a dark background mark donation intake and dispatch routes.",
      "Triangle intake nodes and hexagon cold-storage nodes stand out against square packing station nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "forge-heat-map",
    title: "Forge Heat Map",
    subtitle: "A metal forge where billets travel an outer heat-map ring with hot shortcuts to inner quench stations.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "forge_heat_map",
    rules_summary: [
      "Rendered by the current renderer: Forge Heat Map is a ring and inner stations transit loop.",
      "Billets travel an outer heat-map ring with hot shortcuts to inner quench stations.",
      "Movers traverse both the outer ring and the inner spoke paths at varied speeds.",
    ],
    visual_notes: [
      "Forge Heat Map topology: 8-node outer ring with 4 inner quench station nodes linked by spokes.",
      "Red-orange and cool-blue accent paths on a dark background contrast heat routing with quench shortcuts.",
      "Circle and square outer nodes contrast with hexagon and triangle inner station nodes.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "fusion-shot-campaign",
    title: "Fusion Shot Campaign",
    subtitle: "An inertial-confinement fusion campaign where the AI shot director sequences laser charges, target injection, optical alignment, and ignition to maximise yield measurements.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "fusion_shot_campaign",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Fusion Shot Campaign is a hard AI-operated simulation.",
      "A shot director sequences laser charges, target injection, optical alignment, and ignition to maximise yield measurements.",
      "Win by recording the required number of high-yield shots before the campaign window closes.",
    ],
    visual_notes: [
      "Fusion Shot Campaign topology: laser, injection, alignment, and ignition nodes in a shot-sequence ring.",
      "Electric blue and gold accent paths on a dark background mark active laser and ignition routes.",
      "Diamond nodes mark alignment checkpoints; hexagon nodes mark the ignition chamber.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "greenhouse-water-watch",
    title: "Greenhouse Water Watch",
    subtitle: "An automated greenhouse where weather sensors, water pumps, soil monitors, and irrigation grids must keep every grow bed watered through drought spells and pump failures.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "greenhouse_water_watch",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Greenhouse Water Watch is an intro AI-operated simulation.",
      "Weather sensors, water pumps, soil monitors, and irrigation grids must keep every grow bed watered through drought spells and pump failures.",
      "Win by keeping all grow beds above the minimum soil-moisture level until the end of the day.",
    ],
    visual_notes: [
      "Greenhouse Water Watch topology: sensor, pump, monitor, and bed nodes in an irrigation network.",
      "Leaf-green and sky-blue accent paths on a dark background mark active irrigation and sensor routes.",
      "Circle sensor nodes and square pump nodes contrast with hexagon grow-bed nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "hospital-bed-command",
    title: "Hospital Bed Command",
    subtitle: "A hospital bed command centre where admission requests flow through ward assignment, transfer orders, and discharge planning to maintain live capacity dashboards under surge.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "hospital_bed_command",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Hospital Bed Command is a medium AI-operated simulation.",
      "Admission requests flow through ward assignment, transfer orders, and discharge planning to maintain live capacity dashboards under surge.",
      "Win by keeping ward occupancy below the critical threshold throughout the surge window.",
    ],
    visual_notes: [
      "Hospital Bed Command topology: admission, ward, transfer, and discharge nodes in a capacity management network.",
      "Calm blue and amber accent paths on a dark background distinguish admission flows from discharge planning.",
      "Hexagon ward nodes and triangle discharge nodes stand out against circle admission and transfer nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "kitchen-prep-board",
    title: "Kitchen Prep Board",
    subtitle: "A restaurant prep board where ingredient stations, chopping blocks, grills, and plating zones form a branching kitchen flow.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "kitchen_prep_board",
    rules_summary: [
      "Rendered by the current renderer: Kitchen Prep Board is a spine and branches transit loop.",
      "Ingredient stations, chopping blocks, grills, and plating zones form a branching kitchen flow with fridge and fryer side branches.",
      "Five movers at varied speeds cover all kitchen routes from cold storage to plating.",
    ],
    visual_notes: [
      "Kitchen Prep Board topology: 9 nodes in a main spine with two side branches for fridge and fryer stations.",
      "Cyan and coral accent paths on a dark background distinguish the main cook line from side prep branches.",
      "Circle and square nodes mark ingredient stores; hexagon and diamond nodes mark heat and plating stations.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "library-reshelving-clock",
    title: "Library Reshelving Clock",
    subtitle: "A public library where a returning tide of books must be sorted, catalogued, shelved, and surfaced as availability reports before the reading-room backlog grows unmanageable.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "library_reshelving_clock",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Library Reshelving Clock is an intro AI-operated simulation.",
      "A returning tide of books must be sorted, catalogued, shelved, and surfaced as availability reports before the reading-room backlog grows unmanageable.",
      "Win by clearing the backlog and publishing all availability reports before the library closes.",
    ],
    visual_notes: [
      "Library Reshelving Clock topology: return, sort, catalogue, shelf, and report nodes in a processing pipeline.",
      "Warm amber and teal accent paths on a dark background mark the book-return and shelving flows.",
      "Circle return nodes and square shelf nodes contrast with diamond report publication nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "microgrid-starter",
    title: "Microgrid Starter",
    subtitle: "A rooftop solar microgrid where battery banks, inverters, and local distribution must serve peak loads through cloud cover and inverter thermal limits.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "microgrid_starter",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Microgrid Starter is an intro AI-operated simulation.",
      "Battery banks, inverters, and local distribution must serve peak loads through cloud cover and inverter thermal limits.",
      "Win by meeting all load demands throughout the cloud-cover event without a blackout.",
    ],
    visual_notes: [
      "Microgrid Starter topology: solar panel, battery, inverter, and load nodes in a small distribution network.",
      "Yellow and blue accent paths on a dark background mark generation and load distribution routes.",
      "Diamond nodes mark inverter stages; hexagon nodes mark battery banks; circle nodes mark load endpoints.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "museum-conservation-bench",
    title: "Museum Conservation Bench",
    subtitle: "A conservation studio with a main six-node treatment spine and two side branches (wet cleaning and X-ray stabilisation) that rejoin before documentation.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "museum_conservation_bench",
    rules_summary: [
      "Rendered by the current renderer: Museum Conservation Bench is a spine and branches transit loop.",
      "A six-node main treatment spine is flanked by wet-cleaning and X-ray stabilisation branches that rejoin before documentation.",
      "Movers traverse the spine, both branches, and the documentation endpoint to cover all treatment routes.",
    ],
    visual_notes: [
      "Museum Conservation Bench topology: 8 nodes in a main treatment spine with two side-branch loops.",
      "Warm ivory and teal accent paths on a dark background distinguish the main spine from treatment side branches.",
      "Hexagon and triangle nodes mark specialist treatment stages; circle nodes mark intake and documentation.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "observatory-night-queue",
    title: "Observatory Night Queue",
    subtitle: "A robotic telescope where target requests flow through mount scheduling, CCD exposure, and reduction pipeline to a science archive across a single observing night.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "observatory_night_queue",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Observatory Night Queue is an easy AI-operated simulation.",
      "Target requests flow through mount scheduling, CCD exposure, and reduction pipeline to a science archive across a single observing night.",
      "Win by archiving all scheduled targets before dawn ends the observing window.",
    ],
    visual_notes: [
      "Observatory Night Queue topology: queue, mount, CCD, reduction, and archive nodes in an observation pipeline.",
      "Deep indigo and gold accent paths on a dark background mark telescope pointing and data archiving routes.",
      "Circle mount nodes and hexagon archive nodes contrast with square CCD and diamond reduction nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "pandemic-supply-web",
    title: "Pandemic Supply Web",
    subtitle: "A pandemic vaccine supply network where the AI director must manage API shortages, formulation surges, fill-finish bottlenecks, and cold-chain disruptions to hit allocation targets.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "pandemic_supply_web",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Pandemic Supply Web is a hard AI-operated simulation.",
      "An AI director manages API shortages, formulation surges, fill-finish bottlenecks, and cold-chain disruptions to hit vaccine allocation targets.",
      "Win by delivering the required allocation doses before cold-chain failures cause a supply crisis.",
    ],
    visual_notes: [
      "Pandemic Supply Web topology: API, formulation, fill-finish, cold-chain, and distribution nodes in a multi-stage supply network.",
      "Clinical blue and amber accent paths on a dark background mark supply flow and cold-chain routing.",
      "Square manufacturing nodes and triangle cold-chain nodes contrast with hexagon distribution nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "planetary-defense-array",
    title: "Planetary Defense Array",
    subtitle: "A planetary-defense sensor network: an outer radar octagon with diagonal threat feeds, an inner interceptor pentagon, and a command hub with launch-authorization paths.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "planetary_defense_array",
    rules_summary: [
      "Rendered by the current renderer: Planetary Defense Array is a large radial and ring transit loop.",
      "An outer radar octagon feeds diagonal threat data to an inner interceptor pentagon and a central command hub with launch-authorization paths.",
      "Movers traverse all ring, spoke, and diagonal routes to simulate full sensor-to-command coverage.",
    ],
    visual_notes: [
      "Planetary Defense Array topology: 8-node outer ring, 5-node inner ring, central command, and diagonal threat-feed shortcuts.",
      "Electric blue and red accent paths on a dark background mark radar scanning and interceptor launch routes.",
      "Circle radar nodes and diamond command nodes contrast with hexagon interceptor nodes.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "quantum-control-room",
    title: "Quantum Control Room",
    subtitle: "A cryogenic qubit control room laid out as a dense 4×4 mesh of dilution fridges, FPGA controllers, and readout chains with diagonal cross-links.",
    world_kind: "transit_loop",
    difficulty: "hard",
    palette_name: "quantum_control_room",
    rules_summary: [
      "Rendered by the current renderer: Quantum Control Room is a 4x4 dense mesh transit loop.",
      "A dense grid of dilution fridges, FPGA controllers, and readout chains is connected with diagonal cross-links.",
      "Movers traverse both grid-aligned and diagonal paths to simulate qubit control circuit coverage.",
    ],
    visual_notes: [
      "Quantum Control Room topology: 16 nodes in a 4x4 mesh with horizontal, vertical, and diagonal cross-links.",
      "Cool cyan and violet accent paths on a dark background mark cryogenic control and readout routes.",
      "Square FPGA nodes and circle fridge nodes contrast with diamond readout chain nodes.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "recycling-sort-floor",
    title: "Recycling Sort Floor",
    subtitle: "A recycling sort floor where mixed waste streams are sorted, quarantined for contaminants, and baled for commodity markets under surge pressure.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "recycling_sort_floor",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Recycling Sort Floor is an easy AI-operated simulation.",
      "Mixed waste streams are sorted, quarantined for contaminants, and baled for commodity markets under surge pressure.",
      "Win by clearing all intake before the contamination quarantine overflows and halts sorting.",
    ],
    visual_notes: [
      "Recycling Sort Floor topology: intake, sort, quarantine, baling, and dispatch nodes in a branching sort pipeline.",
      "Green and amber accent paths on a dark background mark sorting flow and contaminant quarantine routes.",
      "Triangle quarantine nodes and hexagon baling nodes contrast with circle intake and dispatch nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "reef-nursery",
    title: "Reef Nursery",
    subtitle: "An underwater coral-nursery ring linking frag stations, growth tanks, and reef-placement buoys around a tidal oval.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "reef_nursery",
    rules_summary: [
      "Rendered by the current renderer: Reef Nursery is an oval ring transit loop.",
      "Frag stations, growth tanks, and reef-placement buoys are linked around a tidal oval with a central cross-shortcut.",
      "Five movers at varied speeds circulate the ring and cross-shortcut to simulate full nursery circuit flow.",
    ],
    visual_notes: [
      "Reef Nursery topology: 9 nodes arranged in an oval ring with one diagonal shortcut path.",
      "Ocean blue and magenta accent paths on a dark background distinguish growth routes from reef-placement shortcuts.",
      "Circle and triangle nodes mark frag and placement stations; diamond, hexagon, and square nodes mark growth stages.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "regional-blackstart",
    title: "Regional Blackstart",
    subtitle: "A power-system blackstart restoration where a darkened grid is re-energized step by step — cranking buses, selecting feeders, and restoring load zones — while managing frequency stability.",
    world_kind: "sl1_scenario",
    difficulty: "hard",
    palette_name: "regional_blackstart",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Regional Blackstart is a hard AI-operated simulation.",
      "A darkened grid is re-energized step by step — cranking buses, selecting feeders, and restoring load zones — while managing frequency stability.",
      "Win by restoring all load zones within the blackstart window before frequency instability causes a cascade failure.",
    ],
    visual_notes: [
      "Regional Blackstart topology: cranking bus, feeder, and load-zone nodes in a sequential power-restoration network.",
      "Electric yellow and deep blue accent paths on a dark background mark energization sequence and feeder routing.",
      "Hexagon cranking nodes and square feeder nodes contrast with circle and triangle load-zone nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "robot-arm-workbench",
    title: "Robot Arm Workbench",
    subtitle: "An assembly workbench where a central workcell dispatches robot arms to seven tool stations in a spoke layout.",
    world_kind: "transit_loop",
    difficulty: "intro",
    palette_name: "robot_arm_workbench",
    rules_summary: [
      "Rendered by the current renderer: Robot Arm Workbench is a 7-spoke star transit loop.",
      "A central workcell dispatches robot arms to seven tool stations in a spoke layout with two cross-links between adjacent tool stations.",
      "Five movers traverse arm spokes and cross-links to simulate concurrent assembly operations.",
    ],
    visual_notes: [
      "Robot Arm Workbench topology: 1 central workcell hub with 7 tool station nodes in a radial spoke arrangement.",
      "Teal and magenta accent paths on a dark background distinguish arm dispatch spokes from tool-station cross-links.",
      "Hexagon workcell node contrasts with circle, square, diamond, and triangle tool station nodes.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "satellite-downlink-window",
    title: "Satellite Downlink Window",
    subtitle: "A ground station managing satellite pass windows where RF contacts flow from dish antenna through demodulation and telemetry decoding to a science archive.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "satellite_downlink_window",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Satellite Downlink Window is a medium AI-operated simulation.",
      "RF contacts flow from dish antenna through demodulation and telemetry decoding to a science archive across satellite pass windows.",
      "Win by capturing and archiving all scheduled contact passes before the communication window closes.",
    ],
    visual_notes: [
      "Satellite Downlink Window topology: antenna, demodulation, telemetry, and archive nodes in a signal-processing pipeline.",
      "Electric blue and gold accent paths on a dark background mark RF contact flow and archive routing.",
      "Circle antenna nodes and diamond archive nodes contrast with square demodulation and hexagon telemetry nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "security-alert-fusion",
    title: "Security Alert Fusion",
    subtitle: "A security operations centre where raw SIEM events are enriched, correlated, triaged by analysts, and published as incident reports under an alert storm.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    palette_name: "security_alert_fusion",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Security Alert Fusion is a medium AI-operated simulation.",
      "Raw SIEM events are enriched, correlated, triaged by analysts, and published as incident reports under an alert storm.",
      "Win by publishing all critical incidents before the alert queue saturates the analyst capacity.",
    ],
    visual_notes: [
      "Security Alert Fusion topology: SIEM, enrichment, correlation, triage, and reporting nodes in a SOC pipeline.",
      "Red and cyan accent paths on a dark background mark alert intake and incident publication routes.",
      "Triangle SIEM nodes and hexagon reporting nodes contrast with circle enrichment and square triage nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "seed-bank-vault",
    title: "Seed Bank Vault",
    subtitle: "A global seed bank with mirrored left and right cold-storage arms branching from a central vault and intake/drying gateway.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "seed_bank_vault",
    rules_summary: [
      "Rendered by the current renderer: Seed Bank Vault is a mirrored tree transit loop.",
      "Mirrored left and right cold-storage arms branch from a central vault and intake/drying gateway.",
      "Movers traverse both arms and the central gateway to simulate dual-wing seed circuit flow.",
    ],
    visual_notes: [
      "Seed Bank Vault topology: a central gateway and vault with symmetric left and right cold-storage branch arms.",
      "Icy blue and warm amber accent paths on a dark background distinguish intake drying from cold-storage routing.",
      "Hexagon vault nodes and triangle cold-storage nodes contrast with circle intake and square drying nodes.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "sensor-calibration-lab",
    title: "Sensor Calibration Lab",
    subtitle: "A metrological calibration laboratory where raw sensor outputs must be corrected against certified reference standards before temperature drift and equipment wear degrade measurement traceability.",
    world_kind: "sl1_scenario",
    difficulty: "intro",
    palette_name: "sensor_calibration_lab",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Sensor Calibration Lab is an intro AI-operated simulation.",
      "Raw sensor outputs must be corrected against certified reference standards before temperature drift and equipment wear degrade measurement traceability.",
      "Win by keeping all sensor calibration within tolerance before the reference standard certification expires.",
    ],
    visual_notes: [
      "Sensor Calibration Lab topology: raw input, correction, reference standard, and certification nodes in a calibration pipeline.",
      "Neutral grey and blue accent paths on a dark background mark calibration flow and reference standard routing.",
      "Diamond reference nodes and circle input nodes contrast with square correction and hexagon certification nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "stormwater-pump-room",
    title: "Stormwater Pump Room",
    subtitle: "An urban stormwater facility where debris filters, lift stations, and retention basins must keep streets clear during flash-flood events, all while maintaining discharge compliance.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "stormwater_pump_room",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Stormwater Pump Room is an easy AI-operated simulation.",
      "Debris filters, lift stations, and retention basins must keep streets clear during flash-flood events while maintaining discharge compliance.",
      "Win by preventing street flooding and keeping discharge within compliance limits throughout the storm event.",
    ],
    visual_notes: [
      "Stormwater Pump Room topology: filter, lift station, basin, and discharge nodes in a drainage network.",
      "Slate blue and green accent paths on a dark background mark drainage flow and compliance monitoring routes.",
      "Hexagon retention basin nodes and circle lift station nodes contrast with triangle filter and square discharge nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "warehouse-cold-chain",
    title: "Warehouse Cold Chain",
    subtitle: "A cold-chain warehouse where temperature-controlled pallets flow from inbound dock through chilling, pick floor, and QC to certified dispatch under surge pressure.",
    world_kind: "sl1_scenario",
    difficulty: "easy",
    palette_name: "warehouse_cold_chain",
    rules_summary: [
      "Uses the scenario_language_v1 grammar: Warehouse Cold Chain is an easy AI-operated simulation.",
      "Temperature-controlled pallets flow from inbound dock through chilling, pick floor, and QC to certified dispatch under surge pressure.",
      "Win by dispatching all certified pallets before cold-chain temperature violations cause product loss.",
    ],
    visual_notes: [
      "Warehouse Cold Chain topology: dock, chill, pick, QC, and dispatch nodes in a cold-chain logistics pipeline.",
      "Ice blue and amber accent paths on a dark background distinguish cold-zone routing from dispatch lanes.",
      "Circle dock nodes and hexagon chill nodes contrast with square pick and diamond QC dispatch nodes.",
    ],
    status: "draft",
  }),
  defineScene({
    id: "weather-balloon-yard",
    title: "Weather Balloon Yard",
    subtitle: "A radiosonde yard where a central launch tower dispatches balloons through inner prep stations to outer tracking posts.",
    world_kind: "transit_loop",
    difficulty: "easy",
    palette_name: "weather_balloon_yard",
    rules_summary: [
      "Rendered by the current renderer: Weather Balloon Yard is a radial and outer posts transit loop.",
      "A central launch tower dispatches balloons through inner prep stations to outer tracking posts in a radial arrangement.",
      "Movers traverse all launch spokes and outer tracking connections to simulate full sounding-balloon coverage.",
    ],
    visual_notes: [
      "Weather Balloon Yard topology: 1 central launch tower, 4 inner prep stations, and 4 outer tracking posts in a radial layout.",
      "Sky blue and amber accent paths on a dark background distinguish launch spokes from tracking post connections.",
      "Hexagon launch tower contrasts with circle prep station and square tracking post nodes.",
    ],
    status: "ready",
  }),
  defineScene({
    id: "wildfire-watch-grid",
    title: "Wildfire Watch Grid",
    subtitle: "A fire-monitoring network laid out as a 3×4 irregular grid of sensor towers and incident-command posts with diagonal fire-spread shortcuts.",
    world_kind: "transit_loop",
    difficulty: "medium",
    palette_name: "wildfire_watch_grid",
    rules_summary: [
      "Rendered by the current renderer: Wildfire Watch Grid is a 3x4 irregular grid transit loop.",
      "Sensor towers and incident-command posts are laid in a 3x4 grid with diagonal fire-spread shortcuts.",
      "Movers traverse grid paths and shortcuts to simulate real-time fire-monitoring sweep coverage.",
    ],
    visual_notes: [
      "Wildfire Watch Grid topology: 12 nodes in a 3 by 4 grid with diagonal shortcut connections.",
      "Flame orange and forest green accent paths on a dark background mark fire-spread shortcuts and monitoring routes.",
      "Circle sensor tower nodes and triangle incident-command nodes make the monitoring network legible.",
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
