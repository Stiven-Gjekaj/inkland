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
- The options belong to an overlay, so each variant with that overlay
  shows the label at the same position.

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
