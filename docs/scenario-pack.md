# Complex Scenario Pack

This pack adds 40 complex local scenarios built with the abilities currently available on `main`. It intentionally keeps the existing visibility model and agent behavior unchanged; the separate agent/visibility worktree can decide which scenes to show or hide later.

## Pack shape

- 40 new `games/*.json` scenarios.
- 20 `scenario_language_v1` draft systems-game scenarios.
- 20 ready legacy-rendered systems worlds using the current Canvas renderer.
- Exactly 10 new scenarios per catalog difficulty: intro, easy, medium, hard.
- All scenarios are registry-backed by `scene_id`; no frontend path selection is introduced.

## How to inspect

Run the app from this branch and use the scene browser to select the new ready rendered worlds. SL1 entries are registered and loadable, but remain `draft` until the SL1 renderer/agent work lands.

```bash
cd frontend && npm run build
cd ../src-tauri && cargo run
```

## Scenario catalog

### Intro

| Scenario | Kind | Shape | Description |
| --- | --- | --- | --- |
| `clinic-triage-desk`<br>Clinic Triage Desk | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 2 pressure | A busy walk-in clinic where triage nurses, assessment rooms, treatment bays, and discharge coordinators must process patient surges without losing anyone in the queue. Agents balance throughput against staff capacity limits. |
| `greenhouse-water-watch`<br>Greenhouse Water Watch | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 2 pressure | An automated greenhouse where weather sensors, water pumps, soil monitors, and irrigation grids must keep every grow bed watered through drought spells and pump failures. |
| `library-reshelving-clock`<br>Library Reshelving Clock | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 2 pressure | A public library where a returning tide of books must be sorted, catalogued, shelved, and surfaced as availability reports before the reading-room backlog grows unmanageable. |
| `microgrid-starter`<br>Microgrid Starter | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 2 pressure | A rooftop solar microgrid where battery banks, inverters, and local distribution must serve peak loads through cloud cover and inverter thermal limits. |
| `sensor-calibration-lab`<br>Sensor Calibration Lab | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 2 pressure | A metrological calibration laboratory where raw sensor outputs must be corrected against certified reference standards before temperature drift and equipment wear degrade measurement traceability. |
| `circuit-garden`<br>Circuit Garden | Rendered ready | 8 nodes, 11 paths, 5 movers | A printed-circuit-board garden where logic-gate nodes route digital signals through copper-trace paths in a compact grid. |
| `kitchen-prep-board`<br>Kitchen Prep Board | Rendered ready | 9 nodes, 10 paths, 5 movers | A restaurant prep board where ingredient stations, chopping blocks, grills, and plating zones form a branching kitchen flow. |
| `archive-index-table`<br>Archive Index Table | Rendered ready | 8 nodes, 10 paths, 5 movers | A document archive where index trolleys travel a three-level tree of stacks and shelves, returning through a shared reading room. |
| `reef-nursery`<br>Reef Nursery | Rendered ready | 9 nodes, 10 paths, 5 movers | An underwater coral-nursery ring linking frag stations, growth tanks, and reef-placement buoys around a tidal oval. |
| `robot-arm-workbench`<br>Robot Arm Workbench | Rendered ready | 8 nodes, 10 paths, 5 movers | An assembly workbench where a central workcell dispatches robot arms to seven tool stations in a spoke layout. |

### Easy

| Scenario | Kind | Shape | Description |
| --- | --- | --- | --- |
| `stormwater-pump-room`<br>Stormwater Pump Room | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | An urban stormwater facility where debris filters, lift stations, and retention basins must keep streets clear during flash-flood events, all while maintaining discharge compliance. |
| `bakery-oven-shift`<br>Bakery Oven Shift | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 2 pressure | A commercial bakery where ingredient stores, prep stations, proofing racks, and ovens must keep the sales counter stocked through holiday rushes and oven faults. |
| `warehouse-cold-chain`<br>Warehouse Cold Chain | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | A cold-chain warehouse where temperature-controlled pallets flow from inbound dock through chilling, pick floor, and QC to certified dispatch under surge pressure. |
| `observatory-night-queue`<br>Observatory Night Queue | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | A robotic telescope where target requests flow through mount scheduling, CCD exposure, and reduction pipeline to a science archive across a single observing night. |
| `recycling-sort-floor`<br>Recycling Sort Floor | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | A recycling sort floor where mixed waste streams are sorted, quarantined for contaminants, and baled for commodity markets under surge pressure. |
| `forge-heat-map`<br>Forge Heat Map | Rendered ready | 11 nodes, 15 paths, 5 movers | A metal forge where billets travel an outer heat-map ring with hot shortcuts to inner quench stations. |
| `seed-bank-vault`<br>Seed Bank Vault | Rendered ready | 11 nodes, 12 paths, 5 movers | A global seed bank with mirrored left and right cold-storage arms branching from a central vault and intake/drying gateway. |
| `drone-repair-bay`<br>Drone Repair Bay | Rendered ready | 10 nodes, 11 paths, 5 movers | A UAV maintenance bay with a figure-8 layout: one repair loop (inspect/disassemble/reassemble) and one certification loop (test/flash/calibrate). |
| `weather-balloon-yard`<br>Weather Balloon Yard | Rendered ready | 11 nodes, 12 paths, 5 movers | A radiosonde yard where a central launch tower dispatches balloons through inner prep stations to outer tracking posts. |
| `crystal-growth-rig`<br>Crystal Growth Rig | Rendered ready | 10 nodes, 16 paths, 5 movers | A materials laboratory with a two-row hexagonal lattice of growth columns and solution chambers for crystal synthesis. |

