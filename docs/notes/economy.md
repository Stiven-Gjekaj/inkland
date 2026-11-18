# Design notes: diplomacy, economy, military, zones, and the story

These notes describe the stages of inkland after the society stages, and
how they follow Azgaar's Fantasy Map Generator (FMG). The FMG files are in
`src/model/political`, `src/model/economy`, `src/model/story`, and
`src/model/annotations` of the FMG source. The FMG design of the economy
is in `docs/domain/economy.md` of the FMG source.

## Order of the stages

`World::build` runs the stages in the order of the FMG generation pipeline
(`model/generation-pipeline.ts`):

1. `economy/goods.rs`: `Goods.generate`. The deposits come before
   `rankCells`, and `rankCells` adds the goods bonus of FMG.
2. The society stages (see `docs/notes/society.md`). The state stage adds
   `generateCampaigns` and `generateDiplomacy` (`diplomacy.rs`). The forms
   stage adds the vassal forms and the finance of each state: the sales
   tax, the poll tax, the market fee, the tariffs, the war alert, the
   budget, and the corruption.
3. `economy/markets.rs`: `Markets.generate`. A market center gets a plaza.
4. `economy/production.rs`: `Production.produce`, one day of trade, with
   `trade.rs`, `taxes.rs`, `orders.rs`, `deals.rs`, `routes.rs`, and
   `output.rs`. Then `Taxes.collect`.
5. `economy/prosperity.rs`, `accounts.rs`, and `traffic.rs`: prosperity,
   the state, province, and world accounts, and the trade traffic.
6. `military.rs`: `Military.generate`.
7. `stability.rs`: the stability of the states and provinces.
8. `markers.rs`: `Markers.generate`, with the markers that read the
   economy.
9. `zones.rs`: `Zones.generate`.
10. `annotations.rs`: the default ruler (`createDefaultRuler`). Generation
    makes no legend box (`Legends.initiate`).
11. `journeys/`: `Journeys.generate`, one story journey. This stage is
    the last.

Each stage takes its random numbers from its own part of the seed tree:
`States/campaigns`, `States/diplomacy`, `Economy/goods`,
`Economy/markets`, the layer `Military`, the layer `Markers`, the layer
`Zones`, and `Journeys/story`. The economy run itself uses no random
numbers, as in FMG.

## Data

- `State::diplomacy` holds the relation to each state, by state id. Entry
  0 is the neutral lands. `State::campaigns` holds the old wars and the
  ongoing wars. `Society::chronicle` is the diplomatic history.
- `State::economy` (`StateFinance`) and `State::alert` hold the finance.
  `State::military` holds the regiments.
- `Society::cells.good` holds the deposits (FMG `cells.good`).
- `World::economy` holds the goods, the markets, `cell_market`, the deal
  log, the economy of each place, corruption, prosperity, the accounts,
  and the traffic.
- `World::stability`, `World::zones`, `World::journeys`,
  `World::measurers`, and `World::legends`. `annotations::notes` lists
  the notes of the regiments, the markers, and the zones.

FMG computes prosperity, the accounts, and stability on demand. inkland
computes them one time when it builds the world, because the zones, the
markers, the overviews, and the output read them.

## Data that follows FMG

- The goods catalogue of `data/goods.ts`, with the distribution expression
  language, the deposit counts, and the clusters.
- Relations, vassals, wars, and the chronicle; the vassal forms Marches,
  Dominion, and Protectorate.
- Tax rates, market fees, tariffs from the relations, the war alert, the
  expenses, and the corruption by form, size, and diversity.
- Market placement, territories, recentering, and margin rates.
- The production run: rural output, deposits, prices, lots, recipes,
  specialization, carriage over the route network, imports, tariffs,
  embargoes, state orders, poll tax, and licenses.
- Prosperity levels, accounts, economy types, and the good that a state is
  known for.
- The military roster, drafts, regiments, names, notes, fronts, upkeep,
  equipment, and power.
- The parts of stability.
- The economy markers (mines, guild halls, merchant houses, customs
  houses), and the economy rules of bridges, inns, brigands, pirates, and
  fairs.
- All 12 zone types with their quantities, names, notes, and hatch colors.
- The journey transports, the lore tables, the archetypes, the
  pathfinding, the story, and the fallback journey.

## Differences from FMG, and why

- Pins win. The economy, the military, the zones, and the journeys only
  read the world. They do not move a pin, a pinned road, or an overlay.
  The report does not change.
- Each land cell is in a state, a province, and a market. Thus the FMG
  paths for neutral land and for cells outside a province do not occur.
- Positions are in world units. The decima, the market spacing, and the
  remoteness are ratios that do not depend on the scale. The default
  ruler keeps the FMG margin of 100 FMG pixels.
- Equal costs in a priority queue come out in insertion order. Sorts are
  stable. `libm` gives the math functions, so the last bits can differ
  from JavaScript.
- The goods catalogue, the military roster, and the journey transports
  start as the FMG defaults. `world.goods`, `world.military_units`, and
  `world.transports` change, add, or remove entries, as the FMG editors do.
  Only the culture type multipliers of a good apply, because the defaults
  use only those. User adjustments are not in the config, so they are
  zero, as on a new FMG map.
- The edit and regeneration functions of FMG (add a market, set taxes,
  move a regiment, edit a journey) are not ported. The app shows these
  data in read-only dialogs.
- The economy debug log is not ported.
- The era of the calendar comes from the markers part of the seed tree,
  and the military uses the same era.
- Journey and segment ids start at 1. A segment keeps the domain and the
  hours per day of its transport. Water paths do not meander.
- Text has no em-dash: two bivouac lines of the journey lore, the lines of
  a regiment note, and the mine names use a colon or a hyphen.
- The places that an overlay adds in a variant are not in the economy.

## Output

- `world.json` holds all the data above (see `docs/CLI.md`). The deal log
  is a table of `fields` and `rows`, because it is the largest part.
- `inkland query` gives the deposit, the market, the prosperity, the
  stability, the zones, and the nearest regiment of a point.
- `inkland export` has the layers `zones`, `markets`, `trade`,
  `regiments`, `journeys`, `measurers`, and `deposits`.
- Colour mode draws `zones`, `goods`, `markets`, `trade`, `military`,
  `journeys`, and `rulers` (see `docs/notes/render.md`).

## Speed

At 20000 cells, one variant, in release, on the machine of the port:

| Branch | `generate` time |
|---|---|
| `main` before these stages | 0.15 s |
| With these stages | 0.23 s |

The economy run is about 0.1 s of this. The military takes about 13 ms,
the markers about 3 ms, the zones about 1 ms, and stability less than
1 ms. Thus the economy has no option to turn it off.
