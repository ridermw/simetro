// frontend/src/catalog/scenes.ts
//
// Static gallery metadata for local scenes. These strings are content, not
// markup; future UI should render them with textContent only.

export type SceneWorldKind = "transit_loop";
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
] as const satisfies readonly SceneCatalogEntry[];

export type SceneCatalogId = (typeof SCENE_CATALOG)[number]["id"];

export function findSceneById(id: string): SceneCatalogEntry | undefined {
  return SCENE_CATALOG.find((scene) => scene.id === id);
}

export function isLocalScenePath(path: string): boolean {
  return /^games\/[a-z0-9][a-z0-9_-]*\.json$/.test(path);
}
