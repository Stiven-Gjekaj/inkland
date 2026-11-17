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
| `edge` | number, in units | Default 0. Width of the soft edge, outside the shape. Inside the shape, heights are always in the range. |
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
| `font_sizes_pt` | `{title, state, region, place_city, place_town, place_village, river, other}` | 12, 10, 8.5, 8, 7, 6, 6.5, 7 |
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

## 6.1 Notes from Milestone 2

- Output files. `SPEC.md` §2 names `report.json` and `world.json` for each
  variant. All variants share one base world, so `generate` writes one
  `world.json` and, for each variant, `<variant>.svg` and
  `<variant>.report.json`. Thus two variants never write the same file.
- Until a milestone applies a feature, `generate` refuses a config that uses
  it with `NOT_IMPLEMENTED` (exit 70) and writes nothing: pinned points and
  regions (M3), pinned rivers (M4), pinned roads (M6), overlays (M9).
  `generate --dry-run` checks all of them now.
- Crops work from Milestone 2, because the page fit uses the crop. The crop
  scale bar comes with the scale bar in Milestone 7.
- Heights are whole numbers from 0 to 100, as in the original. The blob
  tools depend on this: each ring loses up to 1 unit, so a blob stops.
- Each part of a layer gets its own seed (`terrain/grid`,
  `terrain/heightmap`), so a change in the grid code does not move the
  heightmap numbers of a fixed grid.
- The coastline follows the cell edges. Milestone 7 adds smoothing.
- A 20000-cell world with one variant and a PNG preview takes about 0.03 s
  in a release build on an Apple M-series machine.

## 6.2 Notes from Milestone 3

- The soft edge of a region is outside its shape, not inside it. Thus each
  cell inside a region has a height in the range of the region, and the
  checks "a forced sea is water" and "forced mountains are high" hold for the
  full shape. Outside the shape, heights blend to the free terrain over the
  `edge` width.
- Inside a region, the template height is scaled into the range:
  `min + (max - min) * h / 100`. A `forest` or `desert` region without a
  `height` only lifts water to sea level. Where regions overlap, a cell gets
  a height in all of their ranges.
- A point pin is the exact site of its cell, so its report error is 0. The
  cell of a place is land. Its neighbors keep their terrain, so a place
  can be on the coast.
- A port needs one water cell next to it. If the terrain gives none, a
  channel of at most 8 cells joins the port to the nearest water. The
  channel does not cross other places or forced land.
  Else the error is `PORT_NOT_NEAR_COAST`.
- After the features are known, each `sea` region without `landlocked: true`
  must be part of the ocean, else `SEA_NOT_CONNECTED`.
- Known limit: a region box in open sea makes a land block with straight
  coasts, because the soft edge has no noise. Use a polygon outline for now.
  A noisy soft edge is a later change.
- The report gives, for each point pin: requested and placed position,
  error, height, land, and for a port whether it touches water. For each
  region pin: cell count, range, lowest and highest height, and `in_range`.

## 6.3 Notes from Milestone 4

- Climate: temperature from latitude and height, precipitation from winds
  along the lattice rows and columns, as in the original. `wind` is the
  direction to which the wind blows.
- Drainage: a priority flood from the ocean gives each cell a drainage
  elevation (`elevation` in `world.json`). It is the height, or a little
  more where the land is flat or in a hollow. Water flows to the lowest
  neighbor, so it always goes strictly down to the ocean or to a sink.
  "Downhill" in the report and in the tests means this elevation.
- No lakes form in hollows in this version: a hollow fills and drains. The
  lakes are the water that the terrain makes. No erosion.
- Rivers: a land cell with enough water is a river cell. At a confluence the
  river with more water keeps its name. Rivers of fewer than 3 cells are
  dropped, unless pinned.
- Pinned rivers: the course goes from the source through each `through`
  anchor to the mouth, by least cost. Going up costs more, and crossing
  water costs much more: a pinned river is a hard constraint, so a water
  cell on its course becomes land, except in a forced water region. The
  course never goes into a cell whose range starts above the lowest range
  before it, so a downhill course always exists if the path search finds
  one. The heights of the course are carved so that none goes up. The
  drainage then gives the course strictly falling elevations, and the
  course cells always drain along the course: another river can join a
  pinned river but never cross it.
