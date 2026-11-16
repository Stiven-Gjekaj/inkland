# Architecture of the original program

This file describes Azgaar's Fantasy Map Generator (FMG).
It is the result of `SPEC.md` §0 steps 3 and 4.
We read commit `74136292b8012abe0d57a11ac7780ddc43d9433d` (2026-11-16) in
`./reference`.
All paths in this file are relative to `./reference`.

inkland re-implements the algorithms. It does not copy the code.
Matching the output of FMG for the same seed is not a goal.

## 1. General structure

- FMG is a TypeScript browser app (Vite). The code is in `src/`:
  - `src/model/`: generation stages and data model.
  - `src/renderers/`: SVG drawing with d3.
  - `src/controllers/`: editors and tools (the UI).
  - `src/services/io/`: save, load, export.
  - `src/data/`: tables (name bases, heightmap templates, markers, styles).
- The stage order is in `src/model/generation-pipeline.ts`. A small runner
  (`src/model/pipeline.ts`) calls each step in order.
- The global state is three objects on `globalThis` (`src/model/globals.ts`):
  - `grid`: the first graph, a jittered lattice with a Voronoi diagram.
  - `pack`: a second, finer graph made from `grid`, and all the data that
    comes from it (rivers, cultures, burgs, states, and so on).
  - `options`: `options.generation.*` (requests) and `options.map.*` (what a
    `.map` file stores).
- The RNG is the `alea` package. `src/app/seed.ts:26` replaces the global
  `Math.random` with `Alea(seed)`.
  18 places in `src/model/` make a new `Alea(seed)` at the start of a stage.
  Thus an extra random call in one stage does not change a later stage that
  makes its own new RNG.
  The relief icon stage uses `Math.random` without a new seed. Its output
  depends on the RNG state at render time.
- Heights are integers 0 to 100 in a `Uint8Array`. Sea level is 20
  (`src/utils/height-utils.ts:2`).
- Map coordinates are SVG pixels. The default map is 1280 x 800 px with
  10000 points. `options.map.units.distance.scale` gives distance units for
  each pixel (default 3).
- FMG has no physical print scale (mm on paper). inkland adds one.

### 1.1 Headless use

- No CLI exists.
- Generation runs in Node in the unit tests: `src/model/real-map.testing.ts`
  sets the globals and calls `GenerationPipeline.run({})`. Image heightmaps
  need a canvas, so only template heightmaps work in Node.
- Rendering needs a DOM (d3 and `getBBox`).

### 1.2 The word "pin" in FMG

`src/model/options/pins.ts` stores option values that the user locked in
`localStorage` (`fmg-locks`), for example the template or the number of
states. These are not spatial. FMG has no spatial constraints.
The nearest things are the `lock` flags on entities (states, burgs, cultures,
routes, markers), which keep an entity when its stage runs again.
In inkland, "pin" always means a spatial hard constraint (`SPEC.md` §7).

## 2. Generation stages

The table gives the stages in pipeline order.
"RNG" tells where randomness enters.
"New seed" means that the stage makes a new `Alea(seed)` first.

### 2.1 Physical stages

