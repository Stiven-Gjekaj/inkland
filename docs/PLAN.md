# Phase 0 plan

This file is the result of Phase 0 (see `docs/SPEC.md` §0).
It holds the license plan, the workspace layout, the config draft, and the
milestone plan.
`docs/ARCHITECTURE.md` describes the original program.
`docs/ARCHITECTURE.md` §4 tells how each stage changes for fixed inputs.

No Rust code exists yet.
The human approves this plan before Milestone 1 starts.

## 1. Licenses

### 1.1 The original program

- Source: <https://github.com/Azgaar/Fantasy-Map-Generator>.
- Commit that we read: `74136292b8012abe0d57a11ac7780ddc43d9433d`
  (2026-11-16).
- License: MIT, "Copyright 2017-2024 Max Haniyeu (Azgaar)".
  The file adds one paragraph to the usual MIT text.
  The paragraph permits derivative works and commercial use, and it names
  maps and map images as derivative works.
- The program is now TypeScript under `src/`. It is not the old `modules/*.js`
  layout. The spec text that says "JS files" means these TypeScript files.

### 1.2 How inkland keeps the notice

- inkland re-implements the algorithms. It does not translate the code line
  by line.
- inkland copies some data tables: name bases, heightmap template steps,
  biome tables, and possibly marker kinds. These tables are a copy of
  protected work. The MIT notice must come with them.
- `THIRD_PARTY_NOTICES.md` holds the full original `LICENSE` text without
  changes, and a list of the data tables that came from the original.
- Each Rust file that holds a copied table has a one-line comment that names
  the source file in the original.
- `README.md` has an "Attribution" section.
- `inkland version` prints a `third_party` array in its JSON. The array holds
  the name, the URL, the license, and the copyright line of the original.
- The license of inkland itself is `MIT OR Apache-2.0`.
  The repository holds `LICENSE-MIT` and `LICENSE-APACHE`.
- `./reference` is the clone of the original. Git ignores it. inkland never
  changes it.

### 1.3 Slint

Slint has three license options. A project selects one.

| Option | What it permits | What it requires |
|---|---|---|
| Royalty-free 2.0 | Desktop, mobile, and web apps, also closed source. No fee. | Show the `AboutSlint` widget in an About dialog that the top menu opens, or show the Slint badge on the public download page. No embedded use. Do not expose the Slint API from the app. |
| GPLv3 (GPL-3.0-only) | All platforms, also embedded. | The distributed app binary is GPLv3. Our own source files can stay MIT or Apache-2.0. |
| Commercial | Closed-source apps, also embedded, with a fee per device. | A paid plan. |

Decision (see `docs/DECISIONS.md`): royalty-free.

- `inkland-app` shows `AboutSlint` in its Help > About dialog.
- The source of `inkland-app` stays `MIT OR Apache-2.0`.
- Only `inkland-app` depends on Slint. The CLI and the library crates do not.
- `deny.toml` permits the Slint license expression only for the Slint crates.
- The release of the desktop app goes in a separate package, with a
  `NOTICE` that names the Slint license.

## 2. Workspace layout

```
inkland/
  Cargo.toml                 workspace, shared lints, shared profile
  rust-toolchain.toml        pinned stable toolchain
  deny.toml                  cargo-deny: licenses, bans, sources
  LICENSE-MIT  LICENSE-APACHE  THIRD_PARTY_NOTICES.md  README.md
  crates/
    inkland-core/            world model, seed tree, stages. No I/O.
    inkland-constraints/     pin types, validation, feasibility, fitting
    inkland-edit/            Command trait, undo stack, edit scripts
    inkland-render/          SVG writer, print-safe styles, labels, checks
    inkland-io/              config load, world JSON, report, GeoJSON, schema
    inkland/                 library facade: generate, validate, query, ...
    inkland-cli/             binary "inkland". Flags to library calls only.
    inkland-app/             Slint desktop app (Milestone 11 and later)
  fixtures/
    configs/                 fixture configs for golden tests
    svg-bad/                 hand-made SVG files that each check must refuse
  tests/
    golden/                  expected sha256 per fixture and per output file
  docs/
    SPEC.md  PLAN.md  ARCHITECTURE.md  DECISIONS.md
    CLI.md  CONFIG.md  ERRORS.md   (Milestone 14, ERRORS.md starts in M1)
  .github/workflows/
    ci.yml                   fmt, clippy, test, deny, golden on 3 OSes
    release.yml              CLI binaries (Linux musl, macOS, Windows)
```

### 2.1 Dependency direction

