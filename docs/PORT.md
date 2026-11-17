# Port review: inkland and Azgaar's Fantasy Map Generator

This review compares inkland 0.1.0 with Azgaar's Fantasy Map Generator
(FMG), commit `74136292`. `docs/ARCHITECTURE.md` lists the stages and the
editors of FMG. Each line below is checked against the inkland source.

## Verdict

inkland is not a full port of FMG, and it is not meant to be one. It is a
full port of the generation pipeline that a printed map needs: terrain,
climate, water, biomes, cultures, places, states, provinces, roads,
religions, and names. Phase 0 dropped the simulation stages (economy,
military, zones, emblems, journeys). Some kept stages are simpler than in
FMG, and three kept stages are not in this version (see "Gaps").

inkland adds what FMG does not have: hard pins, exact and repeatable
output, print-safe SVG with checks, labels that never overlap, variants
and overlays of one world, and a command line for agents.

Status words:

- **Ported**: the same algorithm, with the FMG data tables where they apply.
- **Changed**: the same job, with a different method.
- **Partial**: a part of the FMG stage.
- **Dropped**: left out by a decision of Phase 0.
- **Gap**: kept by Phase 0, but not in this version.

## Generation stages

| FMG stage | inkland | Status | Notes |
|---|---|---|---|
| Grid (lattice, jitter, Voronoi) | `grid.rs` | Partial | Jittered square lattice with Delaunator. FMG also has hex, diamond, and triangle lattices. Pins put exact sites in the grid. |
| Heightmap templates and tools | `heightmap/` | Ported | All 14 templates. All tools: Hill, Pit, Range, Trough, Strait, Mask, Add, Multiply, Smooth, Invert. Whole-number heights, as in FMG. Added: raster templates and brush strokes. |
| Features (land, ocean, lake) | `features.rs` | Ported | Flood fill of land and water. Water that touches the edge is ocean. |
| Depression lakes | none | Gap | A closed low point drains through the priority flood. It does not become a lake. |
| Near-sea lakes join the ocean | none | Gap | |
| Map size and latitude | `climate.latitude_range` | Changed | A config value, not a random draw. |
| Temperature | `climate.rs` | Ported | Latitude bands and the lapse rate of FMG. |
| Precipitation | `climate.rs` | Ported | Winds by latitude band, humidity along rows and columns, rain on slopes. |
| Regraph (`pack`) | none | Gap | inkland keeps one grid. A finer coast grid is not in this version. |
| Haven and harbor of cells | none | Partial | A generated place is a port when it is next to the ocean. |
| Rivers | `rivers.rs`, `courses.rs` | Changed | Priority flood drainage, then tracing. No downcut and no erosion. Added: pinned rivers that pass their points in order and flow downhill. |
| Biomes | `biomes.rs` | Ported | The FMG biome matrix and habitability table. |
| Feature groups | `geonames.rs` | Partial | Continent, island, isle; freshwater and frozen lakes; ocean. FMG also has salt, dry, sinkhole, and lava lakes, and seas and gulfs. |
| Ice and icebergs | none | Dropped | |
| River names and types | `geonames.rs` | Partial | A name from the culture at the mouth. Type `river` or `stream` from the water flow. FMG has more types and a basin id. |
| Feature names | `geonames.rs` | Partial | Names for continents, islands, and lakes. No names for oceans; seas in a book come from pinned regions. |
| Coastline detail | `decor.rs` | Changed | Chaikin smoothing of the cell edges. FMG adds fractal detail. |
| Ocean depth bands | `decor.rs` | Changed | Two water lines along each coast, in black and white. |
| Relief icons | `decor.rs` | Changed | Own black-and-white symbols for mountains, hills, trees, marsh, and dunes, placed with a spacing test. |
| Cell suitability (rankCells) | `society.rs` | Partial | Suitability from biome, river water, height, and coast. No population numbers. |
| Cultures and expansion | `society.rs` | Changed | Centers on the best cells, expansion by least cost with a speed for each culture. No culture types (naval, highland, and so on). |
| Names (Markov chains) | `names/` | Changed | The 43 name bases of FMG, with a letter chain of order 2. FMG uses syllable chains. |
| Burgs | `society.rs` | Partial | Capitals and towns by suitability with spacing; villages. No population, walls, citadels, or temples. |
| States and expansion | `society.rs` | Partial | One state for each capital, expansion by least cost. No state forms (kingdom, duchy) and no statistics. |
| Provinces | `society.rs` | Partial | Seats are the capital and the best towns of each state. Provinces grow inside the state. A cell that no seat reaches joins the nearest province of its state; FMG makes "wild" provinces. |
| Routes | `roads.rs` | Changed | A shortest tree of roads between the capitals of each landmass, and a trail from each town. FMG uses an Urquhart graph and sea routes. Added: pinned roads with target lengths, stops, and river crossings (bridge or ford). |
| Religions | `religions.rs` | Changed | A folk religion for each culture, and organized religions that spread from the best places up to a range. FMG spreads along roads and has more kinds. No print layer. |
| Markers | pins and overlays | Partial | Ruins, battle sites, and other features come from pins and overlays. No random markers. |
| Labels | `labels.rs` | Changed | A new placer for points, areas, and rivers with no overlap. See "Additions". |
| Measurers | scale bar, road report | Changed | A scale bar for each crop, and road lengths in the report. |
| Goods, markets, production, taxes, development | none | Dropped | |
| Military | none | Dropped | |
| Zones | overlay features | Dropped | Overlay features can show old borders and battle areas. |
| Emblems (coats of arms) | none | Dropped | The owner may add them later for four factions. |
| Journeys and legends | none | Dropped | Road lengths in the report replace journeys. |