| Stage | Files and functions | Algorithm | Reads | Writes | RNG |
|---|---|---|---|---|---|
| grid | `model/graph/grid-generator.ts:27` `Grid.prepare/generate`, `lattice.ts:83` `placePoints`, `voronoi.ts:166` | Lattice (square, hex, diamond, triangle) with jitter of up to `jitter * spacing / 2`. Pseudo points outside the edge. Delaunator, then Voronoi. Points are stored row by row, so `Lattice.findSlot(x, y)` is O(1). | `options.map.graph` | `grid.points`, `grid.cells.{v,c,b,i}`, `grid.vertices`, `cellsX/Y`, `spacing` | New seed. 2 rolls per point. |
| heightmap | `model/physical/heightmap-generator.ts:558` `generate`, `:568` `fromTemplate`, tools `addHill :87`, `addPit :124`, `addRange :162`, `addTrough :272`, `addStrait :384`, `modify :443`, `smooth :459`, `mask :472`, `invert :488`. Templates in `data/heightmap-templates.ts`. | A template is a list of steps `Tool a b c d`. Counts like `"5-7"` or `"1.5"` are random. Hill and Pit spread a blob by BFS with a power falloff. Range and Trough walk a ridge with some wobble and spread out ring by ring. Strait cuts across the map. Mask fades the edges. Add, Multiply, Smooth, and Invert change all heights. | `grid.cells.c`, `grid.points`, template | `grid.cells.h` | New seed. Rolls in each tool. |
| markupGrid | `model/physical/features-generator.ts:104` | Flood fill of land (h >= 20) and water parts. Water that touches the border is `ocean`. Other water is `lake`. Distance field `t`: 1 land coast, -1 water coast, -2 to -9 deep water. | `grid.cells.h`, `b` | `grid.cells.t`, `f`, `grid.features` | None. |
| depressionLakes | `model/graph/grid-generator.ts:177`, `:198` | A land low point that cannot reach water through cells lower than `h + lakeElevationLimit` becomes a lake (h = 19). | `grid.cells.h`, `options.generation.lakeElevationLimit` | `grid.cells.h`, `t`, `grid.features` | None. |
| nearSeaLakes | `grid-generator.ts:242` | A lake with a low shore cell (h <= 22) next to the ocean joins the ocean. The shore cell gets h = 19. | `grid.cells.h`, features | `grid.cells.h`, features | None. Note: line 243 compares the template with `"Atoll"`, but the key is `atoll`. The check never stops the stage. |
| mapSize | `model/physical/coordinates.ts:57`, `:68` | Map size in percent of a world and latitude: from a table for real maps, else random (`gauss`). | `grid.features`, options | `options.map.geography` | Uses the stream after markupGrid. |
| temperatures | `model/physical/temperature-generator.ts:7` | Temperature from latitude for each lattice row, minus 6.5 C/km of height. | `grid.cells.h`, coordinates, climate options | `grid.cells.temp` | None. |
| precipitation | `model/physical/precipitation-generator.ts:28` | Winds from the map edges, one bearing for each 30 degree band. Humidity moves along lattice rows and columns. It increases over water and falls as rain on slopes. Cells with h > 85 stop it. | `grid.cells.h`, `temp`, winds | `grid.cells.prec` | New seed. `rand` for coast rain. |
| regraph | `model/graph/pack-generator.ts:17` | Keep land and coast points, and some deep water points. Add points between coast points that are far apart. Voronoi again. | `grid.*` | `pack.cells.{v,c,b,i,p,g,h,area}`, `pack.vertices` | None. |
| markupPack | `features-generator.ts:160` | Same flood fill on `pack`. Also `haven` (nearest water cell), `harbor` (number of water neighbors), feature outlines, lake heights. | `pack.cells.h` | `pack.cells.{t,f,haven,harbor}`, `pack.features` | None. |
| rivers | `model/physical/river-generator.ts:228` `generate`, `drainWater :240`, `flowDown :331`, `defineRivers :367`, `downcutRivers :424`, `alterHeights :543`, `resolveDepressions :556`, `mergeLocked :479` | 1. Tilt heights a little toward the coast. 2. Fill low points: raise them in steps until each land cell has a lower neighbor (up to 250 passes). 3. Sort land from high to low. Each cell adds rain to its flux and gives it to its lowest neighbor (or to its `haven` at the coast). 4. Flux >= 30 makes a river. The bigger river keeps its id at a join. 5. Drop rivers shorter than 3 cells. 6. Erosion: cut rivers down by up to 5 where h >= 35. `generate` also accepts locked rivers and keeps their course. | `pack.cells.{h,t,c,haven}`, `grid.cells.prec/temp`, lakes | `pack.cells.{fl,r,conf,h}`, `pack.rivers`, lake data | New seed. Small use (meander). |
| biomes | `model/physical/biomes-generator.ts` `generate`, `getId` | Moisture = rain plus a river bonus, averaged with neighbors. Then rules: sea, glacier, hot desert, wetland, else a matrix of moisture band by temperature band. | `pack.cells.{fl,r,h,g}`, `grid.cells.{temp,prec}` | `pack.biomes`, `pack.cells.biome` | None. |
| featureGroups | `features-generator.ts:379` | Name the type of each island (continent, island, isle), lake (salt, frozen, dry, and more) and ocean (ocean, sea, gulf), from size and climate. | features, rivers | `subtype`, `group` | None. |
| ice | `model/physical/ice-generator.ts:34` | Glaciers are outlines around land with temp <= -8. Icebergs on cold water. | `grid.cells.{h,temp}` | `pack.ice` | New seed. |
| riversSpecify (late) | `river-generator.ts:728` | River parent, basin, name (from the culture at the mouth), type. | rivers, cultures | river fields | `rw`. |
| featureNames (late) | `features-generator.ts:434` | Names for islands, lakes, seas from cultures. | features, cultures | names | New seed. |

