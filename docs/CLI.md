# Command line reference

The `inkland` tool never asks a question. Each option is a flag or comes
from the config file. Each command prints JSON on stdout. Logs go to stderr,
and only with `--verbose`. `--verbose` works with each command.

## Errors and exit codes

An error prints one JSON object on stdout:

```json
{ "error": { "code": "PIN_IN_FORCED_SEA", "message": "...", "path": "/pins/points/3", "details": {} },
  "errors": [ ... ] }
```

`error` is the first error. `errors` has all of them. `path` is a JSON
Pointer into the config. `docs/ERRORS.md` lists each code.

| Exit code | Meaning |
|---|---|
| 0 | The command is done. |
| 1 | The output is written, but one or more checks failed. |
| 2 | The config, the flags, or an input file is not valid. |
| 3 | The pins cannot all be satisfied. |
| 4 | A file cannot be read or written. |
| 70 | An internal fault. |

## `inkland generate`

Generate the maps of a config.

```bash
inkland generate --config map.json --out-dir out --preview out/map.png
```

| Flag | Meaning |
|---|---|
| `--config FILE` | The config file. Required. |
| `--out-dir DIR` | The output directory. Default `out`. |
| `--preview FILE` | Also write a PNG preview. With more than one variant, each preview gets the variant name: `map-<variant>.png`. |
| `--preview-width N` | The longer side of the PNG preview, in pixels. Default 1024. The permitted range is 64 to 8192. A value out of the range is a `CLI_USAGE` error. |
| `--variant NAME` | Render only this variant. |
| `--dry-run` | Check the config and the pins. Generate nothing and write nothing. The output is the dry-run report. |
| `--seed N` | Replace `seed`. |
| `--cells N` | Replace `world.cells`. It also removes `units.per_cell_hint`, so `N` sets the cell count. A `--set` of the hint applies after it and sets the hint again. |
| `--set POINTER=JSON` | Replace one value, for example `--set /world/cells=20000`. You can give it more than one time. It applies after `--seed` and `--cells`. |

Files in the output directory:

- `world.json`: the base world. All variants share it. It does not change
  when only overlays, variants, or render options change. Besides the
  terrain and the society, it holds the relations and the campaigns of each
  state and the `chronicle`, the goods, the deposits (`cell_good`), the
  markets (`cell_market`), the deal log (`deals`: `fields` and `rows`),
  the economy of each place (`place_economy`), the finance, accounts,
  regiments, and stability of each state, `cell_prosperity`, the `zones`,
  the `journeys`, the `measurers`, the `legends`, and the `notes`.
- `<variant>.svg`: the map, in mm, black and white, with real `<text>`.
- `<variant>.report.json`: pins, labels, checks, and warnings.

The summary on stdout gives the files and `checks_failed`. When a check
fails, the files are written and the exit code is 1.

## `inkland schema`

Print the JSON Schema of the config.

```bash
inkland schema > config.schema.json
```

| Flag | Meaning |
|---|---|
| `--markdown` | Print the config reference in Markdown (`docs/CONFIG.md`), not the JSON Schema. |

## `inkland import`

Make a config from a map file of Azgaar's Fantasy Map Generator.

```bash
inkland import --map world.map -o map.json
```

| Flag | Meaning |
|---|---|
| `--map FILE` | The `.map` file: plain text, gzip, or base64. Versions 1.100.0 to 1.158.x. |
| `-o`, `--out FILE` | The config file to write. |

The config has the heights as a raster template, each burg as a point pin,
and the states. The summary on stdout gives the counts and the burgs that
the import did not keep.

## `inkland query`

Print facts about one point of a world.

```bash
inkland query --world out/world.json --at 120.5,88
```

| Flag | Meaning |
|---|---|
| `--world FILE` | The `world.json` of a generated world. |
| `--at X,Y` | The point, in config units. |

The output gives the cell, `land`, `height`, `height_above_sea`, the feature,
the biome, the temperature and the precipitation, the pinned regions that
contain the point, the culture, state, and province, and the nearest place,
river, and road with their distances. The state, the province, and the
nearest place have `coa`, the coat of arms. `nearest_river.side` is `left` or
`right` as one looks downstream.

`feature.group` is the FMG group of the feature: for example `ocean`, `sea`,
`gulf`, `freshwater`, `salt`, `frozen`, `continent`, or `isle`. `coast` gives
`distance` (1 for land next to water, then 2, 3, ...; -1 for water next to
land, then -2, -3, ...), `haven` (the nearest water cell of a coastal land
cell), and `harbor` (the number of water cells next to a land cell). `ice` is
the glacier or iceberg over the point, with its id and type.
`population` is the rural population of the cell.

The output also gives the data of the economy and the story stages:
`deposit` (the good of the cell), `market` (its id and the name of its
center), `prosperity` (0 to 100, land only), `stability` (of the state and
of the province, 0 to 1), `zones` (the zones that hold the cell), and
`nearest_regiment`.

## `inkland export`

Export a world as GeoJSON, to stdout.

```bash
inkland export --world out/world.json --format geojson --layers coast,rivers,roads > map.geojson
```

| Flag | Meaning |
|---|---|
| `--world FILE` | The `world.json` of a generated world. |
| `--format geojson` | The format. Only `geojson`. |
| `--layers LIST` | The layers, separated by commas: `coast`, `lakes`, `ice`, `rivers`, `regions`, `sea_routes`, `roads`, `places`, `markers`, `borders`, `zones`, `markets`, `trade`, `regiments`, `journeys`, `measurers`, `deposits`, `population`. Default: all. |