## Editors and tools

| FMG editor or tool | inkland |
|---|---|
| Generation options | `world` config, `--set` flags |
| Layers | `render.layers` |
| Style presets | print-safe style, `render.font_sizes_pt`, `render.extras` |
| Heightmap editor and templates | `world.template`, `world.brushes`, `edit` command `brush` |
| Lakes, rivers, burgs, markers, routes: create and edit | `pins`, `edit` commands `add_pin`, `move_pin`, `rename`, `remove_pin` |
| Labels | label placement, overlay `labels` options |
| Submap | variant `crop` |
| Units and scale | `units`, scale bar |
| Cell info | `inkland query` |
| Export (SVG, PNG, JSON, GeoJSON) | SVG, PNG preview, `world.json`, `inkland export` |
| Load an FMG map | `inkland import` (heights, burgs, states) |
| Desktop editors | `inkland-app` (see `docs/APP.md`) |

## Additions over FMG

1. **Hard pins.** A place stays at its exact position (error 0). A region
   gets heights in its range. A sea connects to the ocean unless it is
   `landlocked`. A river passes its points in order, flows downhill, and
   ends at its mouth. A road has a target length and a tolerance. The
   engine fits the world around the pins and never moves them.
2. **Feasibility first.** `--dry-run` checks the config and the pins before
   generation. Each failure has a stable error code, a JSON Pointer, and an
   exit code.
3. **Same bytes on each machine.** A seed tree gives each layer its own
   random stream. CI checks golden hashes of the SVG, the world, and the
   report on Linux, macOS, and Windows.
4. **Print-safe SVG.** Black and white only, lines of 0.5 pt or more, real
   `<text>`, fonts by name, page size in mm, stable element ids. Seven
   checks run on the written SVG; `inkland check` runs them on any SVG.
5. **Labels without overlap.** Places, states, regions, rivers, lakes, and
   overlay features. A label that has no free place goes in the report. A
   required label without a place fails a check. White halos, and no relief
   symbols under a label.
6. **Variants and overlays.** Crops with their own scale bar and labels.
   Overlays that add, remove, and rename items for another age of the same
   world, without a change to `world.json`.
7. **Reports for a book.** Road lengths, stops, legs, and river crossings;
   pin positions and their state, province, and culture.
8. **Agent interface.** JSON on stdout, no prompts, JSON Schema, `query` at a
   point, GeoJSON export, and edit scripts with exact undo.
9. **Speed and packaging.** A 20000-cell world with one variant takes about
   0.1 s. The Linux tool is one static binary. Packages for Linux, macOS,
   and Windows come from the release workflow.
10. **Documents from code.** `docs/CONFIG.md` comes from the schema, and
    tests keep `docs/CLI.md` and `docs/ERRORS.md` complete.

## Gaps

These stages were kept by Phase 0, but are not in this version:

1. **Depression lakes and near-sea lakes.** Lakes come only from water
   below sea level and from pinned lake regions.
2. **The second, finer grid (`pack`).** Coasts follow the cells of one
   grid. A region shape in open water gives a block-shaped coast.
3. **River erosion.** Rivers do not cut the terrain.

These stages are simpler than in FMG: lattice types, culture types, state
forms, burg details, feature groups, route graphs, and religions.

Known limits of the pins:

- A road target depends on the cell size. For example, at 50000 cells the
  pinned road of `roads-book-layout` is 265 units against a target of 240
  with a tolerance of 5 %. The report gives this as a failed check.
- A river course across a lake fills a thin strip of land.
