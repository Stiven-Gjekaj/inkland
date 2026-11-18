# Design notes: coats of arms

These notes describe the coats of arms (emblems) of inkland and how they
follow Azgaar's Fantasy Map Generator (FMG). The FMG files are
`src/model/political/emblems-generator.ts`, `src/data/emblems/*.ts`,
`src/renderers/emblems/*.ts`, `src/renderers/draw-emblems.ts`, and
`src/controllers/political/emblems/emblems-editor.ts`.

## Generation

- `crates/inkland-core/src/emblems/mod.rs` is a port of `generate`,
  `getTincture`, `definePattern`, `selectCharge`, `getSize`, and
  `defineChargeAttributes`. The tables are in `emblems/data.rs`, in the
  key order of FMG.
- The stage runs after the provinces, with its own random part
  (`states/emblems` of the seed tree). Thus it moves no other random
  numbers. The order is the order of FMG: states (`createStates`), then
  places (`defineEmblem` of `burgsSpecify`), then provinces
  (`Provinces.generate`).
- A place takes from the arms of its state with a kinship of 0.25, plus
  0.1 for a capital, less 0.1 for a port, and less 0.25 for another
  culture. A province with a seat takes from the arms of its seat (0.8
  when it has the name of the seat, else 0.4). A wild province takes from
  its state, and is a dominion with a canton of the arms of the state at
  the chances of FMG (colony 0.95, island 0.7, other 0.3).
- The shield shape comes from `world.emblem_shape`: `culture` (default),
  `state`, `random`, or one shield. Each culture of a culture set has the
  shield of FMG. A culture of the `random` set gets a random shield in
  the emblem stage.

## Data and output

- A coat of arms has the JSON shape of FMG: `t1`, `division`,
  `ordinaries`, `charges`, and `shield`. `sinister` and `reversed` are
  `1`.
- `world.json` has `coa` for each state, province, and place, `shield`
  for each culture, and `wild` for each province. `inkland query` gives
  the arms of the state, the province, and the nearest place. The
  GeoJSON export gives the arms of each place.

## Custom arms

- `coa` of a point pin replaces the arms of the place. `state_coa` of a
  pin with `capital_of` replaces the arms of its state. The generated arms
  of the places and provinces take from the custom arms.
- The generator still draws its arms first, so custom arms do not move
  the random numbers of the items before them.
- The config check refuses an unknown tincture, shield, division,
  ordinary, line, charge, or position (`COA_INVALID`), as `requireHeraldic`
  of FMG does.

## Drawing

- `crates/inkland-render/src/emblem/` is a port of `renderer.ts`: the
  field, the division with its line, the ordinaries, the charges at the
  positions of the shield, the counterchanged parts, and the backlight.
- The charge files use CSS classes and CSS variables. resvg and print do
  not use CSS, so the renderer writes the fills of the classes into a
  copy of the charge for each set of tinctures.
- The emblem layer of colour mode uses `getEmblemSize`, the pole of
  inaccessibility of each state and province (`polylabel` with a
  precision of 20 pixels), and the collision force of d3 for 3 ticks.
  As in FMG, a group is hidden when its emblems are smaller than 25 or
  larger than 300 pixels at the zoom of the variant. `render.emblems.show_all`
  shows each group.
- `render.emblems.hatching` draws the emblems in print-safe mode, in black
  and white. The tinctures have the hatching of Petra Sancta: or dots,
  gules vertical lines, azure horizontal lines, vert lines in bend,
  purpure lines in bend sinister, sable black, argent white. Tenné,
  sanguine, murrey, carnation, celeste, and cendrée have two of these
  hatchings. Each line is at least `render.min_line_width_pt` wide on the
  page, also in a scaled charge and in a pattern tile. Each ordinary, each
  line of partition, and the shield have a black outline. Charge parts with
  an opacity are left out.

## Differences from FMG

- The charges are only the 108 charge files of FMG with the license CC0.
  The other charge files of FMG have licenses (for example CC BY-NC-SA
  3.0) that do not agree with the license of inkland. The generator draws
  only from categories and charges with art, with the weights of FMG. The
  categories beasts, beast heads, and fishes have no CC0 charge, so the
  arms have no lions or eagles. A charge of a culture type without art
  falls back to the normal choice.
- Picture emblems, inscriptions, diapers, the zoom of a shield, and the
  placement fields (`x`, `y`, `size`, `angle`, `stretch`, `outside`,
  `layered`, `compony`, `gyronny`) are not in the config shape.
- FMG uses one random source, so inkland does not give the same arms as
  FMG for the same seed.
