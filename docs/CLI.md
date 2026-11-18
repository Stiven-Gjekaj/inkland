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
| `--preview FILE` | Also write a PNG preview (1024 px on the longer side). With more than one variant, each preview gets the variant name: `map-<variant>.png`. |
| `--variant NAME` | Render only this variant. |
| `--dry-run` | Check the config and the pins. Generate nothing and write nothing. The output is the dry-run report. |
| `--seed N` | Replace `seed`. |
| `--cells N` | Replace `world.cells`. It also removes `units.per_cell_hint`, so `N` sets the cell count. A `--set` of the hint applies after it and sets the hint again. |
| `--set POINTER=JSON` | Replace one value, for example `--set /world/cells=20000`. You can give it more than one time. It applies after `--seed` and `--cells`. |

Files in the output directory:

- `world.json`: the base world. All variants share it. It does not change
  when only overlays, variants, or render options change.
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

## `inkland export`

Export a world as GeoJSON, to stdout.

```bash
inkland export --world out/world.json --format geojson --layers coast,rivers,roads > map.geojson
```

| Flag | Meaning |
|---|---|
| `--world FILE` | The `world.json` of a generated world. |
| `--format geojson` | The format. Only `geojson`. |
| `--layers LIST` | The layers, separated by commas: `coast`, `rivers`, `regions`, `sea_routes`, `roads`, `places`, `markers`, `borders`. Default: all. |

The coordinates are config units of the frame with 3 decimals, not WGS84.
`crs` is `inkland:local`. Each feature has `layer`, an id, and, where they
apply, a pin id, a name, a kind, and a length. A place has `coa`, its coat
of arms.

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
`rename`, `brush`, and `remove_brush`. The output on stdout has `undo`: a
script that gives back the first file. The edited config must pass all
config checks, else nothing is written.

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
`min_line_width`, `all_text_is_text_elements`, `font_families_allowed`, and
`deterministic_ids`. A failed check gives exit code 1.

## `inkland version`

Print the version, the license, and the notice of the original program.

| Flag | Meaning |
|---|---|
| `--json` | Accepted for clarity. The output is always JSON. |
