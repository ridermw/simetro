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

/** Synthesize NodeView/PathView from sl1_places/sl1_links when the
 *  legacy node/path arrays are empty. Returns the original payload
 *  unchanged if no synthesis is needed (non-SL1 scenes, SL1 scenes
 *  that already have legacy geometry, or SL1 scenes with no places). */
export function synthesizeSl1Geometry(payload: StaticPayload): StaticPayload {
  if (payload.nodes.length > 0) return payload;
  const places = payload.sl1_places ?? [];
  if (places.length === 0) return payload;

  // Map place id → synthetic numeric node id for link endpoint lookup.
  // Order is deterministic (sorted by place id) so two renders of the
  // same scene produce identical ids.
  const sortedPlaces = [...places].sort((a, b) => a.id.localeCompare(b.id));
  const idByPlace = new Map<string, number>();
  const posByPlace = new Map<string, [number, number]>();
  const colorByPlace = new Map<string, number>();

  const paletteLen = payload.palette.length;
  const clampColor = (c: number): number =>
    paletteLen > 0 ? Math.max(0, Math.min(paletteLen - 1, c)) : 0;

  const nodes: NodeView[] = sortedPlaces.map((place, idx) => {
    const id = idx + 1;
    idByPlace.set(place.id, id);
    posByPlace.set(place.id, place.pos);
    const hint = ROLE_HINTS[place.role] ?? FALLBACK_HINT;
    const shape =
      place.shape !== undefined && isNodeShapeTag(place.shape) ? place.shape : hint.shape;
    const color = clampColor(place.color ?? hint.color);
    colorByPlace.set(place.id, color);
    return { id, pos: place.pos, shape, color };
  });

  const links = payload.sl1_links ?? [];
  const sortedLinks = [...links].sort((a, b) => a.id.localeCompare(b.id));
  const paths: PathView[] = [];
  let pathIdCounter = 1;
  for (const link of sortedLinks) {
    const fromPos = posByPlace.get(link.from);
    const toPos = posByPlace.get(link.to);
    if (fromPos === undefined || toPos === undefined) continue;
    // Color the link with its source place's color so the eye can
    // trace data flow from origin.
    const color = colorByPlace.get(link.from) ?? FALLBACK_HINT.color;
    paths.push({
      id: pathIdCounter++,
      from_pos: fromPos,
      to_pos: toPos,
      color: clampColor(color),
    });
  }

  return { ...payload, nodes, paths };
}

function isNodeShapeTag(s: string): s is NodeShapeTag {
  return s === "circle" || s === "square" || s === "triangle" || s === "diamond" || s === "hexagon";
}