Render-time geometry (not pipeline steps):

| Part | File | Note |
|---|---|---|
| Coastline | `model/physical/coastline-generator.ts:85-160` | Simplify, then add fractal detail to each edge. Has its own seed for each feature. The detail can move the drawn coast across a nearby point. |
| Ocean depth bands | `model/physical/ocean-generator.ts:22` | Rings from `grid.cells.t`. Display only. |
| Relief icons | `model/physical/relief-generator.ts:95` | Rules on height, temperature and biome choose icons. Poisson-disc placement. Uses `Math.random` without a new seed. |

### 2.2 Human and political stages

| Stage | Files and functions | Algorithm | Reads | Writes | RNG |
|---|---|---|---|---|---|
| goods (economy) | `model/economy/goods-generator.ts:33` | Resource deposits on cells. | biomes | `cells.good` | Yes. |
| rankCells | `model/society/population-generator.ts:24` | Suitability `s` from biome, river flux, height, coast type, goods. Rural population from `s` and cell area. | `cells.{biome,fl,h,t,haven,harbor,area}` | `cells.s`, `cells.pop` | None. |
| cultures | `model/society/cultures-generator.ts:1208` `generate`, `:1290` `placeCenter` | Choose centers from the best cells with a minimum spacing (spacing shrinks on retry). Type from the center cell (Naval, Highland, River, and more). | `cells.s`, options | `pack.cultures` | `biased`, `P`, `rand`. |
| culturesExpand | `cultures-generator.ts:1447` | Priority-queue flood from the centers. Cost from biome, height, rivers, coast, divided by expansionism. Stops at a total budget. | cultures, cells | `cells.culture` | Indirect. |
| names | `model/society/names-generator.ts:18` `calculateChain`, `:74` `getBase`, `:157` `getCulture`, `:200` `getState`. Data: `data/name-bases.ts`. | Markov chain on pseudo-syllables (1 to 5 letters, split by vowel rules). Start from the `""` key, add syllables until `max` length. Clean double letters. Fall back to a real name from the base. | name base | strings | `ra`, `P`. |
| burgs | `model/political/burgs-generator.ts:84` `generate`, `:226` `assignPorts` | Capitals: best cells by `s`, spacing `(w+h)/2/states`. Towns: `s * gauss`, smaller spacing, halved until all are placed. Ports: grouped by water body; coastal ports move toward their `haven` edge, river towns move to a bank. | `cells.s`, `haven`, rivers | `pack.burgs`, `cells.burg` | `gauss`, `P`. |
| states | `model/political/states-generator.ts:438` `generate`, `:449` `expandStates`, `:516` `normalize` | One state for each capital. Priority-queue flood. Cost from culture, population, biome, height (sea 1000, mountains 2200), rivers, coast. `normalize` cleans single cells. Pole of the label from `polylabel`. | burgs, cultures, cells | `pack.states`, `cells.state` | `rand`, `P`. |
| routes | `model/economy/routes-generator.ts:229` `generate`; `utils/path-utils.ts:340` `findPath` | For each landmass, an Urquhart graph of burgs (Delaunay with the longest edge of each triangle removed). Groups in order: sea routes (ports), roads (capitals), trails (all burgs). Dijkstra with a heap. Land cost: squared distance, height factor 1 to 3, half cost on a cell that a route already uses. No bridges: rivers do not change the cost. Accepts locked routes. | burgs, cells | `pack.routes`, `cells.routes` | New seed (names only). |
| development | `model/society/development-generator.ts:27` | A field from culture, biome, river, coast, smoothed, with noise. | cells | `cells.development` | Noise. |
| religions | `model/society/religions-generator.ts:633`, expand `:1069` | Folk religion for each culture. Organized religions at big burgs. Flood with road costs. | cultures, burgs, routes | `pack.religions`, `cells.religion` | Yes. |
| burgsSpecify | `burgs-generator.ts:606` | Population (uses route connections), walls, citadel, temple, group (city, town, fort, village, and more). Skips locked burgs. | burgs, routes | burg fields | `gauss`, `P`. |
| stateStatistics, stateForms | `states-generator.ts:598`, `:885` | Area and population. Form name (kingdom, duchy, and more) from size, religion and name base. | states | state fields | `P`. |
| provinces | `model/political/provinces-generator.ts:87`, poles `:538` | Seats from the burgs of each state. Flood inside the state with height costs. Rest becomes "wild" provinces. | states, burgs | `pack.provinces`, `cells.province` | New seed. |
| markets, production, taxes (economy) | `model/economy/*` | Trade simulation. | burgs, goods | economy data | Yes. |
| military | `model/political/military-generator.ts:198` | Units and regiments. | states | `state.military` | Yes. |
| markers | `model/story/markers-generator.ts:187`, `:733` | For each marker kind (volcano, bridge, inn, ruin, battlefield, dungeon, and more), a candidate list, then a weighted draw. One marker for each cell. Ruins: `listRuins :1338`. Bridges: burg cells on big rivers (`:1003`). | cells, burgs, rivers | `pack.markers` | Yes. |
| zones | `model/political/zones-generator.ts:145` | Small floods for events (rebels, disease, flood, and more). | cells, burgs, routes | `pack.zones` | `gauss`. |
| emblems | `model/political/emblems-generator.ts:75` | Coats of arms. Called inside other stages. | parent emblem | emblem data | Heavy use. |
| labels | `model/annotations/added-labels.ts`, `renderers/labels/*`, `controllers/annotations/labels/label-spread.ts:131` | No generation step. State labels follow rays from the state pole. A separate user action moves labels apart: 12 directions x 6 steps for point labels, simulated annealing on box overlap, burg icons are obstacles. It measures text with the DOM (`getBBox`). | entities | label offsets | Seeded Alea in the solver. |
| measurers | `model/annotations/measurers-generator.ts` | A default ruler across the largest landmass. | features | ruler | None. |
| journeys, legends | `model/story/journeys/*`, `model/annotations/legends.ts` | A random story journey. Empty legend boxes. | burgs | data | Yes. |

