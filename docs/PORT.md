# Port review: inkland and Azgaar's Fantasy Map Generator

This review compares inkland 1.0 with Azgaar's Fantasy Map Generator
(FMG), commit `74136292`. The notes in `docs/notes/` give each stage with
its FMG source and the details that differ.

## Verdict

inkland is a faithful remake of FMG in Rust. It ports each generation
stage of FMG, the colour layers of the 15 style presets, and the data of
the FMG editors. It adds what FMG does not have: hard pins, exact and
repeatable output, print-safe SVG with checks, labels that never overlap,
variants and overlays of one world, and a command line for agents.

An FMG editor changes the map in the browser. In inkland, each edit is a
value in the config, so the edit comes back after each regeneration, and
undo gives back the exact config.

Status words:

- **Ported**: the same algorithm, with the FMG data tables.
- **Changed**: the same job, with a different method, for a reason that
  the notes give.

## Generation stages

| FMG stage | inkland | Status |
|---|---|---|
| Grid: square, hex, diamond, and triangle lattices, jitter, Voronoi | `grid.rs` | Ported |
| Heightmap templates and tools, and imported heightmaps | `heightmap/` | Ported |
| Features, depression lakes, near-sea lakes | `features.rs`, `lakes.rs` | Ported |
| Map position, latitude, temperature, precipitation, winds | `coordinates.rs`, `climate.rs` | Ported |
| The second grid (`pack`), haven, and harbor | `pack.rs`, `features.rs` | Ported |
| Rivers, downcut, meandering, and widths | `rivers.rs`, `meander.rs` | Ported |
| Biomes, ice, and icebergs | `biomes.rs`, `ice.rs` | Ported |
| Feature groups and names, river names and types | `geonames.rs` | Ported |
| Cell ranking, population | `population.rs` | Ported |
| Cultures, culture types, and culture sets | `cultures.rs`, `culture_sets.rs` | Ported |
| Names: the syllable chains of the 43 name bases | `names/` | Ported |
| Burgs, ports, and their details | `burgs.rs` | Ported |
| States, state forms, provinces, wild provinces | `states.rs`, `provinces.rs` | Ported |
| Routes: the Urquhart graph, roads, trails, and sea routes | `routes.rs`, `roads.rs` | Ported |
| Religions | `religions.rs` | Ported |
| Coats of arms | `emblems/` | Ported |
| Diplomacy and campaigns | `diplomacy.rs` | Ported |
| Goods, deposits, markets, production, trade, taxes, prosperity | `economy/` | Ported |
| Military and stability | `military.rs`, `stability.rs` | Ported |
| Markers and zones | `markers.rs`, `zones.rs` | Ported |
| Journeys | `journeys/` | Ported |
| Measurers, legend boxes, and notes | `annotations.rs` | Ported |
| Labels | `labels.rs` | Changed: a placer with no overlap |

## Colour layers

All FMG layers draw in `colour` mode with the style presets: ocean with
its pattern and waves, landmass, the paper texture, heightmap tints,
contours and hachures, lakes with ripples, biomes, cells, grid,
coordinates, compass, rivers, relief icons, religions, cultures, states,
provinces, zones, borders, routes, temperature, coastline, ice, goods,
markets, trade, precipitation with wind arrows, population, emblems, burg
icons, labels, military, markers, journeys, rulers, scale bar, vignette,
and the legend. `docs/notes/render.md` gives the sources.

## Editors and tools

| FMG editor or tool | inkland |
|---|---|
| Generation options | `world` config, `--set` flags, Options tab |
| Layers and style presets | `render.layers`, `render.style`, `render.style_overrides` |
| Heightmap editor | `world.template`, `world.brushes`, `brush` command |
| Burgs, lakes, rivers, routes, regions: create and edit | `pins`, pin commands, app editors |
| States, cultures, religions, provinces: names | `world.renames`, `name_area` command, app editors |
| Emblems editor | pinned arms, `world.renames` arms, app editor with JSON or selects |
| Goods, units, transports | `world.goods`, `world.military_units`, `world.transports` |
| Taxes, budget, tariffs | `world.finance`, Economy dialog |
| Markets | `world.markets`, Markets dialog |
| Regiments | `world.regiments`, Military dialog |
| Journeys | `world.journeys`, Journeys dialog |
| Zones, notes, measurers, legend boxes | `world.zones`, `world.notes`, `world.measurers`, `world.legends` |
| Submap | variant `crop` |
| Cell info | `inkland query`, Cell Details |
| Export (SVG, PNG, JSON, GeoJSON) | SVG, PNG preview, `world.json`, `inkland export` |
| Load an FMG map | `inkland import` |

The `insert`, `remove`, `set`, and `unset` commands edit each list of the
config with exact undo.

## Additions over FMG

1. **Hard pins.** A place stays at its exact position. A region gets
   heights in its range. A river passes its points in order and flows
   downhill. A road has a target length. The world fits around the pins.
2. **Feasibility first.** `--dry-run` checks the config and the pins before
   generation. Each failure has a stable error code, a JSON Pointer, and an
   exit code.
3. **Same bytes on each machine.** A seed tree gives each layer its own
   random stream. CI checks golden hashes on Linux, macOS, and Windows.
4. **Print-safe SVG.** Black and white only, lines of 0.5 pt or more, real
   `<text>`, fonts by name, page size in mm. `inkland check` runs the
   print checks on any SVG.
5. **Labels without overlap**, with halos, and a report of each label that
   has no place.
6. **Variants and overlays** of one world: crops, and other ages with
   added, removed, and renamed items.
7. **Agent interface.** JSON on stdout, a JSON Schema, `query`, GeoJSON
   export, and edit scripts with exact undo.
8. **Documents from code.** `docs/CONFIG.md` comes from the schema, and
   tests keep `docs/CLI.md` and `docs/ERRORS.md` complete.

## What is not ported

- The trade animation: the SVG is still, so the trade layer has lines.
- The fogging layer.
- Map drawing tools in the app for new markets, regiments, journeys,
  measurers, legend boxes, and zone shapes. The config and the CLI have
  these edits (GitHub issue 21).
- The icon font of FMG in the app (GitHub issue 22).
- On Linux, the PNG preview cannot draw color emoji of CBDT fonts. The SVG
  has the emoji.
