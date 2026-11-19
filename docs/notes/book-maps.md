# Notes: options for the maps of a book

A map in a book shows the places of the story and nothing more. These
options take out what generation adds, and give the author more control.
`docs/CONFIG.md` gives each field.

## Show only the names of the config

```json
"render": { "labels": { "generated": false, "check_generated": true } }
```

- A name is "generated" when the config does not write it. The config
  writes the name of a pin, a region, a river, a zone, the `capital_of` of
  a pin, a rename, and the names of an overlay.
- `generated: false` leaves out each label with a generated name: rivers,
  lakes, seas, islands, states, provinces, and generated places. The
  report lists each one with the reason `generated`.
- `check_generated: true` adds the report check `no_generated_labels`. It
  fails when a variant shows a generated name, and gives the label ids. A
  book build can stop on this check.

## No generated places

```json
"world": { "states": 0, "towns": 0 }
```

- `states: 0` makes no states and no capitals.
- With `towns: 0` too, the only places are the pins.
- To keep a state, give one point pin `capital_of`. That pin is the
  capital, and generation adds no other capital for the state.

## Fewer rivers

```json
"world": { "river_limit": 25 }
```

Generation keeps the pinned rivers and the 25 generated rivers with the
most water. `0` keeps only the pinned rivers.

## Fewer relief symbols

```json
"render": { "relief": { "density": 0.35, "hide": ["tree"] } }
```

- This is for `print-safe` mode. `density` from 0 to 1 puts the symbols
  further apart. 0.35 keeps about a third of them.
- `hide` leaves out a symbol kind: `mountain`, `hill`, `tree` (the
  forests), `marsh`, or `dune` (the deserts).
- `colour` mode uses the style value `relief.options.density`.

## Regions inside regions

```json
{ "id": "lowland", "kind": "plains", "height": [22, 34], "polygon": [...] },
{ "id": "wolds", "kind": "hills", "height": [30, 44], "box": [...], "priority": 1 }
```

Where regions overlap, only the regions with the highest `priority` set
the height range. Regions with the same priority share the range, as
before. Two overlapping regions with ranges that have no common value are
an error only when they have the same priority. A lowland can now hold the
mountains and the hills, and needs no hole for them.

## A sea at the coast

A `sea` region can reach over the coast line. A land cell in the sea, for
example the coast, a port, or a road, is shore. `SEA_NOT_CONNECTED` fails
only when the sea has water that the ocean does not reach. The error gives
that water in `details.at`.

## Labels that find no place

```json
"overlays": { "today": { "labels": { "feature:gaunt": { "at": [128, 226] } } } }
```

- `at` puts the label there, level, centered on the point, before the
  other labels. The other labels keep clear of it.
- The label needs no free place. Its item can be outside the view.
- The key is `<kind>:<id>`, as for `hide` and `size_pt`: `place`, `river`,
  `region`, `feature`, or `state`.
- The key of a state uses the id of its capital pin, for example
  `state:solmere`. A key such as `state:1` is a `REF_UNKNOWN` error. A
  state key is valid only in `labels`, not in `remove` or `rename`.
- With `at`, `size_pt` sets the size of the label. Without `size_pt`, the
  label gets the size that the placer finds for it.
- The options belong to an overlay, so each variant with that overlay
  shows the label at the same position.

## Sea labels

- A `sea` or `lake` region puts its label only on water. The label does
  not go on the land next to the region, also at the frame edge.
- A sea far from a coast has no grid cells, because the grid has no cells
  in deep water. Its label goes on a lattice of points in the region.
- A region label that has no place is in `unplaced` in the report, with
  the reason `collision` or `outside`. With `at`, the label always goes
  to `at`.

## Curved and level labels

```json
"render": { "labels": { "curved": true, "max_angle": 0 } }
```

- An area label can follow a smooth curve along the middle of its area,
  as the original program draws state labels. The curve is one arc. Its
  direction changes by 40 degrees or less from one end to the other. The
  text reads from left to right. Its letters are spaced to fill part of
  the curve. This applies to states, regions, lakes, seas, and islands.
- The SVG keeps the text as a `text` element with a `textPath`, with no
  stroke. In `print-safe` mode, white boxes along the curve make the halo.
- The placer takes a curve before a straight label that fits as well.
  `curved: false` gives only straight area labels.
- A straight area label can turn 90 degrees, to read from bottom to top
  in a tall, narrow area. `max_angle` is the largest angle to the level,
  in degrees, of a straight label and of each part of a curve. `0` gives
  only level, straight labels. It applies in both render modes. The
  default is 90.

## No symbol without a label

```json
"render": { "labels": { "generated": false, "unlabelled_symbols": false } }
```

`unlabelled_symbols: false` leaves out the symbol of a place whose label
the config leaves out. The config leaves out a label for two causes:

- the name is generated, and `generated` is `false`. An example is a
  capital pin with no `name`.
- the label options of the overlay have `hide: true`.

A label that has no free place keeps its symbol. The default is `true`.

## Land up to the frame

```json
"world": { "fill_land": true }
```

The water of the template and the brushes becomes low land, 1 to 3 above
sea level. Only the pinned `sea` and `lake` regions, and the lakes that
rivers make, stay water. The frame shows no sea that the config does not
ask for.

## Text in a PDF

