// frontend/src/renderer/sl1_synth.ts
//
// scenario_language_v1 (SL1) geometry synthesis.
//
// Most simetro scenes carry their renderable geometry in
// `StaticPayload.nodes` and `StaticPayload.paths`. SL1 scenes
// currently leave those empty and put their semantic geometry in
// `sl1_places` and `sl1_links` instead — the engine doesn't yet have
// a transport/queue runtime that would populate the legacy node/path
// arrays for SL1 worlds.
//
// To make SL1 scenes visible on the canvas (and in gallery
// thumbnails) without waiting for the engine-side transport runtime,
// we synthesize NodeView/PathView records from the SL1 metadata at
// the frontend boundary. This is purely a render-time projection;
// the engine remains unaware and deterministic.
//
// Place role → node shape / palette index. Visible-stakes principle:
// the four roles in the current scene pack (source, compute_cluster,
// dashboard, operator) get distinct shapes so a viewer can tell at a
// glance which place is which without reading text.

import type { NodeView, PathView, StaticPayload, NodeShapeTag } from "../protocol/messages";

interface RoleHint {
  shape: NodeShapeTag;
  /** Preferred palette index. Renderer clamps to palette length. */
  color: number;
}

// Palette index 0 is the background; foreground is index 1. Other
// indices are scene-specific theme colors. The hints below assume the
// SL1 scene palette has at least 4 entries (background + 3 hues),
// which the SL1 GPU Launch Week palette guarantees (7 entries).
const ROLE_HINTS: Readonly<Record<string, RoleHint>> = {
  source: { shape: "circle", color: 2 },
  compute_cluster: { shape: "hexagon", color: 4 },
  dashboard: { shape: "square", color: 5 },
  operator: { shape: "diamond", color: 6 },
};

const FALLBACK_HINT: RoleHint = { shape: "circle", color: 1 };

/** Locale-independent string comparator for stable id ordering across
 *  any JS runtime/locale. Use this instead of String.localeCompare. */
function compareIds(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/** Synthesize NodeView/PathView from sl1_places/sl1_links when the
 *  legacy node/path arrays are empty. Returns the original payload
 *  unchanged if no synthesis is needed (non-SL1 scenes, SL1 scenes
 *  that already have legacy geometry, or SL1 scenes with no places).
 *
 *  Duplicate place ids: only the first occurrence (after deterministic
 *  sort) is rendered; subsequent duplicates are skipped to avoid
 *  ambiguous link endpoints. Schema should also reject duplicates,
 *  but synthesis is defensive. */
export function synthesizeSl1Geometry(payload: StaticPayload): StaticPayload {
  if (payload.nodes.length > 0) return payload;
  const places = payload.sl1_places ?? [];
  if (places.length === 0) return payload;

  // Stable, locale-independent sort by place id for deterministic
  // synthetic ids across environments.
  const sortedPlaces = [...places].sort((a, b) => compareIds(a.id, b.id));
  const idByPlace = new Map<string, number>();
  const posByPlace = new Map<string, [number, number]>();
  const colorByPlace = new Map<string, number>();

  const paletteLen = payload.palette.length;
  const clampColor = (c: number): number =>
    paletteLen > 0 ? Math.max(0, Math.min(paletteLen - 1, c)) : 0;

  const nodes: NodeView[] = [];
  const nodeNames: Record<number, string> = { ...payload.node_names };
  let nodeIdCounter = 1;
  for (const place of sortedPlaces) {
    // Skip duplicate place ids — first occurrence wins. This keeps
    // link endpoint resolution deterministic.
    if (idByPlace.has(place.id)) continue;
    const id = nodeIdCounter++;
    idByPlace.set(place.id, id);
    posByPlace.set(place.id, place.pos);
    const hint = ROLE_HINTS[place.role] ?? FALLBACK_HINT;
    const shape =
      place.shape !== undefined && isNodeShapeTag(place.shape) ? place.shape : hint.shape;
    const color = clampColor(place.color ?? hint.color);
    colorByPlace.set(place.id, color);
    nodes.push({ id, pos: place.pos, shape, color });
    // Populate node_names so hover/inspector show the SL1 place id
    // rather than a synthetic "node#N" fallback.
    nodeNames[id] = place.id;
  }

  const links = payload.sl1_links ?? [];
  const sortedLinks = [...links].sort((a, b) => compareIds(a.id, b.id));
  const paths: PathView[] = [];
  const pathNames: Record<number, string> = { ...payload.path_names };
  let pathIdCounter = 1;
  for (const link of sortedLinks) {
    const fromPos = posByPlace.get(link.from);
    const toPos = posByPlace.get(link.to);
    if (fromPos === undefined || toPos === undefined) continue;
    // Color the link with its source place's color so the eye can
    // trace data flow from origin.
    const color = colorByPlace.get(link.from) ?? FALLBACK_HINT.color;
    const id = pathIdCounter++;
    paths.push({
      id,
      from_pos: fromPos,
      to_pos: toPos,
      color: clampColor(color),
    });
    pathNames[id] = link.id;
  }

  return { ...payload, nodes, paths, node_names: nodeNames, path_names: pathNames };
}

function isNodeShapeTag(s: string): s is NodeShapeTag {
  return s === "circle" || s === "square" || s === "triangle" || s === "diamond" || s === "hexagon";
}