### 2.3 What inkland keeps

| Keep in inkland | Drop |
|---|---|
| grid, heightmap, features, lakes, temperature, precipitation, regraph, rivers, biomes, feature groups, cultures, names, burgs (places), states, provinces, routes, religions, markers (ruins only, from pins and overlays), labels (new solver), coastline detail, relief icons | goods, markets, production, taxes, development, military, emblems, zones, journeys, legends, ice (later, if a map needs it), precreated real-world heightmaps |

Religions stay in the world model but have no default layer in print-safe
mode.

## 3. Editors and tools

Paths are relative to `src/controllers/` unless they start with `app/`.

### 3.1 Core

These have an equal in inkland, as a config field, a CLI command, or a
desktop editor.

| Editor | File | inkland equal |
|---|---|---|
| Generation options | `app/chrome/tabs/options-tab.ts` | `world` config, editor in the app |
| Layers | `app/chrome/tabs/layers-tab.ts`, `layers-order.ts` | `render.layers` |
| Style presets | `app/chrome/tabs/style-tab.ts`, `style/*` | `render` style, print-safe preset |
| Heightmap editor and template choice | `physical/heightmap-editor.ts`, `heightmap-selection.ts` | `world.template`, heightmap brush (M12) |
| Coastline | `physical/coastline-editor.ts` | coast style |
| Lakes | `physical/lakes-editor.ts` | `pins.regions` kind `lake` |
| Relief icons | `physical/relief/*` | relief layer rules |
| Rivers: create, auto-create, edit | `physical/rivers/river-creator.ts`, `river-auto-creator.ts`, `river-editor.ts` | `pins.rivers` |
| Burgs: create, edit, groups | `political/burgs/burg-creator.ts`, `burg-editor.ts`, `burg-group-editor.ts` | `pins.points`, places editor |
| States | `political/states-editor.ts` | states editor |
| Zones | `political/zones-editor.ts` | overlay features (old borders, battle areas) |
| Name bases | `society/namesbase-editor.ts` | name base data |
| Markers: create, edit | `story/markers/marker-creator.ts`, `markers-editor.ts` | `pins.points` (ruin, custom), overlay features |
| Routes: create, edit, groups, list | `economy/routes/route-creator.ts`, `route-editor.ts`, `route-groups-editor.ts`, `routes-overview.ts` | `pins.roads` |
| Labels: create, edit, groups, spread | `annotations/labels/*` | label placement (M8), labels editor |
| Submap | `graph/submap-tool.ts` | crops (M9) |
| Units and scale | `units-editor.ts` | `units`, scale bar |
| Paint cells | `paint-editor.ts` | region pin drawing |
| Fill as hatch | `color-picker.ts` | print-safe hatches |
| Icons | `icon-picker/*` | symbol set for places |