In `print-safe` mode, no text has a stroke. The halo of a label at a point
is a white box behind it, so the lines do not cross the text. A label on a
path, such as a river name, has no box. `render.labels.halo: false` draws
no boxes. The print check `no_text_stroke` fails when any text has a
stroke, because a PDF printer turns stroked text into a Type 3 font.

## A rough coast with places on it

A rough sea or land outline keeps each pinned place where the config puts
it: the noise fades to 0 near each point pin and each place of an overlay.
So a coast town stays on the coast, and a large `roughness` is safe.

## The main river

```json
{ "id": "river-dunn", "name": "The River Dunn", "main": true, ... }
```

`main: true` on a pinned river gives it at least the water flow and the
width of the largest other river, so it is drawn at least as wide.

## Relief for each map

```json
"variants": [
  { "name": "ostrey-then", "relief": { "density": 0.6, "hide_in": ["low-fen"] }, ... }
]
```

- `relief` on a variant replaces `render.relief` for that map.
- `hide_in` lists region pins whose relief symbols the map leaves out, in
  both render modes. One shared terrain can then show a marsh or a forest
  on the map of one age only.

## Fields on farmland

In `print-safe` mode, small fields with furrows stand on open grassland in
the lowland near the places. `"hide": ["field"]` in the relief options
leaves them out.

## A state on two shores

A state with a seat on other land (two pins with the same `capital_of`)
gets a label on each part. The label of a seat part has the key of the
seat pin in the overlay label options, for example `state:east-lassiter`.

## Sea labels

- A label with `at` gets the size that the placer gives it without `at`.
- A sea label that finds no place, for example in a narrow strait, goes on
  the deepest water of its sea, smaller if needed.
- `world.deep_sea: true` sinks the seabed of the template, so its hills do
  not show as shallows in open ocean.

## Fonts of the styles

Georgia and Times New Roman have no free license, so inkland cannot bundle
them. A style that names one writes a list, for example
`Georgia, Caladea, serif`: a system with Georgia uses it, and the preview
uses the bundled Caladea.

## Only the pinned roads

`"world": { "generated_roads": false }` leaves out the roads and trails
that generation adds between the places. The pinned roads and the roads of
the overlays stay. The sea routes stay.

## Region heights and areas

- A `forest` region is 21 to 45 high by default, and a `desert` region 21
  to 50. Give `height` for another range.
- An area in a land region shapes the land inside the range of the region:
  `rough` gives ridges up to the top of the range, `rolling` gives low
  swells, and `flat` keeps the land near the bottom.
- `hide_in` also takes the id of an area pin, so a wood can be an area and
  still be left out on the map of another age.

## Climate

`"climate": { "preset": "temperate" }` gives the middle of the map 11 C at
sea level. The other presets are `warm`, `cool`, `cold`, and `tropical`.
`"temperature": 9` sets the value in degrees C. Without them, a map at 50
degrees is near 0 C at sea level, and its plains become taiga.

## A coast that wanders

```json
{ "id": "western-sea", "kind": "sea", "wander": 40, "roughness": 10, ... }
```

`wander` bends the coast in and out by up to that distance, in bends of
about 6 times its size. The bends fade near each pinned place, so a port
stays on the coast.

## The relief in the report

Each `<variant>.report.json` has a `relief` section: the symbols that the
map shows, and the symbols that each id in `hide_in` leaves out. See
docs/TERRAIN.md for the heights at which hills and mountains get symbols.

## No province borders

`"world": { "provinces_ratio": 0 }` makes no provinces, as in Azgaar's
Fantasy Map Generator. The map then has no province borders, in both
render modes. The state borders stay.

## A sea label in a narrow strait

With `render.labels.max_angle` 0, each label is level. A strait that runs
north and south is often narrower than the level label of its sea, so the
label gets smaller. Give the label a turn in the label options of an
overlay:

```json
"overlays": { "full": { "labels": { "region:serenity": { "angle": 90 } } } }
```

The label then reads from the bottom up along the strait, at the size of
the other sea labels. The other labels stay level.

Without `angle`, a level sea label gets the largest size that fits on the
water, in the middle of the widest part of the strait. A long name in a
narrow strait is still smaller than the other sea labels.

## The scale bar

The scale bar of a `print-safe` map has paper behind it, so no symbol or
line of the map goes through its labels.

## Hills without peaks

A `hills` region shows hill symbols and no ridge peaks, whatever the rest
of the map is. Its default range is 42 to 62. The default range of
`plains` is 21 to 42, so the two kinds still share a height where they
meet. `"peaks": true` on a region lets its ridges show peaks.

## The relief of a variant

A variant `relief` changes only the fields that it gives. The other fields
come from `render.relief`:

```json
"variants": [{ "name": "ostrey-then", "relief": { "hide_in": ["low-fen"] } }]
```

## A symbol kind in chosen places only

```json
"render": { "relief": { "only_in": { "marsh": ["low-fen"] } } }
```

The marsh symbols then stand only in the fen. The kinds are `mountain`,
`hill`, `tree`, `marsh`, `dune`, and `field`. Each takes the ids of
region pins and area pins.

## The build in the report

`report.tool.version` gives the build: `1.3.2` at a release, and for
example `1.3.2+2-gb12d578` for a build after it. A map then records which
build drew it.

## Hill shading

In `colour` mode, `render.hill_shading` lights the slopes that turn to the
north-west and darkens the slopes that turn away. The south-east side of
each hill is darker. On pale land the dark side shows more than the light
side. `"hill_shading": false` leaves the shading out.