```
inkland-cli ─┐
inkland-app ─┼─> inkland (facade) ─> inkland-io ─> inkland-render ─> inkland-core
             │                    └> inkland-edit ─────────────────> inkland-core
             └──────────────────────> inkland-constraints ─────────> inkland-core
```

- `inkland-core` depends on no other workspace crate.
- `inkland-core` and `inkland-constraints` have no `std::fs`, no `std::env`,
  and no clock. A CI step searches for these names in the two crates.
- `inkland-render` makes SVG text. Only `inkland-io` writes files.
- The PNG preview (`resvg`) is a feature of `inkland-render`. The CLI turns
  the feature on. Library users can turn it off.

### 2.2 Main crates that we expect to use

| Need | Crate |
|---|---|
| Config types and schema | `serde`, `serde_json`, `schemars` |
| CLI flags | `clap` (derive) |
| RNG | `rand_chacha` (ChaCha8), `rand_core` |
| Stable hash for the seed tree and `config_hash` | `sha2` |
| Math | `libm` |
| Delaunay / Voronoi | `delaunator` (check that output order is stable), else our own |
| Ordered maps | `indexmap`, `std::collections::BTreeMap` |
| Polygon clip and offset | `geo` / `i_overlay` (check determinism first) |
| SVG parse for checks | `roxmltree` |
| PNG preview | `resvg`, `usvg`, `tiny-skia`, `fontdb` |
| Property tests | `proptest` |
| Benchmarks | `criterion` |
| Desktop | `slint` (only in `inkland-app`) |

Each crate goes in with a short check of its license and of its determinism.
The check goes in the commit message of the commit that adds the crate.

## 3. Config draft (version 1)

This draft makes the sketch in `SPEC.md` §3 exact.
`inkland schema` makes the JSON Schema from the Rust types.
Every field has a description and a unit.

### 3.1 Identifiers

- An `id` matches `^[a-z0-9]+(-[a-z0-9]+)*$`.
- An `id` is unique across all pins of all kinds.
- An overlay refers to an item as `"<kind>:<id>"`, for example
  `"place:ostrey"`, `"road:kings-road"`, `"feature:old-wall"`.
- SVG element ids are `<kind>-<id>`, for example `place-ostrey`.

### 3.2 Top level

| Field | Type | Default | Notes |
|---|---|---|---|
| `version` | integer, must be `1` | none | Required. |
| `seed` | u64 | none | Required. |
| `units` | object | `{name: "mile"}` | `name` is free text. `per_cell_hint` is the wanted mean cell width in units. |
| `frame` | object | see 3.3 | Size and axes of the world. |
| `world` | object | see 3.4 | Generation options. |
| `pins` | object | empty | See 3.5. |
| `overlays` | map of name to overlay | empty | See 3.6. A `BTreeMap`, so the order is stable. |
| `variants` | array | one variant `"default"` | See 3.7. |
| `render` | object | see 3.8 | Shared render options. A variant can change some of them. |
| `font_dirs` | array of paths | empty | Only for the PNG preview. |

### 3.3 `frame`

| Field | Values | Default |
|---|---|---|
| `origin` | `top-left`, `bottom-left` | `top-left` |
| `x_axis` | `east` | `east` |
| `y_axis` | `south` (with `top-left`), `north` (with `bottom-left`) | `south` |
| `width`, `height` | number > 0, in units | none, required |

Other combinations are a schema error (`FRAME_AXES_INVALID`).

### 3.4 `world`

| Field | Type | Default |
|---|---|---|
| `cells` | integer, 1000 to 100000 | 10000 |
| `template` | template name, or `{steps: [...]}` | `continents` |
| `sea_level` | integer, 0 to 100 (height scale of the original) | 20 |
| `climate.latitude_range` | `[south, north]` in degrees | `[30, 55]` |
| `climate.wind` | array of 6 bearings, one for each latitude band | from the original |
| `cultures` | integer | 6 |
| `culture_set` | name | `european` |
| `states` | integer | 5 |
| `towns` | integer or `"auto"` | `"auto"` |
| `religions` | integer | 4 |
| `places.min_distance_from_pins` | number, in units | 3 * mean cell width |

### 3.5 `pins`

All coordinates are in units, in the frame of 3.3.

`pins.points[]`

| Field | Type | Notes |
|---|---|---|
| `id` | id | Required. |
| `kind` | `city`, `town`, `village`, `ruin`, `fortress`, `custom` | Required. |
| `at` | `[x, y]` | Required. The place is put here exactly. |
| `name` | string | Default: a generated name. |
| `port` | bool | Default `false`. `true` means the cell touches the sea. |
| `capital_of` | state name | Optional. The state forms around this place. |
| `custom_kind` | string | Required when `kind` is `custom`. |

