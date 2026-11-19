# Errors, warnings, and exit codes

This file lists each code of the `inkland` command and of the library.
A code is never renamed and never used again for a different fault.
New codes go at the end of their group.

## Output

An error goes to stdout as JSON:

```json
{
  "error": { "code": "PIN_IN_FORCED_SEA", "message": "...",
             "path": "/pins/points/3/at", "details": { "region": "western-sea" } },
  "errors": [ ... ]
}
```

- `error` is the first error. `errors` holds all of them, in a fixed order.
- `path` is a JSON Pointer to the value in the effective config, or `""`.
- `details` holds more data. Its keys depend on the code.

## Exit codes

| Exit | Meaning |
|---|---|
| 0 | The command succeeded. |
| 1 | The output is written, but one or more checks failed. |
| 2 | The command line or the config is not valid. |
| 3 | The pins cannot be satisfied. |
| 4 | A file cannot be read or written. |
| 70 | A bug in inkland, or a command that this version does not have yet. |

With more than one error, the exit code comes from the most basic fault:
2 first, then 4, then 3, then 70, then 1.

## Error codes

| Code | Exit | Since | When |
|---|---|---|---|
| `CLI_USAGE` | 2 | 0.1.0 | The command line has an unknown command or flag, or a flag without its value. |
| `CONFIG_PARSE` | 2 | 0.1.0 | The config file is not valid JSON. `details`: `line`, `column`. |
| `CONFIG_SCHEMA` | 2 | 0.1.0 | The JSON does not agree with the schema (`inkland schema`). |
| `CONFIG_VERSION` | 2 | 0.1.0 | The config `version` is not supported. `details`: `supported`. |
| `CONFIG_SET_INVALID` | 2 | 0.1.0 | A `--set` flag is not `POINTER=JSON`, or its pointer cannot be set. |
| `FRAME_AXES_INVALID` | 2 | 0.1.0 | The frame axes do not agree with the origin, or the frame size is not more than 0. |
| `ID_INVALID` | 2 | 0.1.0 | An id does not match `^[a-z0-9]+(-[a-z0-9]+)*$`. |
| `ID_DUPLICATE` | 2 | 0.1.0 | Two items have the same id. `details`: `first` (pointer to the first item). |
| `REF_UNKNOWN` | 2 | 0.1.0 | A reference names an id, an overlay, or a variant that does not exist. |
| `SHAPE_INVALID` | 2 | 0.1.0 | A region or a feature does not have exactly one valid shape. |
| `REGION_POLYGON_INVALID` | 2 | 0.1.0 | A polygon has fewer than 3 points or crosses itself. |
| `FONT_NOT_ALLOWED` | 2 | 0.1.0 | A font family is not in `render.fonts_allowed`. `details`: `allowed`. |
| `RIVER_MOUTH_REGION_KIND` | 2 | 0.1.0 | A river mouth region is not of kind `marsh`, `lake`, or `sea`. |
| `VALUE_OUT_OF_RANGE` | 2 | 0.1.0 | A value is outside its permitted range. |
| `TEMPLATE_INVALID` | 2 | 0.1.0 | A step of a custom heightmap template cannot be read, a raster template does not have `cols` x `rows` values, or the file of an image template is not a PNG file of 2 to 8192 pixels on each side. The message names the step. |
| `MAP_PARSE` | 2 | 0.1.0 | `inkland import` cannot read the map file: it is not text, gzip, or base64, or a record is missing or not valid. `details`: `record` (the record number, from 0), if a record is the cause. |
| `MAP_VERSION` | 2 | 0.1.0 | `inkland import` cannot read maps of this version. `details`: `found`, `supported`. |
| `WORLD_PARSE` | 2 | 0.1.0 | A world file is not a `world.json` of inkland, it has a format version that this version cannot read, or its config does not agree with its `world_hash`. |
| `EDIT_SCRIPT_INVALID` | 2 | 0.1.0 | An edit script is not valid JSON, has a version other than 1, or has a command that `inkland edit` does not know. `details`: `line`, `column` for a JSON error. |
| `COA_INVALID` | 2 | 0.1.0 | A coat of arms in the config (`coa` or `state_coa` of a point pin) names a tincture, a shield, a division, an ordinary, a line, a charge, or a position that does not exist, or `state_coa` is on a place without `capital_of`. The path points to the wrong value. |
| `TABLE_INVALID` | 2 | 0.1.0 | A good, a military unit, or a transport in `world.goods`, `world.military_units`, or `world.transports` cannot be used: a spread expression that cannot be read, an unknown need or culture type, a new unit without `kind`, a new transport without `speed` and `domain`, or a list of transports without a transport of the domain `stay`. The path points to the wrong value. |
| `RENAME_INVALID` | 2 | 0.1.0 | A rename in `world.renames` has both or none of `at` and `pin`, an empty name, a `full_name` or a `coa` for a culture or a religion, or a point where no area of its kind is. The path points to the entry or to the wrong value. |
| `ZONE_INVALID` | 2 | 0.1.0 | A zone in `world.zones` has more than one shape, a shape and `at`, neither, `remove` without `at`, a new zone without `name` and `kind`, a shape that is not valid or holds no cell, or a point where no generated zone is. The path points to the entry. |
| `FINANCE_INVALID` | 2 | 0.1.0 | An entry of `world.finance` has both or none of `at` and `pin`, a point where no state is, or a tariff rate that is not a number or `embargo`. The path points to the entry or to the rate. |
| `MARKET_INVALID` | 3 | 0.1.0 | An entry of `world.markets` adds a market at a place that is a market center already, moves or removes the market of a place that is not a center, moves a market to a place with a market, or has both `remove` and `to`. The path points to the entry. |
| `REGIMENT_INVALID` | 3 | 0.1.0 | An entry of `world.regiments` has more than one of `add`, `split`, and `remove`, a new regiment without units or outside a state, an unknown unit, a negative number of troops, a regiment too small to split, or no regiment to change. The path points to the entry. |
| `JOURNEY_INVALID` | 3 | 0.1.0 | An entry of `world.journeys` removes a journey that does not exist or gives no `id`, adds a journey with fewer than 2 stops or an unknown transport, or a leg has no valid path for the transport. The path points to the entry. |
| `EDIT_OUTSIDE_WORLD` | 2 | 0.1.0 | `inkland edit --world` has a `set` or `unset` of `/overlays`, `/variants`, `/render`, or `/font_dirs`. A `world.json` keeps only the part of the config that makes the world, so the edit has no target. Replay the script with `--config`. The path is the pointer of the command. `details`: `command` (the index of the command, from 0). |
| `PIN_OUTSIDE_FRAME` | 3 | 0.1.0 | A pin, a crop, or an overlay item is outside the frame. |
| `PIN_IN_FORCED_SEA` | 3 | 0.1.0 | A place is inside a `sea` or `lake` region, and no land region with a higher `priority` holds it. A place with `island: true` does not give this error. `details`: `region`. |
| `PIN_SAME_POSITION` | 3 | 0.1.0 | Two point pins have the same position. Each place needs its own cell. `details`: `other`. |
| `PORT_NOT_NEAR_COAST` | 3 | 0.1.0 | A port has no water next to it, and no channel of 8 cells or fewer can join it to water. `details`: `max_channel_cells`. |
| `REGION_CONFLICT` | 3 | 0.1.0 | Two regions overlap, but their height ranges have no common value. `details`: `other`. |
| `SEA_NOT_CONNECTED` | 3 | 0.1.0 | A `sea` region without `landlocked: true` has water that does not connect to the ocean at a frame edge. Land in the region is shore. `details.at` gives the cut-off water. |
| `RIVER_MOUTH_ON_LAND` | 3 | 0.1.0 | A point mouth is in a land region or on land that does not touch water, or a place mouth is not a port. |
| `RIVER_NOT_DOWNHILL` | 3 | 0.1.0 | The heights of the regions on a river course do not permit downhill flow, or no path joins the points of the river. |
| `RIVER_CROSSES_RIVER` | 3 | 0.1.0 | A pinned river cannot reach its next point without crossing another pinned river, in each order of planning. |
| `ROAD_TARGET_TOO_SHORT` | 3 | 0.1.0 | The road target, with its tolerance, is shorter than the straight lines through its points. `details`: `straight_length`. |
| `ROAD_NO_PATH` | 3 | 0.1.0 | No land path joins the points of a pinned road. |
| `ROAD_LENGTH_OUT_OF_TOLERANCE` | 1 | 0.1.0 | A road is written, but its length is outside the tolerance. The report gives this as the failed check `pinned_road_length`. |
| `IO_READ` | 4 | 0.1.0 | A file cannot be read, for example the config or the image of an image template. `details`: `file`. |
| `IO_WRITE` | 4 | 0.1.0 | A file cannot be written. |
| `NOT_IMPLEMENTED` | 70 | 0.1.0 | The command, or a config feature that `generate` must apply, is not in this version. No command gives it in 0.1.0; the code stays reserved. Nothing is written. |
| `INTERNAL` | 70 | 0.1.0 | A bug in inkland. |

"(Milestone N.)" means that the code exists, but no check gives it before
that milestone.

## Warning codes

A warning goes in the `warnings` list of the report. It does not change the
exit code.

| Code | Since | When |
|---|---|---|
| `FONT_NOT_FOUND` | 0.1.0 | A font family is not available for the PNG preview. |
| `LABEL_UNPLACED` | 0.1.0 | A label cannot go on the map without an overlap. |
