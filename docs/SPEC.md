# inkland: Specification

A Rust rebuild of Azgaar's Fantasy Map Generator
(https://github.com/Azgaar/Fantasy-Map-Generator, MIT): a headless,
deterministic, constraint-driven map engine with a library crate, a thin
agent-friendly CLI, and a Slint desktop app on top of it.

The main use is producing print-ready, reproducible maps for a D&D book.
Chapters cite facts from the maps ("the road crosses the Brann at mile 20"),
so maps must be exact, traceable and checkable by machine.

Priority order: **library + CLI + determinism + fixed inputs + print
rendering first**. The desktop UI comes after the headless pipeline is solid.

---

## 0. Phase 0: research, then a plan (no code yet)

1. Clone the original into `./reference` (read-only, never modified).
2. Confirm the license (MIT) and plan how its notice is kept (see §13).
   Summarize Slint's license options (GPLv3 / royalty-free / commercial)
   and what each means for distributing the desktop app.
3. Write `docs/ARCHITECTURE.md` describing the original:
   - Every generation stage (points/grid, Voronoi, heightmap templates,
     features/coastlines, climate, rivers/lakes, biomes, cultures, burgs,
     states, provinces, routes, religions, markers, name generation): which JS
     files it lives in, its inputs/outputs, and the global state it touches.
   - Every UI editor/tool, grouped as core / nice-to-have / skip.
4. Explain how each stage changes to respect fixed inputs (§7).
5. Propose the workspace layout, config schema draft and milestone plan.
   **Wait for approval before writing Rust.**

Algorithms are re-implemented, not cloned. Matching the original's output for
the same seed is a non-goal.

---

## 1. Workspace

| Crate | Role |
|---|---|
| `inkland-core` | World model + generation stages. Pure: no I/O, no UI, no clock, no env. |
| `inkland-constraints` | Pinned points/regions/rivers/roads; validation; fitting generation around them. |
| `inkland-edit` | Every edit is a serializable, undoable `Command`. Used by the CLI and the app. |
| `inkland-render` | SVG writer (the real output), print-safe mode, label placement, PNG preview. |
| `inkland-io` | Config + world JSON, report, GeoJSON export, JSON Schema, optional Azgaar import. |
| `inkland` (lib facade) | Public embedding API re-exporting the stable surface. |
| `inkland-cli` | Thin binary `inkland`. Parses flags, calls the library, prints JSON. No logic of its own. |
| `inkland-app` | Slint desktop app. Separate binary; the CLI must never depend on it. |

The library API mirrors the CLI: `generate(&Config) -> Result<Output, Error>`,
`validate(&Config)`, `query(&World, Point)`, `export_geojson(&World)`, etc.
Anything the CLI can do, embedding Rust code can do.

---

## 2. CLI contract

### Commands
```
inkland generate --config map.json [--out-dir out/] [--preview out.png] [--variant NAME]
inkland generate --config map.json --dry-run
inkland schema                                   # JSON Schema of the config, to stdout
inkland query  --world out/world.json --at 120.5,88
inkland export --world out/world.json --format geojson [--layers coast,rivers,regions,roads]
inkland edit   --world out/world.json --script edits.json -o out/world.json
inkland check  --svg out/map.svg                 # run print/label checks on an existing SVG
inkland version
```

### Rules
- **No prompts, ever.** Every option is a flag or comes from the config file.
  Flags override config values; the effective config goes into the report.
- **JSON in, JSON out.** Machine output goes to stdout as JSON. Human-readable
  logs go to stderr only, and only with `--verbose`.
- `generate` writes, per variant: `<name>.svg`, `report.json`, `world.json`,
  and optionally a PNG preview.
- `--dry-run` parses the config, validates it against the schema, and checks
  constraint feasibility (pins in conflicting regions, river paths that can't
  go downhill, roads whose target is shorter than the straight-line distance,
  crops outside the world). It generates nothing and outputs a report with
  `"dry_run": true`.
- **Errors are JSON** with stable codes:
  ```json
  {"error": {"code": "PIN_IN_FORCED_SEA", "message": "...",
             "path": "/pins/points/3", "details": {}}}
  ```
  Codes are a documented, versioned enum (`docs/ERRORS.md`); existing codes are
  never renamed or reused.
- **Exit codes:**
  `0` ok · `1` output written but one or more checks failed ·
  `2` invalid config / schema error · `3` constraints infeasible ·
  `4` I/O error · `70` internal bug.
- `inkland schema` is generated from the Rust config types (e.g. `schemars`), so
  it can never drift from the code. Every field has a description and units.

---

## 3. Config model (sketch; finalize in Phase 0)

```json
{
  "version": 1,
  "seed": 42,
  "units": { "name": "mile", "per_cell_hint": 2.0 },
  "frame": {
    "origin": "top-left",
    "x_axis": "east",
    "y_axis": "south",
    "width": 400, "height": 300
  },
  "world": {
    "cells": 20000,
    "template": "continents",
    "climate": { "latitude_range": [30, 55] },
    "cultures": 6, "states": 5
  },
  "pins": {
    "points":  [{ "id": "ostrey", "kind": "city", "at": [120, 88], "name": "Ostrey" }],
    "regions": [{ "id": "greyfens", "kind": "marsh", "polygon": [[...]] },
                { "id": "spine",    "kind": "mountains", "box": [200, 40, 260, 140] },
                { "id": "gulf",     "kind": "sea", "polygon": [[...]] }],
    "rivers":  [{ "id": "brann", "through": [[150,60],[140,90],[135,130]], "mouth": [130,170] }],
    "roads":   [{ "id": "kings-road", "from": "ostrey", "to": "harrowgate",
                  "target_length": 72, "tolerance": 0.05 }]
  },
  "overlays": {
    "then":  { "labels": {...}, "features": [...], "roads": [...] },
    "today": { "labels": {...}, "features": [...], "remove": ["ruin:ostrey"] }
  },
  "variants": [
    { "name": "ostrey-then", "overlay": "then",  "page": { "w_mm": 167, "h_mm": 250 } },
    { "name": "land-today",  "overlay": "today", "page": { "w_mm": 167, "h_mm": 250 } },
    { "name": "act-1", "overlay": "today", "crop": [80, 40, 220, 160],
      "page": { "w_mm": 167, "h_mm": 120 }, "extras": { "scale_bar": true } }
  ],
  "render": {
    "mode": "print-safe",
    "fonts": { "title": "IM Fell English", "labels": "EB Garamond", "small": "EB Garamond" },
    "min_line_width_pt": 0.5,
    "extras": { "scale_bar": true, "compass": true, "cartouche": { "title": "The Land" }, "grid": false },
    "layers": ["coast", "relief", "rivers", "regions", "roads", "places", "labels"]
  }
}
```

- All coordinates in the config are in `units`, in the declared `frame`.
  Internally the world may use any representation; conversion happens only at
  the boundaries, and the report always speaks config units.
- One base world + overlays: variants differ **only** in what their overlay
  adds, removes or renames. Terrain, rivers and coastlines are shared and
  byte-identical across variants.

---

## 4. Determinism

**The same seed + config + tool version gives byte-identical output files on
any machine/OS.**

- **Seed tree:** derive an independent RNG per layer from
  `hash(master_seed, layer_name)` (e.g. ChaCha8 seeded via a stable hash).
  Layers: `terrain`, `climate`, `rivers`, `biomes`, `cultures`, `names`,
  `places`, `states`, `roads`, `labels`, `decoration`. Changing a label or
  overlay must never change the coastline. Test this explicitly.
- **No unordered iteration in any output path:** no `HashMap`/`HashSet`
  iteration affecting results. Use `BTreeMap`, `IndexMap`, or sorted vectors.
- **Floating point:**
  - Use the `libm` crate for transcendentals (`sin`, `cos`, `exp`, `pow`,
    `atan2`), not platform libm, which differs between OSes.
  - Fixed evaluation order; no parallel reductions in output paths (if `rayon`
    is used, results must be combined in index order).
  - No `mul_add`/FMA unless it's used everywhere on purpose.
  - Quantize coordinates when writing SVG/JSON/GeoJSON (fixed decimals, e.g. 3),
    so tiny float noise never reaches the files.
- No timestamps, absolute paths, hostnames or env-derived values in outputs.
- CI checks golden hashes of SVG + world.json + report.json on Ubuntu and
  macOS (and Windows if cheap) for a set of fixture configs.

---

## 5. Generation stages

Same idea as the original (grid → Voronoi → heightmap → features → climate →
rivers/lakes → biomes → cultures → names (Markov chains) → places → states →
provinces → roads → religions), but every stage:
- is a pure function `fn(&World, &Config, &mut Rng) -> StageOutput`,
- reads pins from `inkland-constraints` and treats them as hard constraints,
- can be re-run from that stage onward (`regen` / overlay changes).

---

## 6. (reserved: keep section numbers stable for references)

---

## 7. Fixed inputs (the most important feature)

Pins are **hard constraints**. Generation fits around them and never moves
them. If they can't be satisfied, fail with exit `3` and a specific error code.
Never silently relax a pin.

- **Pinned points** (city, town, ruin, fortress, custom kinds): placed at exact
  coordinates. Terrain is shaped so they make sense (land, not sea; ports touch
  the coast if `"port": true`). Automatic places must keep a configurable
  minimum distance from pins.
- **Pinned regions** (`mountains`, `hills`, `marsh`, `forest`, `desert`,
  `sea`, `lake`, `plains`; polygon or box): the heightmap and biome stages are
  biased or forced inside them. `sea` forces height below sea level; `mountains`
  forces elevation and relief. Soft edges blend over a configurable width.
  Conflicts (a pinned city inside a forced sea) are validation errors.
- **Pinned rivers:** the river passes through the given points in order and
  reaches the given mouth (sea or lake). Terrain is carved so height strictly
  decreases along the path, so it flows downhill. The rest of the drainage
  system is generated consistently around it (tributaries can join it; no other
  river crosses it).
- **Pinned roads:** routed between pinned points over a terrain cost field
  (slope, rivers → bridges/fords, marsh). `target_length` with `tolerance`:
  the router aims for the target (by choosing routes and meander); the report
  gives the real length, and the check fails if it's outside tolerance.
  Optional `via` waypoints.
- **Units and frame** come from the config (§3). The report gives everything in
  those units.

---

## 8. Variants and crops

- A variant = base world + one overlay + render settings + optional crop.
- Overlays can: add/remove/rename places and labels, add features (ruins,
  battle sites, borders of a given era), add/hide roads, and change styling.
  Overlays cannot change terrain, rivers or coastlines.
- **Crops** (`act-1`, `act-2`): a rectangle in world units, rendered to their
  own page size with their own scale bar computed for that crop's scale.
  Labels are re-placed for the crop. Features cut by the crop edge are clipped
  cleanly.
- `--variant NAME` renders just one; by default all are rendered.

---

## 9. Rendering

- **SVG is the primary output.** All text is real `<text>` (never converted to
  outlines). Font families come from the config by name.
- **Page size:** `width="167mm" height="250mm"` with a matching `viewBox`.
  Line widths and font sizes are specified in pt/mm in the config and converted
  exactly.
- **Print-safe mode:**
  - black on white only: no greys from opacity; shading done with hatching
    or stippling patterns (deterministic, seeded from `decoration`),
  - no `opacity`, `fill-opacity`, `stroke-opacity`, `rgba`, `hsla`, `<mask>`,
    `<filter>`, blend modes, or gradients,
  - every stroke ≥ `min_line_width_pt` (default 0.5 pt).
  A colour mode can exist too, but print-safe is the reference.
- **Label placement:** collision-free (labels vs labels, vs place icons, vs
  rivers where possible). Point labels try candidate positions; area labels
  follow region shape; river labels follow the curve. Anything that can't be
  placed is left out and listed in the report, never overlapped.
- **Extras (optional):** scale bar (in config units, correct for each
  crop), compass, title cartouche, grid (with labels in config units).
- **Stable group ids:** `<g id="coast">`, `relief`, `rivers`, `regions`,
  `roads`, `places`, `labels`, `scale-bar`, `compass`, `cartouche`, `grid`.
  Elements get stable ids too (`place-ostrey`, `river-brann`, `road-kings-road`)
  derived from config ids, so tests and other tools can find them.
- **Element order is deterministic** (sorted by layer, then id).

---

## 10. Report (`report.json`)

```json
{
  "tool": { "name": "inkland", "version": "0.1.0" },
  "config_hash": "sha256:…", "seed": 42, "variant": "land-today",
  "units": "mile", "dry_run": false,
  "pins": {
    "points": [{ "id": "ostrey", "requested": [120, 88], "placed": [120, 88], "error": 0.0 }],
    "rivers": [{ "id": "brann", "through_ok": true, "max_deviation": 0.0, "downhill": true, "length": 61.3 }],
    "roads":  [{ "id": "kings-road", "target": 72, "actual": 71.4, "within_tolerance": true,
                 "crossings": [{ "river": "brann", "at_length": 20.1, "kind": "bridge" }] }]
  },
  "labels": { "placed": 143, "unplaced": [{ "id": "label-hollow-mere", "reason": "collision" }] },
  "checks": [
    { "name": "no_opacity", "pass": true },
    { "name": "min_line_width", "pass": true, "min_found_pt": 0.5 },
    { "name": "all_labels_placed", "pass": false, "count": 1 },
    { "name": "all_text_is_text_elements", "pass": true },
    { "name": "no_masks_filters_rgba", "pass": true },
    { "name": "deterministic_ids", "pass": true }
  ],
  "warnings": [{ "code": "FONT_NOT_FOUND", "message": "…", "font": "IM Fell English" }]
}
```
- Checks run on the **written SVG** (parse it back), not on internal state,
  so they test what will actually be printed. `inkland check` exposes the same
  checks for any SVG.
- Any failed check makes the exit code `1` (output still written).

---

## 11. Inspection (for agents)

- `inkland query --world world.json --at x,y` → JSON: height, biome, land/sea,
  region pins containing the point, nearest river (id + distance + which side),
  nearest place, nearest road, state/province/culture.
- `inkland export --format geojson` → FeatureCollection of coastlines, rivers,
  regions, roads, places, borders, in config units and frame (declare a local
  CRS in the output; this is not WGS84). Properties carry ids, names,
  lengths and kinds, so chapters can quote real facts.
- `--preview out.png`: a low-res raster (e.g. 1024 px longest side) rendered
  from the SVG with `resvg`, so an agent can look at the map without a browser.

---

## 12. Desktop app (Slint), after the headless milestones

- Built purely on the library crates. Every edit goes through `inkland-edit`
  commands, so undo/redo and "save edits as script" come for free, and app
  edits can be replayed by the CLI.
- **Map view:** render with tiny-skia into a `SharedPixelBuffer` shown as a
  Slint `Image`; cache layers; rough preview during pan/zoom and a sharp render
  when idle. Hit-testing in Rust via a spatial index (screen → world →
  cell/feature). Target 60 fps at 50k cells.
- Generation and heavy renders run on worker threads and post results back with
  `slint::invoke_from_event_loop`, with per-stage progress.
- Layout: map in the center; layers + tools on the left; a context editor on
  the right; menu bar; status bar (cursor coords in config units, height,
  biome, region).
- Editors (core first): config/generation options, **pin editor** (place and
  drag pinned points, draw pinned regions, draw pinned river paths, define
  roads with targets: this edits the config, not just the world), layers,
  places, states, cultures, religions, heightmap brush, labels, overlays/variants
  switcher, crop-region tool, print preview (print-safe rendering + check
  results).
- `.slint` files modular (one component per editor); logic in Rust view-models.

---

## 13. Packaging and licensing

- `inkland` CLI: **one static binary with no runtime dependencies**
  (`x86_64-unknown-linux-musl` for Linux; works in GitHub Actions
  `ubuntu-latest`). No Slint/GUI dependencies in the CLI's dependency tree.
  Enforce this in CI (`cargo tree -p inkland-cli` must not contain `slint`).
- Fonts are referenced by family name in the SVG and **not embedded**,
  unless `render.embed_fonts: true` is set (with paths given in the config).
  The PNG preview resolves fonts via system fonts plus an optional `font_dirs`
  config list. A missing font produces a `FONT_NOT_FOUND` warning, not an error.
- Keep Azgaar's MIT license notice: `LICENSE` for this project, plus
  `THIRD_PARTY_NOTICES.md` containing the original copyright + MIT text,
  and attribution in `README.md` and `inkland version --json`.
- Release workflow builds Linux (musl), macOS and Windows CLI binaries; the
  desktop app is packaged separately.

---

## 14. Testing

- Unit tests per stage; `proptest` properties:
  - rivers always flow downhill to sea/lake; pinned rivers pass through all
    their points in order,
  - pinned points end up exactly where they were requested (error == 0),
  - every land cell belongs to exactly one state; places sit on land,
  - forced-sea regions are sea and forced-mountain regions are high,
  - changing only an overlay/labels leaves terrain/river output byte-identical,
  - every `Command`'s undo restores the exact previous world.
- Golden-file tests: fixture configs → SVG + world.json + report.json hashes,
  checked on several OSes in CI.
- SVG checks tested against hand-made "bad" SVGs (opacity, thin lines, outlined
  text) to prove each check can actually fail.
- CLI contract tests: JSON on stdout, error codes, exit codes, `--dry-run`
  has no side effects, `schema` validates every fixture config.
- Benchmarks (release): 20k-cell world + one variant in under 2 s.
- `cargo clippy -D warnings`, `cargo fmt --check`, `cargo deny` in CI.

---

## 15. Milestones (one at a time; stop for review after each)

1. Workspace, config types + `inkland schema`, JSON error/exit-code framework,
   `--dry-run` validation, seed tree, CI with static musl build.
2. Grid + Voronoi + heightmap templates + coastlines; minimal SVG writer
   with stable ids and page sizing; PNG preview; golden-hash CI.
3. Pinned regions + pinned points in terrain generation; report pin section.
4. Climate, biomes, rivers/lakes, then **pinned rivers** (carving, downhill).
5. Cultures, Markov names, places, states, provinces (all around pins).
6. Roads: cost field routing, pinned roads with target lengths, crossings.
7. Print-safe renderer: hatching/stippling, line-width rules, fonts, extras
   (scale bar, compass, cartouche, grid); SVG checks + `inkland check`.
8. Label placement with collision report.
9. Overlays, variants and crops (own scale bars, re-placed labels).
10. `query`, GeoJSON export, `edit` + command system + scripts.
11. Slint app: map view (pan/zoom/hit-test) + layers + status bar.
12. Slint editors: pin editor, places/states/cultures, brush, labels,
    variant/crop tools, print preview with checks; undo/redo.
13. Optional: Azgaar `.map` / JSON import.
14. Polish: performance, docs (`docs/CLI.md`, `docs/CONFIG.md`,
    `docs/ERRORS.md`), release packaging.