### Medium

| Scenario | Kind | Shape | Description |
| --- | --- | --- | --- |
| `datacenter-cooling-surge`<br>Datacenter Cooling Surge | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | A datacenter where thermal sensors, chiller plants, and coolant distribution loops must prevent hot-aisle temperature runaway during a compute surge. |
| `hospital-bed-command`<br>Hospital Bed Command | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | A hospital bed command centre where admission requests flow through ward assignment, transfer orders, and discharge planning to maintain live capacity dashboards under surge. |
| `food-bank-allocation`<br>Food Bank Allocation | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | A food bank where donation batches are sorted, cold-stored, packed into delivery boxes, and allocated under surge donation pressure. |
| `security-alert-fusion`<br>Security Alert Fusion | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | A security operations centre where raw SIEM events are enriched, correlated, triaged by analysts, and published as incident reports under an alert storm. |
| `satellite-downlink-window`<br>Satellite Downlink Window | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | A ground station managing satellite pass windows where RF contacts flow from dish antenna through demodulation and telemetry decoding to a science archive. |
| `bioreactor-balance`<br>Bioreactor Balance | Rendered ready | 13 nodes, 14 paths, 6 movers | A bioprocess plant with dual top/bottom pipelines (inoculation->harvest and media->formulation) bridged by three cross-connectors. |
| `disaster-supply-staging`<br>Disaster Supply Staging | Rendered ready | 12 nodes, 13 paths, 6 movers | A disaster-response staging network with three supply hubs forming a triangle, each dispatching relief convoys to two delivery zones. |
| `fabric-dye-lab`<br>Fabric Dye Lab | Rendered ready | 12 nodes, 15 paths, 6 movers | A textile dye laboratory with three parallel dye chains (acid, reactive, vat) whose intake and fix ends are cross-linked for batch routing. |
| `museum-conservation-bench`<br>Museum Conservation Bench | Rendered ready | 12 nodes, 13 paths, 6 movers | A conservation studio with a main six-node treatment spine and two side branches (wet cleaning and X-ray stabilisation) that rejoin before documentation. |
| `wildfire-watch-grid`<br>Wildfire Watch Grid | Rendered ready | 12 nodes, 23 paths, 6 movers | A fire-monitoring network laid out as a 3x4 irregular grid of sensor towers and incident-command posts with diagonal fire-spread shortcuts. |

### Hard

| Scenario | Kind | Shape | Description |
| --- | --- | --- | --- |
| `chip-fab-yield-crisis`<br>Chip Fab Yield Crisis | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | A semiconductor fab where wafer lots move through lithography, etching, and inspection to yield analytics during a process-defect crisis demanding rapid lot disposition. |
| `regional-blackstart`<br>Regional Blackstart | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | A power-system blackstart restoration where a darkened grid is re-energized step by step -- cranking buses, selecting feeders, and restoring load zones -- while managing frequency stability. |
| `airport-ground-stop`<br>Airport Ground Stop | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | An airport ground stop where an AI flow manager must route flight plans through gate assignment, pushback, taxiway sequencing, and runway departure queues while preventing gridlock. |
| `pandemic-supply-web`<br>Pandemic Supply Web | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | A pandemic vaccine supply network where the AI director must manage API shortages, formulation surges, fill-finish bottlenecks, and cold-chain disruptions to hit allocation targets. |
| `fusion-shot-campaign`<br>Fusion Shot Campaign | SL1 draft | 6 places, 6 links, 5 things, 4 transforms, 2 demand, 3 pressure | An inertial-confinement fusion campaign where the AI shot director sequences laser charges, target injection, optical alignment, and ignition to maximise yield measurements. |
| `quantum-control-room`<br>Quantum Control Room | Rendered ready | 16 nodes, 32 paths, 7 movers | A cryogenic qubit control room laid out as a dense 4x4 mesh of dilution fridges, FPGA controllers, and readout chains with diagonal cross-links. |
| `deep-sea-habitat-grid`<br>Deep Sea Habitat Grid | Rendered ready | 15 nodes, 25 paths, 7 movers | An underwater habitat network with an outer pressure-module octagon, inner life-support pentagon, and central command hubs connected by an ROV dock. |
| `city-budget-war-room`<br>City Budget War Room | Rendered ready | 15 nodes, 25 paths, 7 movers | A municipal budget network with department request nodes, committee review nodes, and treasury allocation nodes in three cross-linked columns. |
| `planetary-defense-array`<br>Planetary Defense Array | Rendered ready | 16 nodes, 26 paths, 7 movers | A planetary-defense sensor network: an outer radar octagon with diagonal threat feeds, an inner interceptor pentagon, and a command hub with launch-authorization paths. |
| `autonomous-farm-season`<br>Autonomous Farm Season | Rendered ready | 15 nodes, 29 paths, 7 movers | An autonomous farm laid out as a 5x3 seasonal grid (spring-harvest rows, west-east columns) with horizontal irrigation lanes, vertical growth channels, and diagonal crop-routing shortcuts. |

## Validation

The pack is covered by catalog invariants and the world-quality checklist:

- Exact 40-scene pack membership.
- Exact 20/20 SL1 vs rendered split.
- Exact 10-per-difficulty distribution.
- Catalog, Tauri registry, and `games/*.json` slug alignment.
- Loader/world-quality checks for each authored scene.