- A course does not use the anchor cells of another river. Rivers with the
  same mouth can join. If an earlier course blocks a later river, the
  planner plans the blocked river first and tries again.
- Mouths: a point in water ends at the shore of that water body, at the
  shore cell nearest the point; the line ends at the point. A port place:
  the river drains into a water cell next to the place. A marsh region: the
  river ends at the first course cell in the marsh, and the water stays
  there. A source region: the highest free land cell in the region.
- The report gives, for each pinned river: source, mouth kind and point,
  `through_ok`, `max_deviation`, `downhill`, length in units, and cells.

## 6.4 Notes from Milestone 5

- Names: a letter chain of order two, learned from a name base. A name is
  refused if it is too short or too long for the base, has three equal
  letters in a row, doubles a letter that the base does not double, or is
  already in use on the map.
- Culture sets: `european` (default), `oriental`, `antique`,
  `high-fantasy`, `dark-fantasy`. `cultures` must not be more than the set
  has.
- Suitability of a land cell: biome habitability, plus rivers, plus coast,
  minus height.
- Cultures and states grow by least cost over the map. Water costs much
  more than land, so each land cell, islands too, gets exactly one state.
  Water cells have no state.
- Places: the pinned places first, exactly where they are, with their names.
  A pinned place without a name gets a name from its culture. Each pin with
  `capital_of` is the capital of a state with that name. More capitals are
  generated until there are `states` states. Towns: `towns`, or about one
  for 150 land cells. Generated places keep `min_distance_from_pins` from
  each pinned place, and a distance from each other that becomes smaller
  only if not enough places fit. A generated place is a port if it touches
  the ocean.
- Provinces: about a third of the places of a state are seats; each grows
  inside its state.
- The SVG draws state borders (dash-dot) and province borders (dots) in the
  `regions` group, and a symbol for each place in the `places` group, with
  the id `place-<pin id>` or `place-<number>`. Names come with labels in
  Milestone 8.
- Religions are not in this version.

## 6.5 Notes from Milestone 6

- A road is a least-cost path of land cells. A step costs its distance,
  more on slopes, in wetland, and into a river (a bridge or a ford). A cell
  that a road uses costs less, so roads share their paths.
- A pinned road goes from its start place, through its `via` points in
  order, to its end place. A later leg does not go back over an earlier leg.
- Target length: if the road is too long with terrain costs, the router
  uses the shortest path by distance. If the road is too short, it gets a
  detour: a waypoint to one side of the middle of its longest leg. A scan of
  the offset on both sides, and halving where the length passes the target,
  finds an offset that gives a length in the tolerance. A road cannot be
  shorter than its shortest path: then the report gives
  `within_tolerance: false`, the check `pinned_road_length` fails, the
  files are written, and the exit code is 1.
- Crossings: a run of road cells on one river, where the road comes from
  one side of the river and leaves on the other side. The kind is `bridge`
  on a road, `ford` on a trail over a small river.
- The generated network joins the capitals of each landmass (shortest tree)
  with roads, and each town to the nearest place on the network with a
  trail.
- The SVG draws pinned roads with the id `road-<id>`, and the network as
  `road-net-<n>` and `trail-net-<n>`.
- Known limits: a region box or polygon in open water makes a block-shaped
  coast. A pinned river course across a lake fills a thin strip of land.

## 6.6 Notes from Milestone 7

- Print-safe output: black and white only. Shading is lines and dots: two
  water lines along each coast, and relief symbols (mountain, hill, tree,
  marsh, dune stipple) placed from the `decoration` seed with a spacing in
  millimetres. Coasts and water lines are smoothed (two rounds of corner
  cutting). Rivers and roads are not smoothed, so that they pass their
  pinned points exactly.
- Layers are drawn in the order of `render.layers`. A layer that is not in
  the list is not drawn.
- Extras: scale bar (a 1, 2, or 5 times a power of ten length, at most a
  quarter of the map width, in the scale of the variant page), compass,
  title cartouche, and grid with labels in config units. A variant value of
  an extra replaces the render value.