`pins.regions[]`

| Field | Type | Notes |
|---|---|---|
| `id` | id | Required. |
| `kind` | `sea`, `lake`, `mountains`, `hills`, `plains`, `marsh`, `forest`, `desert` | Required. |
| shape | one of `polygon: [[x, y], ...]`, `box: [x0, y0, x1, y1]`, `circle: {at, r}` | Required. A polygon has 3 or more points and does not cross itself. |
| `edge` | number, in units | Default 0. Width of the soft edge, inside the shape. |
| `height` | `[min, max]` on the 0 to 100 height scale (sea level is 20) | Optional. Changes the default range of the kind. Example: the Wolds are `hills` with `[35, 50]`. |
| `landlocked` | bool | Only for `sea` and `lake`. Default `false`. With `false`, a `sea` must connect to the ocean (a frame edge), else `SEA_NOT_CONNECTED`. With `true`, an inland sea or a large lake is correct. |
| `name` | string | Optional. A region label. |

Default height ranges: `sea` and `lake` 5 to 15, `marsh` 20 to 23,
`plains` 21 to 35, `hills` 35 to 60, `mountains` 70 to 100.
`forest` and `desert` only make sure of land.

`pins.rivers[]`

A river has no height field. The tool calculates the heights along the
course from the region heights, so that the river flows downhill.

| Field | Type | Notes |
|---|---|---|
| `id` | id | Required. |
| `name` | string | Optional. |
| `source` | `[x, y]`, or `{place: id}`, or `{region: id}` | Required. With a region, the tool chooses the source cell inside the region: the highest cell from which the course can flow downhill. |
| `through` | array of `[x, y]` or `{place: id}` | Optional, in order. The course passes each one exactly. |
| `mouth` | `[x, y]`, or `{place: id}`, or `{region: id}` | Required. A point must be in sea or lake. A place must be a port; the river reaches the water at the cell of the place. A region of kind `marsh`, `lake` or `sea` is the end of the river; a river that ends in a `marsh` has no sea mouth. |

Two pinned rivers can share a mouth or join another pinned river. They then
meet in a confluence. Two pinned rivers never cross.
Example: the Corrin and the Tey both have `mouth: {place: "corrantey"}`.
They join before the sea, at the last land cell before Corrantey or earlier.

`pins.roads[]`

| Field | Type | Notes |
|---|---|---|
| `id` | id | Required. |
| `from`, `to` | point pin id | Required. |
| `via` | array of point pin ids or `[x, y]` | Optional, in order. |
| `kind` | `road`, `trail` | Default `road`. |
| `target_length` | number, in units | Optional. |
| `tolerance` | fraction, 0 to 1 | Default `0.05`. |

### 3.6 `overlays.<name>`

| Field | Type | Notes |
|---|---|---|
| `add.places[]` | like `pins.points[]` | Places that only this overlay shows. They do not change terrain. |
| `add.features[]` | `{id, kind, at or polyline or polygon, name?}` | Ruins, battle sites, old borders. |
| `add.roads[]` | like `pins.roads[]` | Routed on the shared terrain. |
| `remove[]` | `"<kind>:<id>"` | Hide an item. |
| `rename` | map of `"<kind>:<id>"` to name | Change a label. |
| `labels` | map of `"<kind>:<id>"` to label options | Size, style, `hide`. |
| `style` | partial `render.style` | Style changes for this overlay. |

An overlay never changes heights, rivers, or coasts.
The type system enforces this: an overlay has no field that can do it.

### 3.7 `variants[]`

| Field | Type | Notes |
|---|---|---|
| `name` | id | Required. It is also the output file stem. |
| `overlay` | overlay name | Optional. |
| `crop` | `[x0, y0, x1, y1]` in units | Optional. Must be inside the frame. |
| `page` | `{w_mm, h_mm, margin_mm}` | Required. |
| `extras` | partial `render.extras` | Changes for this variant. |

### 3.8 `render`

| Field | Type | Default |
|---|---|---|
| `mode` | `print-safe`, `colour` | `print-safe` |
| `fonts` | `{title, labels, labels_sc, other}` family names | `title`, `labels`: `IM Fell English`. `labels_sc` (small capitals, for region names): `IM Fell English SC`. `other` (scale bar, compass, grid, notes): `Caladea`. |
| `fonts_allowed` | array of family names | `["Caladea", "Inter", "IM Fell English", "IM Fell English SC"]` |
| `font_sizes_pt` | `{title, region, place_city, place_town, river, other}` | to decide in M7 |
| `min_line_width_pt` | number | `0.5` |
| `extras` | `{scale_bar, compass, cartouche: {title}, grid: {step}}` | all off |
| `layers` | ordered array of layer names | all layers |

