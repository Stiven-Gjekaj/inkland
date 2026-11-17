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
| `TEMPLATE_INVALID` | 2 | 0.1.0 | A step of a custom heightmap template cannot be read. The message names the step. |
| `MAP_PARSE` | 2 | 0.1.0 | `inkland import` cannot read the map file: it is not text, gzip, or base64, or a record is missing or not valid. `details`: `record` (the record number, from 0), if a record is the cause. |
| `MAP_VERSION` | 2 | 0.1.0 | `inkland import` cannot read maps of this version. `details`: `found`, `supported`. |
| `PIN_OUTSIDE_FRAME` | 3 | 0.1.0 | A pin, a crop, or an overlay item is outside the frame. |
| `PIN_IN_FORCED_SEA` | 3 | 0.1.0 | A place is inside a `sea` or `lake` region. `details`: `region`. |
| `PIN_SAME_POSITION` | 3 | 0.1.0 | Two point pins have the same position. Each place needs its own cell. `details`: `other`. |
| `PORT_NOT_NEAR_COAST` | 3 | 0.1.0 | A port has no water next to it, and no channel of 8 cells or fewer can join it to water. `details`: `max_channel_cells`. |
| `REGION_CONFLICT` | 3 | 0.1.0 | Two regions overlap, but their height ranges have no common value. `details`: `other`. |
| `SEA_NOT_CONNECTED` | 3 | 0.1.0 | A `sea` region without `landlocked: true` does not connect to the ocean at a frame edge. |
| `RIVER_MOUTH_ON_LAND` | 3 | 0.1.0 | A point mouth is in a land region or on land that does not touch water, or a place mouth is not a port. |
| `RIVER_NOT_DOWNHILL` | 3 | 0.1.0 | The heights of the regions on a river course do not permit downhill flow, or no path joins the points of the river. |
| `RIVER_CROSSES_RIVER` | 3 | 0.1.0 | A pinned river cannot reach its next point without crossing another pinned river, in each order of planning. |
| `ROAD_TARGET_TOO_SHORT` | 3 | 0.1.0 | The road target, with its tolerance, is shorter than the straight lines through its points. `details`: `straight_length`. |
| `ROAD_NO_PATH` | 3 | 0.1.0 | No land path joins the points of a pinned road. |
| `ROAD_LENGTH_OUT_OF_TOLERANCE` | 1 | 0.1.0 | A road is written, but its length is outside the tolerance. The report gives this as the failed check `pinned_road_length`. |
| `IO_READ` | 4 | 0.1.0 | A file cannot be read. `details`: `file`. |
| `IO_WRITE` | 4 | 0.1.0 | A file cannot be written. |
| `NOT_IMPLEMENTED` | 70 | 0.1.0 | The command, or a config feature that `generate` must apply, is not in this version. The message names the milestone that adds it. Nothing is written. |
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