- Line widths round up to 3 decimals of a millimetre, so that no line is
  thinner than `min_line_width_pt` after rounding. The `min_line_width`
  check found this fault.
- SVG checks, run on the written SVG text in each report and by
  `inkland check`: `no_opacity`, `no_masks_filters_rgba`,
  `black_and_white`, `min_line_width` (with inherited widths),
  `all_text_is_text_elements`, `font_families_allowed`,
  `deterministic_ids`. Each check has a bad SVG in `fixtures/svg-bad` that
  fails it.
- The PNG preview loads the system fonts and `font_dirs`. It matches a
  family name without regard to case, because a font file can name its
  family "IM FELL English". A missing family gives `FONT_NOT_FOUND`.
## 6.7 Notes from Milestone 13

- `inkland import --map FILE.map -o CONFIG.json` makes a config from a map of
  Azgaar's Fantasy Map Generator (FMG). It prints a JSON summary: the
  version, the seed, the size, the unit, the raster size, the counts of
  places, capitals, ports, and states, and the burgs that it did not keep.
- The import reads plain text, gzip data, and base64 text. It reads map
  versions 1.100.0 to 1.158.x. It refuses other versions with `MAP_VERSION`.
  A file that it cannot read gives `MAP_PARSE`.
- The import reads the version, the seed, and the map size in pixels
  (record 0), the distance unit and scale (record 1), the grid lattice and
  its heights (records 6 and 7), the state names (record 14), and the burgs
  (record 15).
- The config: the frame is the map size times the distance scale. The unit
  name is the distance unit of the map. `world.template` is a raster of the
  grid heights. `world.cells` is the number of grid cells. `world.states` is
  the number of states with a capital. `world.towns` is 0, because each
  burg is a pin.
- Each burg is a point pin. The id comes from the name, in lower-case ASCII
  letters and digits. A second burg with the same id gets `-2`, `-3`, and
  so on. A capital is a `city` with `capital_of` set to the name of its
  state. Another burg is a `town`. The port flag stays.
- The import skips removed burgs, empty entries, burgs without a position,
  burgs outside the frame, and burgs at the same position as an earlier
  burg. It does not make pinned rivers, regions, or roads.
- A raster template gives each cell the height of the raster value nearest
  to its site. Pins apply after the raster, as after template steps.
- Two FMG test maps, 1.112.1 and 1.154.0, are in `fixtures/azgaar/` with
  gzip (about 1 MB each). They import to valid configs and generate. Each
  place stays exactly where the map has it, on land. The land and the sea
  of each world agree with the map, except for the cells that the places
  force to land.

## 6.8 Notes from Milestone 8

- Label placement uses a table of font metrics, not the font files. The
  table has the advance widths, the cap height, and the descender of IM
  Fell English, IM Fell English SC, Caladea, and Inter.
  `examples/font_metrics.rs` makes it from local font files. A family that
  is not in the table uses the IM Fell English widths.
- Labels: each place in the view, each state, each pinned region with a
  `name`, and each pinned river with a `name`. Generated rivers have no
  names in this version.
- Families: places and rivers use `fonts.labels`. States and regions use
  `fonts.labels_sc`. Sizes come from `render.font_sizes_pt`.
- The placer takes the labels in this order: pinned places and capitals,
  then pinned rivers, then states and regions, then other cities, towns,
  and villages. In each group the order is by id. Each label gets the
  candidate with the lowest cost that does not overlap a placed label, a
  place symbol, the scale bar, the compass, or the cartouche, and that is
  inside the map.
- A place label has eight candidates around its symbol: right, top right,
  bottom right, left, top left, bottom left, top, bottom.
- An area label goes on the cells that are farthest from the edge of the
  area. The text is along the page or upright (bottom to top). Letter
  spacing makes the text fill about 70 % of the width of the area, between
  limits. A label part outside the area costs more.
- A river label follows a straight part of its river (chord at least 0.8 of
  the length), above the line, from left to right. It is a `<textPath>` on a
  path in `<defs>` with the id `path-<label id>`.
- A label that crosses a river costs more. Each label has a white halo:
  `stroke="#ffffff"` with `paint-order="stroke"`.