Fonts:

- The SVG names each family in a `font-family` attribute, exactly as the
  config gives it, for example `font-family="IM Fell English"`.
- inkland never embeds a font and never changes text to outlines. The spec
  option `render.embed_fonts` is removed.
- The `font_families_allowed` check parses the written SVG and fails if a
  `font-family` is not in `fonts_allowed`. A config that names a family that
  is not in `fonts_allowed` is a schema error (`FONT_NOT_ALLOWED`, exit 2).
- The PNG preview uses `font_dirs` and the system fonts. A missing font gives
  the warning `FONT_NOT_FOUND`.

### 3.9 Flags that change the config

The CLI applies a flag after it reads the file.
The report holds the effective config.

| Flag | Field |
|---|---|
| `--seed N` | `seed` |
| `--cells N` | `world.cells` |
| `--variant NAME` | renders only that variant |
| `--set PATH=JSON` | any field, by JSON Pointer; repeatable |

## 4. Error codes (first set)

`docs/ERRORS.md` starts in Milestone 1 with this list.
A code is never renamed and never reused.

| Code | Exit | When |
|---|---|---|
| `CONFIG_PARSE` | 2 | The file is not valid JSON. |
| `CONFIG_SCHEMA` | 2 | The JSON does not agree with the schema. |
| `CONFIG_VERSION` | 2 | `version` is not supported. |
| `FRAME_AXES_INVALID` | 2 | The frame axes do not agree with the origin. |
| `ID_INVALID` | 2 | An id does not match the pattern. |
| `ID_DUPLICATE` | 2 | Two items have the same id. |
| `REF_UNKNOWN` | 2 | A reference names an id or overlay that does not exist. |
| `PIN_OUTSIDE_FRAME` | 3 | A pin or a crop is outside the frame. |
| `PIN_IN_FORCED_SEA` | 3 | A point pin is in a `sea` or `lake` region. |
| `PORT_NOT_NEAR_COAST` | 3 | A port pin has no sea cell that it can touch. |
| `REGION_CONFLICT` | 3 | Two forced regions overlap with kinds that cannot agree. |
| `REGION_POLYGON_INVALID` | 2 | A polygon has fewer than 3 points or crosses itself. |
| `FONT_NOT_ALLOWED` | 2 | A font family in the config is not in `render.fonts_allowed`. |
| `SEA_NOT_CONNECTED` | 3 | A `sea` region without `landlocked: true` does not connect to the ocean. |
| `RIVER_MOUTH_ON_LAND` | 3 | A point mouth is not in sea or lake, or a place mouth is not a port. |
| `RIVER_MOUTH_REGION_KIND` | 2 | A region mouth is not of kind `marsh`, `lake` or `sea`. |
| `RIVER_NOT_DOWNHILL` | 3 | The region heights on the course make downhill flow impossible, for example a course that goes from the Low Fen up into the Grey Teeth. |
| `RIVER_CROSSES_RIVER` | 3 | Two pinned rivers cross. A join (confluence) is not a cross. |
| `ROAD_TARGET_TOO_SHORT` | 3 | The target is shorter than the straight line through all via points. |
| `ROAD_NO_PATH` | 3 | No land path joins the road ends. |
| `ROAD_LENGTH_OUT_OF_TOLERANCE` | 1 | The route was made, but its length is outside the tolerance (a check). |
| `IO_READ` | 4 | A file cannot be read. |
| `IO_WRITE` | 4 | A file cannot be written. |
| `INTERNAL` | 70 | A bug. |

Warnings have codes too: `FONT_NOT_FOUND`, `LABEL_UNPLACED`.

## 5. Determinism plan

- `inkland-core::seed::SeedTree` makes one `ChaCha8Rng` per layer.
  The seed of a layer is the first 32 bytes of
  `sha256("inkland/v1/" + master_seed as decimal + "/" + layer_name)`.
- The `v1` part changes only when we accept a break of all golden files.
- Layers are the list in `SPEC.md` §4.
- A stage gets only the RNG of its own layer. The type system enforces this:
  `SeedTree::layer(Layer::Terrain)` returns a `LayerRng<Terrain>`.
- Floats: `libm` for all transcendental functions. A clippy lint config bans
  `f64::sin` and its relatives (`disallowed-methods` in `clippy.toml`).