### 3.2 Nice to have

`app/chrome/advanced-options.ts`, `app/chrome/tabs/heightmap-tab.ts`,
`app/chrome/io-panes.ts` (tile export), `annotations/labels/labels-overview.ts`,
`annotations/measurers-editor.ts`, `physical/image-placement.ts`,
`physical/ice-editor.ts`, `physical/biomes-editor.ts`,
`physical/rivers/river-groups-editor.ts`, `physical/rivers/rivers-overview.ts`,
`physical/features-overview.ts`, `political/burgs/burgs-overview.ts`,
`political/provinces-editor.ts`, `political/provinces-regeneration.ts`,
`society/cultures-editor.ts`, `story/markers/markers-overview.ts`,
`story/markers/markers-settings.ts`, `story/journeys/*` (adventure paths),
`story/lore-editor.ts` (cartouche text), `graph/transform-tool.ts`,
`world-configurator.ts`, `csv-upload.ts`, `cell-info.ts`, `omnibar.ts` (search).

### 3.3 Skip

`app/chrome/options-panel.ts`, `performance-settings.ts`, `tabs/tools-tab.ts`,
`tabs/about-tab.ts`, `app-info.ts`, `physical/elevation-profile.ts`,
`physical/temperature-graph.ts`, `political/diplomacy-*`,
`political/stability-overview.ts`, `political/military/*`,
`political/emblems/*`, `society/religions-editor.ts`, `society/eras-editor.ts`,
`society/development-editor.ts`, `story/markers/markers-in-radius.ts`,
`story/notes-*`, `graph/wrap-tool.ts`, `graph/vertex-brush.ts`,
`hierarchy-tree.ts`, `charts-overview.ts`, `context-menu.ts`, `minimap.ts`,
`view-3d.ts`, `ai-generator.ts`, `assistant/*`, `economy/*` (but not
`economy/routes/*`).

## 4. Fixed inputs: how each stage changes

These rules make the pins of `SPEC.md` §7 hard constraints.
A pin never moves. If a stage cannot satisfy a pin, generation stops with
exit code 3 and a specific error code.

### 4.1 General rules

1. **Resolve pins to cells.** `inkland-constraints` changes each pin from
   config units to world coordinates. Then it maps the pin to cells.
2. **Pin points are cell sites.** Before the Voronoi step, the grid stage puts
   each point pin in the lattice slot that `findSlot` gives. The site moves to
   the exact pin coordinate. The pin is then the center of its cell in both
   `grid` and `pack`. If two pins fall in the same slot, the grid stage adds a
   site. It does not move a pin.
3. **A lock mask.** Each cell has a constraint record: forced height range,
   forced biome, "on a pinned river path", "pinned place". Each stage that
   changes heights reads this mask. FMG has 5 such stages: the heightmap tools
   (Smooth and Mask most of all), depressionLakes, nearSeaLakes, the depression
   fill in rivers, and river erosion.
4. **Apply, then assert.** A stage that writes heights clamps the locked cells
   after it runs. A check at the end of each stage asserts that each locked
   cell is in its range. A failed assert is an `INTERNAL` error (exit 70),
   because it is a bug and not an infeasible config.