- Relief symbols under a label are left out.
- The scale bar goes in the bottom-right corner when fewer place symbols are
  there than in the bottom-left corner.
- The report: `labels.placed` is the number of placed labels.
  `labels.unplaced` lists each label without a place, with `reason`
  (`collision` or `outside`) and `required`. Required labels are the labels
  of pins, capitals, and states. The check `all_labels_placed` fails only
  for required labels, and each one gives a `LABEL_UNPLACED` warning. Other
  labels, such as generated villages, can be left out without a failure.
- Proptest: no two placed label boxes overlap, and no label box leaves the
  map or covers a symbol or an extra.

## 6.9 Notes from Milestone 9

- A variant with an overlay renders a copy of the world with the overlay
  applied. The copy has the same terrain, rivers, and coasts. Only places,
  roads, and names change.
- `world.json` has `world_hash`: the hash of the config without
  `overlays`, `variants`, `render`, and `font_dirs`. So `world.json` is
  byte-identical when only these change. The report keeps `config_hash`,
  the hash of the full config.
- `add.places`: each place is at its exact position, in the cell under it.
  The terrain does not change, so the place can be on any terrain. The
  report gives `on_land`. A place without a name gets a name from the
  culture of its cell.
- `add.roads`: routed on the shared terrain, after the roads of the world.
  They reuse the cells of those roads at a lower cost. The report section
  `overlay.roads` gives each road as for pinned roads. The check
  `overlay_road_length` fails when a road with a target is not in its
  tolerance.
- `add.features`: a new layer `features`, after `roads`. A point feature is
  a symbol (ruin: cross; battle site: crossed blades; other: triangle). A
  line or an outline is a border (dash-dot), a wall (thick), or a dotted
  line. A named feature gets a label.
- `remove`: `place:<id>` and `road:<id>` leave the copy. `river:<id>` hides
  the river line and its label; the river stays in the world.
  `region:<id>` and `feature:<id>` hide the label or the feature.
- `rename`: changes the label text of a place, region, river, or feature.
- `labels`: `hide` and `size_pt` for one label.
- Crops: labels are placed again for each variant, only for items in the
  view. The scale bar uses the scale of the crop. All map layers are
  clipped to the map area.

## 6.10 Notes from Milestone 10

- `world.json` keeps the part of the config that makes the world, in
  `config`. `read_world` reads it, checks it against `world_hash`, and makes
  the same world again. A file that is not a world, or whose config does
  not agree with its hash, gives `WORLD_PARSE`.
- `inkland query --world FILE --at x,y` prints: the cell and its site,
  `land`, `height`, `sea_level`, `height_above_sea`, the feature, the biome,
  the temperature and the precipitation, the pinned regions that contain
  the point, the culture, state, and province, and the nearest place, river
  (with `side`: `left` or `right` as one looks downstream), and road. A
  point outside the frame gives `VALUE_OUT_OF_RANGE`.
- `inkland export --world FILE --format geojson [--layers ...]` prints a
  FeatureCollection with the layers coast, rivers, regions, roads, places,
  and borders. Coordinates are config units with 3 decimals. `crs` is
  `inkland:local`, and `inkland` gives the units, the frame, and the world
  hash. A circle region is a polygon of 64 points.
- `world.brushes`: heightmap brush strokes `{at, radius, delta}`, applied
  in order after the template and before the pins. A stroke adds `delta`
  at its centre and less with distance, to 0 at the radius.
- `inkland-edit`: commands `set`, `unset`, `add_pin`, `remove_pin`,
  `move_pin`, `rename`, `brush`, and `remove_brush`. Each command works on
  the config and gives its inverse. `History` keeps undo and redo. A script
  is `{"version": 1, "commands": [...]}`.
- `inkland edit --world FILE --script S -o OUT` applies the script to the
  config in the world file, checks the config, makes the world again, and
  writes the new `world.json`. `--config FILE` edits a config file instead.
  The output gives `undo`: a script that gives back the first file.
- Proptest: undo of any command sequence gives back the exact config. Test:
  the undo script of an edit gives back the same `world.json` bytes.

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