The coordinates are config units of the frame with 3 decimals, not WGS84.
`crs` is `inkland:local`. Each feature has `layer`, an id, and, where they
apply, a pin id, a name, a kind, and a length. A place has `coa`, its coat
of arms. `zones` and `markets` are MultiPolygons of their cells, with the
holes in each outer ring. `trade` has one line for each route link that
carries goods, with the units of the day. `regiments` and `deposits` are
points. `journeys` has one line for each segment.

A `coast` line names its land and water features, with `water_kind`
(`ocean` or `lake`) and `water_group`. `lakes` are MultiPolygons of the
cells of each lake, with the group, the name, the number of cells, and the
area. `ice` has one Polygon for each glacier and iceberg, with its type and
the size of an iceberg. `population` has one point for each cell with rural
population.

## `inkland emblem`

Write the coat of arms of one state, province, or place as an SVG file.

```bash
inkland emblem --world out/world.json --state 3 -o state-3.svg
inkland emblem --world out/world.json --place ostrey --hatching --size-mm 20 -o ostrey.svg
```

| Flag | Meaning |
|---|---|
| `--world FILE` | The `world.json` of a generated world. |
| `--state ID` | A state: its id or its name. |
| `--province ID` | A province: its id or its name. |
| `--place ID` | A place: the id of its pin, its id, or its name. |
| `--hatching` | Black and white, with the heraldic hatching of Petra Sancta. Each line is at least `render.min_line_width_pt` of the config wide at the given size. |
| `--size-mm N` | The width and height of the SVG, in mm. Default 50. |
| `-o FILE`, `--out FILE` | The SVG file to write. |

Give exactly one of `--state`, `--province`, and `--place`. An item that
the world does not have is `REF_UNKNOWN`. The output on stdout gives `svg`
(the file), `id` (for example `state-3`), `name`, and `coa` (the coat of
arms in the JSON shape of Azgaar's Fantasy Map Generator). With
`--hatching`, it also gives `checks`, the print checks of the SVG.

## `inkland edit`

Apply an edit script to a world file or to a config file.

```bash
inkland edit --world out/world.json --script edits.json -o out/world.json
inkland edit --config map.json --script edits.json -o map.json
```

| Flag | Meaning |
|---|---|
| `--world FILE` | A `world.json`. The output is a new `world.json`. |
| `--config FILE` | A config file. The output is a new config file. |
| `--script FILE` | The edit script. |
| `-o`, `--out FILE` | The file to write. |

Give `--world` or `--config`. An edit script:

```json
{ "version": 1, "commands": [
  { "op": "move_pin", "id": "ostrey", "at": [120, 88] },
  { "op": "brush", "at": [200, 150], "radius": 20, "delta": 15 },
  { "op": "rename", "list": "points", "id": "harrow", "name": "Harrowgate" },
  { "op": "set", "path": "/world/sea_level", "value": 22 }
] }
```

The commands are `set`, `unset`, `add_pin`, `remove_pin`, `move_pin`,
`rename`, `brush`, `remove_brush`, `name_area`, `remove_rename`, `insert`,
and `remove`. `insert` puts a value into a list at a pointer that ends
with an index, for example
`{ "op": "insert", "path": "/world/notes/0", "value": { ... } }`, and
`remove` takes the value at such a pointer away.
`name_area` gives a generated state, culture, religion, or province a name
or arms, for example
`{ "op": "name_area", "rename": { "kind": "state", "pin": "ostrey", "name": "Ostreya" } }`.
Without `index`, it changes the entry of the same area, or adds one at the
end. `remove_rename` removes the entry at `index`. The output on stdout has `undo`: a
script that gives back the first file. The edited config must pass all
config checks, else nothing is written.

A `world.json` keeps only the part of the config that makes the world.
Thus `--world` refuses a `set` or `unset` of `/overlays`, `/variants`,
`/render`, or `/font_dirs` with the error `EDIT_OUTSIDE_WORLD`, and
writes nothing. Apply such a script to the config file with `--config`.

## `inkland check`

Run the print checks on an SVG file.

```bash
inkland check --svg out/map.svg
```

| Flag | Meaning |
|---|---|
| `--svg FILE` | The SVG file. |
| `--min-line-width-pt N` | The smallest permitted line width, in points. Default 0.5. |
| `--fonts-allowed LIST` | The permitted font families, separated by commas. Default: Caladea, Inter, IM Fell English, IM Fell English SC. |

The checks: `no_opacity`, `no_masks_filters_rgba`, `black_and_white`,
`min_line_width`, `all_text_is_text_elements`, `no_text_stroke`,
`font_families_allowed`, and `deterministic_ids`. A failed check gives exit
code 1.

The `no_text_stroke` check fails when a `<text>`, `<textPath>`, or `<tspan>`
has a stroke, set on it or inherited from a group. A PDF printer such as
headless Chrome turns stroked text into a Type 3 font.

The `min_line_width` check measures each line as it is printed. It applies
each transform and each view box, follows each `<use>` to its target, and
measures the lines of a pattern at the scale of the shape that it fills. A
root `<svg>` with a width in `mm`, `cm`, `in`, `pt`, or `pc` and a view box
maps the view box to that width. In another root, one user unit is 1 mm.

## `inkland version`

Print the version, the license, and the notice of the original program.

| Flag | Meaning |
|---|---|
| `--json` | Accepted for clarity. The output is always JSON. |