5. **Feasibility first.** `--dry-run` and the start of `generate` do the
   checks that need no terrain: pins inside the frame, point pins outside
   forced sea, regions that cannot agree, river mouths in forced water, road
   targets against the straight-line length.
6. **RNG per layer.** Constraint code uses its own layer RNG (`SPEC.md` §4),
   so a pin change does not move random choices in other layers.

### 4.2 Stage by stage

| Stage | Change for fixed inputs |
|---|---|
| grid | Pin point sites, as in rule 2. River `through` points and the mouth also become sites, so the path passes them exactly. |
| heightmap | Run the template. Then run a constraint pass: (a) forced regions set heights inside the shape, in the range of the region (`height` in the config, else the default range of the kind in `PLAN.md` §3.5). Inside the range, the template height is scaled to fit, so the terrain keeps its form; `mountains` also get relief noise. `forest` and `desert` keep the template height but make sure of land. (b) Soft edges: blend from the forced value to the template value over `edge` units, outside the shape. Inside the shape the heights stay in the range. (c) Point pins: the pin cell and one ring around it are land (h >= 21). A port pin also needs one neighbor below 20; if the template gives none, the pass lowers the nearest neighbor toward the sea side. If a forced region blocks this, the error is `PORT_NOT_NEAR_COAST`. The lock mask then protects these cells from later Smooth and Mask steps. |
| markupGrid, markupPack | No logic change in the flood fill. After it, a check: each `sea` region without `landlocked: true` must be in a water part that touches the frame edge, else `SEA_NOT_CONNECTED`. A `landlocked` sea keeps the kind `sea` in the world model and in labels, but the stages treat it as a lake. |
| depressionLakes, nearSeaLakes | Skip locked cells. A depression lake must not cover a point pin or a river path cell. |
| temperature, precipitation | No change. Climate is soft. A `desert` or `marsh` region forces the biome later, not the rain. |
| regraph | Keep every pinned site. Do not add coast midpoints that would move a pinned cell. |
| rivers | Pinned rivers come first. A river has no height in the config. The heights come from the regions. (1) Source: a point or a place is the source cell. A region source: the tool picks the highest cell in the region that has a downhill course to the first `through` point (the `rivers` layer RNG breaks ties). (2) Course: join source, `through` points, and mouth with a least-cost path on `pack` (A*). The cost prefers falling ground and refuses a step that goes up from a forced range into a higher forced range. (3) Carve: give the course strictly falling heights. Each course cell gets a height inside the range of its region, if it has one, and lower than the cell before it. A cell with no region gets a height between its two neighbors on the course. If no strictly falling sequence fits the ranges, the error is `RIVER_NOT_DOWNHILL`. (4) Banks: each cell next to the course and not on it is at least as high as the course cell. (5) Mouth: a point or a port place: the last land cell drains into the water cell at the mouth (set its `haven`). A `marsh` region: the river ends at the first course cell inside the region; this cell has no outflow, and it is the end of the river in the report. Two rivers with the same mouth join at a confluence on the last common cell. (6) Run the usual drainage. The depression fill only raises cells, so a carved course stays. Use the locked-river input (FMG `mergeLocked`): an auto river that reaches a pinned river becomes a tributary. An auto river cannot cross it, because the course is lower than its banks. (7) Erosion skips course cells. |
| biomes | After the usual rules, forced regions set the biome: `marsh` to wetland, `forest` to the forest biome for the temperature band, `desert` to hot or cold desert by temperature, `mountains` keep the usual rule. |
| cultures | No hard change. Each point pin reads its culture from its cell. A pin on a cell with no culture gets the culture of the nearest culture cell, for its name base only. |
| names | A pin with a `name` keeps it. Later stages (state names from a capital) read the pinned name. |
| burgs (places) | Put the pinned places in first, at the exact coordinates, with `lock`. A pin with `capital_of` is a capital. Then auto capitals and towns. A second spatial index of pins refuses auto candidates closer than `places.min_distance_from_pins`. Ports: a pin with `port: true` is a port of its neighbor water body. The port step does not move a pinned place toward its haven, and it does not move it to a river bank. |
| states | Pinned capitals seed states. Other pins fall in whatever state covers them. |
| routes | Pinned roads come after auto routes are planned, but before burgsSpecify and religions use them. See 4.3. |
| burgsSpecify | Skip locked places. Map the pin kind: `city`, `town`, `village` to the group; `fortress` to group `fort` with citadel; `ruin` to a ruin place with no population. |
| provinces, religions | No change. A pinned city can be a province seat. |
| markers | Only pinned or overlay ruins and features. No random markers in v1. |
| labels | New solver in inkland (M8), with font metrics from the font files, not from a DOM. A label that cannot go in is left out and listed in the report. The solver must measure text, but placement must not depend on the fonts of the machine. So inkland keeps a metrics table (advance widths, kerning, ascent, descent) for each permitted family in the repository, made one time from the font files. The four families have free licenses (SIL OFL for IM Fell and Inter, Apache 2.0 for Caladea), so the tables can go in the repository. The SVG does not embed fonts. A family with no table is a config error. |
| coastline detail | The fractal detail stays a fixed distance from each point pin and from each river mouth, so the drawn coast does not cross a pin. |

