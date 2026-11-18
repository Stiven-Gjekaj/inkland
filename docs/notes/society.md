# Design notes: the society stages

These notes describe the society stages of inkland and how they follow
Azgaar's Fantasy Map Generator (FMG). The FMG files are in
`src/model/society`, `src/model/political`, `src/model/economy`, and
`src/model/story` of the FMG source.

## Order of the stages

`World::build` runs the stages in the order of FMG:

1. `population.rs`: `rankCells`. Suitability and rural population.
2. `cultures.rs`: `Cultures.generate` and `Cultures.expand`.
3. `burgs.rs`: `Burgs.generate` and `assignPorts`.
4. `states.rs`: `createStates`, `expandStates`, `normalize`,
   `findNeighbors`, `assignColors`.
5. `development.rs`: `Development.generate`.
6. `roads.rs` and `routes.rs`: pinned roads, then `Routes.generate`.
7. `religions.rs`: `Religions.generate`.
8. `society::specify`: `burgsSpecify`, `collectStatistics`,
   `defineStateForms`, and `provinces.rs` (`Provinces.generate`).
9. `geonames.rs`: feature and river names.
10. `markers.rs`: `Markers.generate`.

Each stage takes its random numbers from its own part of the seed tree.
FMG uses one global random source. Thus inkland does not give the same map
as FMG for the same seed. This is not a goal.

## Data that follows FMG

- Names: the syllable chain, `getBase`, `getBaseShort`, `getState`,
  `getAdjective`, and `abbreviate`, with the 43 name bases of FMG.
- Culture sets: `world`, `european`, `oriental`, `english`, `antique`,
  `high-fantasy`, `dark-fantasy`, and `random`, with the odds, the
  development, and the sort function of each culture.
- Culture types, expansionism, and the expansion costs of each type.
- Capitals and towns with the spacing rules and the auto town count of
  FMG. The auto count is `populated / 5 / (cells / 10000) ^ 0.8`. `populated`
  is the number of populated cells with a culture in the second grid.
  `cells` is the number of cells of the first grid, as in FMG
  (`grid.points.length`). The second grid has fewer cells, so a count on it
  gives too many towns. Ports by water body. Burg groups, population, and buildings.
- State costs, forms, form names, full names, statistics, and colors.
- Province forms and wild provinces (islands, colonies, territories).
- Route groups (`roads`, `trails`, `searoutes`), Urquhart graphs, path
  costs, and route names.
- Religion types, forms, deities, expansion modes, and heresies.
- Marker types, quantities, and notes.

## Differences from FMG, and why

- Pins win. A pinned place stays at its position and keeps its kind. A
  pinned capital leads the state in its `capital_of`, and that state keeps
  its name. A generated place keeps `min_distance_from_pins` from each pin,
  also after a port moves to the coast.
- Each land cell is in a state and in a province. FMG leaves neutral land.
  inkland gives a cell that no state reaches the state of the nearest cell.
  A state with fewer than 2 places gets one province.
- A pinned place on wildland takes the nearest culture, so that each pinned
  place has a culture.
- Towns keep their spacing from each other. The FMG commit of the port does
  not add towns to the spacing tree; older FMG versions do. Without this,
  towns form dense clusters.
- Names are unique on a map. FMG does not check this.
- The plaza of a place uses the rule of FMG before its market stage,
  because inkland has no economy.
- No diplomacy: the vassal forms (Marches, Dominion, Protectorate) do not
  come.
- Markers that need the economy (mines, guild halls, merchant houses,
  customs houses) are not made.
- Equal costs in a priority queue come out in insertion order.

## Additions for print

- A required label (a pin, a capital, a state with its capital in the
  view, or a pinned river with its source or its mouth in the view) that
  finds no free position can hide the symbols of generated
  places that are not capitals. The SVG leaves out a hidden symbol, and the
  label of that place gets the reason `hidden`.
- The labels of the many generated places are optional.

## Configuration

- `world.culture_set`: the culture set.
- `world.size_variety`: the random part of the expansionism (FMG
  `sizeVariety`).
- `world.growth_rate`: the budget of the culture and state growth (FMG
  `growthRate`).
- `world.provinces_ratio`: provinces for each 100 places (FMG
  `provinces.ratio`).
- `world.towns`: a count, or `auto` for the FMG count.