- The world model uses `Vec` with integer ids. Iteration order is the id order.
- `HashMap` and `HashSet` are in `disallowed-types` in the output crates.
- Writers round all coordinates to 3 decimals, with a fixed formatter
  (no `{}` on `f64`, because its output can change between Rust versions).
- Golden hashes run on `ubuntu-latest`, `macos-latest`, and `windows-latest`.

## 6. Milestone plan

The milestones are those of `SPEC.md` §15. This section adds the deliverables
and the test that closes each milestone. We stop for a review after each one.

| # | Deliverables | Closing test |
|---|---|---|
| 1 | Workspace, all crates as empty shells. Config types, `inkland schema`, JSON errors and exit codes, `generate --dry-run` with schema and simple feasibility checks (frame, ids, refs, pin in forced sea, road target vs straight line). Seed tree. CI: fmt, clippy, test, deny, musl static build, `cargo tree` check for Slint. `docs/ERRORS.md`. | Every fixture validates against `inkland schema`. Contract tests for exit codes 0, 2, 3. A dry run writes no files. |
| 2 | Jittered grid, Voronoi, heightmap template engine with all named templates, features (ocean, lake, island markup), coastline polygons. Minimal SVG writer: page size, `viewBox`, `coast` group. PNG preview. Golden hashes. | Same config gives the same sha256 on 3 OSes. |
| 3 | Pinned regions and pinned points in the heightmap. Report pin section. | Proptest: a forced-sea region is all sea, a forced-mountain region is all high, each pinned point is land with error 0. |
| 4 | Temperature, precipitation, biomes, rivers and lakes. Pinned rivers with carving. | Proptest: each river flows downhill to sea or lake. Each pinned river passes all its points in order. |
| 5 | Cultures, Markov names, places (burgs), states, provinces, religions. All of them respect pins. | Proptest: each land cell is in exactly one state. Places are on land. Auto places keep the minimum distance. |
| 6 | Cost field, A* routes, pinned roads with target length, crossings (bridge or ford). | Report lengths agree with GeoJSON lengths. A test road hits its target within tolerance. |
| 7 | Print-safe renderer: hatching and stippling, line rules, fonts, scale bar, compass, cartouche, grid. SVG checks and `inkland check`. | Each bad SVG in `fixtures/svg-bad/` fails its check. Each golden SVG passes all checks. |
| 8 | Label placement for points, areas, and rivers, with a collision report. | Proptest: no two placed label boxes overlap. |
| 9 | Overlays, variants, crops. | A change to an overlay leaves the terrain bytes the same (test with two configs). Crop scale bars agree with the crop scale. |
| 10 | `query`, GeoJSON export, `edit` with the command system and scripts. | Undo of each command gives the same world bytes. |
| 11 | Slint app: map view, pan and zoom, hit test, layers, status bar. | 60 fps at 50k cells (manual measure, recorded). |
| 12 | Slint editors: pins, places, states, cultures, brush, labels, variants, crop, print preview, undo and redo. | App edits saved as a script give the same world when the CLI replays them. |
| 13 | Optional import of an Azgaar `.map` file. | A sample `.map` loads, and its land and sea agree. |
| 14 | Speed work, `docs/CLI.md`, `docs/CONFIG.md`, release packages. | 20k cells and one variant in less than 2 s (release build). |

## 7. Road lengths in the report

The book checks travel days against road lengths.
Each road in the report has a `stops` list: each pinned place on the road
and its length along the road from `from`.
The report also has a `legs` list: the length between each two stops that
follow each other.

```json
{ "id": "kings-road", "target": 72, "actual": 71.4, "within_tolerance": true,
  "stops": [{ "place": "ostrey", "at_length": 0.0 },
            { "place": "brannocks-ford", "at_length": 20.1 },
            { "place": "harrowgate", "at_length": 71.4 }],
  "legs":  [{ "from": "ostrey", "to": "brannocks-ford", "length": 20.1 },
            { "from": "brannocks-ford", "to": "harrowgate", "length": 51.3 }],
  "crossings": [{ "river": "brann", "at_length": 20.1, "kind": "bridge" }] }
```

## 8. Answers to the Phase 0 questions

`docs/DECISIONS.md` records the answers. In short:

1. A pinned `sea` must connect to the ocean, unless it has
   `landlocked: true`.
2. A river has no fixed height. Region heights drive the river heights.
   Sources and mouths can be areas or places.
3. Journeys, coats of arms, economy, military and zones are out.
   Coats of arms can come later as a separate milestone.
4. Pages are 167 x 250 mm and 167 x 120 mm. Fonts: IM Fell English,
   IM Fell English SC, Caladea, Inter. No embedding.