### 4.3 Pinned roads

1. The router uses a cost field on `pack`: distance, slope, marsh, and a
   crossing cost for each river edge (bridge or ford). Water cells are
   blocked. Cells of other pinned roads cost less, so roads share a path.
2. Path: A* from `from` through each `via` to `to`.
3. Length control: the router computes the path length in config units.
   - If the path is too short, the router adds detour cost or a meander
     until the length is in `target * (1 +- tolerance)`.
   - If the shortest path is still too long, the router cannot shorten it.
     The road is written, and the `pinned_road_length` check fails (exit 1).
   - If the target is shorter than the straight line through all the via
     points, `--dry-run` and `generate` stop with `ROAD_TARGET_TOO_SHORT`
     (exit 3).
4. The router uses the `roads` layer RNG for its choices.
5. Crossings: the router walks the path. An edge that goes from one side of a
   river to the other side is a crossing. An edge that follows the river is
   not. The report gives the river id, the length along the road at the
   crossing, and the kind: `bridge` on a road, `ford` on a trail where the
   river flux is small.
6. The auto route stage gets the pinned roads as locked routes, so auto
   routes join them.

## 5. Renderers and print safety

FMG draws with d3 into a live SVG. inkland writes its own SVG.
This list tells which FMG effects are not print-safe, so inkland does not
copy them.

- Opacity on many layers in `src/model/styles/default-styles.json`: borders
  0.8, burg icons 0.7, cultures 0.6, lakes 0.5, and more (62 values).
- Masks: ocean waves, coastal bands, lakes, landmass, heightmap, vignette, fog.
- Filters: `blur*`, `dropShadow*`, `splotch`, `turbulence`, `pencil`,
  grayscale and sepia filters (`src/index.html`).
- CSS `text-shadow` halos on labels. inkland uses a white stroke under the
  text with `paint-order: stroke` instead.
- Raster images: paper texture, the ocean canvas cache.
- The `ink.json` and `monochrome.json` presets in `public/styles/` look like
  print styles, but they use grayscale filters and opacity.
- Clean parts that inkland can follow: rivers as filled polygons, routes with
  dashes, borders, relief icons as `<use>`, hatch patterns (`hatches.ts`).

## 6. Saved maps and units

- A `.map` file is UTF-8 text, records split by CRLF, about 70 records in a
  fixed order (`src/services/io/save.ts`). Record 0 is
  `VERSION|license|date|seed|width|height|mapId`. Other records are JSON or
  comma-separated typed arrays. The browser saves it with gzip.
- The loader (`src/services/io/load.ts`) accepts plain text, base64, or gzip.
  It applies about 60 version migrations (`src/services/io/auto-update.ts`).
  The version is semver; the current one is 1.158.0.
- Test maps for an importer are in `tests/fixtures/*.map` (versions 1.112.1
  to 1.154.0). An inkland importer (M13) accepts 1.150 and later only.
- Units: `options.map.units.distance = {unit, scale}`, units per pixel.
  The scale bar in FMG depends on the screen zoom. inkland computes it from
  the page size and the crop, with no zoom.
