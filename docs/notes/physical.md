# Notes on the physical stages

These notes describe the physical stages of inkland after the port of the
physical stages of Azgaar's Fantasy Map Generator (FMG). Paths that start
with `src/` are in the FMG source, commit `74136292`.

## Stage order

`World::build` in `crates/inkland-core/src/world.rs` runs these stages:

1. The first grid (FMG `grid`): a lattice with exact pin sites
   (`grid.rs`).
2. The heightmap: the template, the raster, and the brush strokes.
3. The pins on the first grid: the region ranges and the pinned places.
4. Depression lakes and near-sea lakes (`lakes.rs`).
5. The coordinates of the map on the globe (`coordinates.rs`).
6. The temperature and the precipitation (`climate.rs`).
7. Glaciers and icebergs (`ice.rs`).
8. The second grid (FMG `pack`, `pack.rs`).
9. The pins on the second grid: the region ranges, the ports, and the
   pinned river courses (`fit.rs`, `courses.rs`).
10. The coast markup of the second grid (`Features::mark_pack`).
11. The water flow and the rivers (`rivers.rs`).
12. Biomes, the society, names and groups, religions, and roads.

All stages after step 7 use the second grid. `World::grid` is the second
grid. `World::base` holds the first grid, its heights, its features, its
temperature, and its precipitation. `World::base_cell` gives the
first-grid cell of each cell (FMG `pack.cells.g`).

## What follows FMG

| Stage | FMG source | inkland |
|---|---|---|
| Lattices | `src/model/graph/lattice.ts` | `world.lattice`: `square`, `hex-pointy`, `hex-flat`, `diamond`, `triangle`. `world.jitter`. The steps, the jitter in a circle, and the ring of boundary points. |
| Depression lakes | `src/model/graph/grid-generator.ts:198` | `world.lake_elevation_limit`, default 20. 80 makes no lakes. |
| Near-sea lakes | `grid-generator.ts:242` | A lake behind a shore cell of at most `sea_level + 2` joins the ocean. |
| Map size and latitude | `src/model/physical/coordinates.ts` | `world.climate.geography`: `map_size`, `latitude`, `longitude`. A value that is not given comes from the FMG draw. |
| Temperature | `src/model/physical/temperature-generator.ts` | One latitude for each lattice row. The result is cut toward 0. |
| Second grid | `src/model/graph/pack-generator.ts:17` | Land and coastal water stay. Three of four cells of the second ring of water stay. Sites between far coastal pairs. |
| Coast markup | `src/model/physical/features-generator.ts:160`, `lakes.ts` | Distance field, haven, harbor, feature area and outline, lake shoreline and height. |
| Rivers | `src/model/physical/river-generator.ts:228` | Tilt, closed lakes, depression filling (250 passes), water flow with lake outlets and evaporation, rivers of 3 or more cells, widths, basins, cuts of the river beds. |
| River types | `river-generator.ts:728` | `river`, `creek`, `brook`, `stream`, `fork`, `branch`, with the FMG weights. |
| Feature groups | `features-generator.ts:379` | `continent`, `island`, `isle`, `lake_island`; `freshwater`, `salt`, `frozen`, `dry`, `sinkhole`, `lava`; `ocean`, `sea`, `gulf`. |
| Feature names | `features-generator.ts:434` | Adjectives, map sides for oceans, and names from the culture of the first cell. |
| Ice | `src/model/physical/ice-generator.ts` | Glaciers and icebergs as data in `World::ice` and `world.json`. |

## What differs, and why

- **Pins win.** Each exact site (a pinned place or a pinned river point)
  stays a site of the second grid, also in deep water. The lake stages do
  not change a pinned site, a cell of a land region, or a pinned lake. A
  new cell of the second grid in a region gets a height in the range of
  the region. After the river stage, the heights go back into the region
  ranges.
- **Pinned rivers are locked rivers.** FMG keeps a locked river and cuts a
  generated river where it reaches the locked river (`mergeLocked`). A
  pinned course has fixed, strictly falling elevations, and its cells
  always drain along the course. A course does not tilt, fill, or cut. A
  lake at the end of a course is closed, so its surface does not rise
  above the course.
- **Sea level.** FMG has the fixed sea level 20. inkland translates the
  fixed heights: lake water is `sea_level - 1`, a shore cell that breaks
  is at most `sea_level + 2`, the cut of a river bed starts at
  `sea_level + 15`, and a lava lake is above `sea_level + 40`.
- **Raster templates.** The heights of a raster (for example an imported
  FMG map) hold their lakes already, so the lake stages do not run on
  them.
- **Order of the new sites.** FMG puts each new site of the second grid
  right after its first-grid cell. inkland puts the new sites after all
  kept cells. The cells are the same; only the cell indices differ.
- **Lake islands.** FMG calls an island a lake island when the cell before
  its first cell is in a lake. inkland calls an island a lake island when
  all water next to it is lake water and it does not touch the frame edge.
- **River lines.** FMG draws a meandered line. inkland keeps the cell sites,
  so that a pinned river passes its points exactly. The meander point count
  of FMG still sets the river width.
- **Names.** inkland writes the noun into the name: `Lake <name>`,
  `<name> Ocean`, `<name> Sea`, `<name> Gulf`. A name from a culture does
  not repeat. An adjective can repeat, as in FMG.
- **Feature area.** The area of a feature is the sum of the areas of its
  cells. FMG uses the area of the outline, which also holds the lakes in
  an island.
- **Ice.** The ice uses the random numbers of the part `climate/ice`. No
  layer draws it.

## world.json

The format version is 2. It adds:

- `base`: the lattice, the column and row counts, the sites, and the
  heights of the first grid.
- `grid.base_cell`: the first-grid cell of each cell.
- `coordinates`: the map size, the position, and the latitude and longitude
  box.
- `distance`, `haven`, `harbor`: the coast markup of each cell.
- For each feature: `area` and `outline`; for a lake also `height` and
  `shoreline`.
- For each river: `basin`, `source_width`, `width_factor`, `width`, and the
  mouth kind `edge` for a river that leaves the frame.
- `ice`: glaciers and icebergs.

## Effects on the later stages

- FMG makes many more rivers than the old drainage: 331 rivers on the full
  fixture (30000 cells in the first grid, 16266 in the second), against 11
  before. The river threshold
  of the society, the roads, and the SVG is now the FMG flux of 30. The SVG
  river widths come from the flux; `River::width` holds the FMG width for a
  future layer.
- Places, labels, and the scale bar move with the terrain. A label
  placement check of a crop can fail when a required label has no room.
- The water lines of the SVG come from the first grid, as the ocean bands
  of FMG (`src/model/physical/ocean-generator.ts`). The second grid has no
  deep water.
