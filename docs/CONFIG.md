# Config reference

This file is made from the JSON Schema of the config. Do not edit it.
To make it again, run:

```bash
cargo run -p inkland-cli -- schema --markdown > docs/CONFIG.md
```

`inkland schema` prints the JSON Schema itself. All lengths are in
config units (`units.name`), unless a field says mm or points. Heights
are on the scale 0 to 100.

## Top level

The config of one map.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `font_dirs` | array of string |  | `[]` | More directories with font files for the PNG preview. The preview also has the free fonts of the style presets built in, and it uses the fonts of the system. The SVG never embeds a font. |
| `frame` | [`Frame`](#frame) | yes |  | The size and the axes of the world. |
| `overlays` | map of name to [`Overlay`](#overlay) |  |  | Named overlays. A variant shows the base world and one overlay. |
| `pins` | [`Pins`](#pins) |  |  | Hard constraints. Generation fits around them and never moves them. |
| `render` | [`Render`](#render) |  |  | Render options for all variants. |
| `seed` | integer | yes |  | Master seed. Each layer of generation gets its own seed from it. |
| `units` | [`Units`](#units) |  |  | The distance unit of all coordinates and lengths. |
| `variants` | array of [`Variant`](#variant) |  | see the type | The maps to render. Default: one variant `default`, full frame, on a 167 x 250 mm page. |
| `version` | integer | yes |  | Config format version. Must be 1. |
| `world` | [`World`](#world) |  |  | Generation options. |

## Types

### Anchor

A position: a point, a pinned place, or a pinned region.

One of:

- array of 2 number: A point `[x, y]`, in units.
- [`PlaceRef`](#placeref): A pinned place.
- [`RegionRef`](#regionref): A pinned region.

### AreaKind

The kind of a generated area that a rename names.

Type: `"state"`, `"culture"`, `"religion"`, `"province"`

### AreaPin

A soft area with a character of the land. Give exactly one of `polygon`, `box`, and `circle`. The outline is loose: fractal noise moves it, and the effect fades out over the `edge` outside it. An area changes only land: the coast stays where it is. A region pin rules over an area.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `box` | array of 4 number |  |  | Rectangle `[x0, y0, x1, y1]`, in units. |
| `character` | [`Character`](#character) | yes |  | The character of the land in the area. |
| `circle` | [`Circle`](#circle) |  |  | Circle, in units. |
| `edge` | number |  |  | Width of the soft edge outside the outline, in units. The effect fades out over this width. Default: a fifth of the size of the shape (the square root of the area of its bounding box). |
| `id` | string | yes |  | Identifier: lower-case letters and digits, words joined by `-`. Unique across all pins. |
| `polygon` | array of array of 2 number |  |  | Outline `[[x, y], ...]`, in units. 3 or more points. The outline must not cross itself. |
| `strength` | number |  |  | How strong the character is. 0 to 1. Default 1. |

### AutoKeyword

The keyword `auto`.

Type: `"auto"`

### Brush

One heightmap brush stroke: it adds `delta` at its centre, and less with distance, to 0 at `radius`.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `at` | array of 2 number | yes |  | Centre `[x, y]`, in units. |
| `delta` | number | yes |  | Height change at the centre, on the 0 to 100 scale. -100 to 100. |
| `radius` | number | yes |  | Radius, in units. More than 0. |

### BudgetEntry

The budget of a state, in percents of the taxes collected. A value that the entry does not give keeps the generated value.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `administration` | number |  |  | The administration, in percent. 0 or more. |
| `corruption` | number |  |  | The percent of each expense that officials pocket. 0 to 100. |
| `court` | number |  |  | The court, in percent. 0 or more. |
| `infrastructure` | number |  |  | The infrastructure, in percent. 0 or more. |
| `military` | number |  |  | The military, in percent. 0 or more. |

### Cartouche

A title cartouche.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `title` | string | yes |  | The title text. |

### Character

The character of the land in an area.

One of:

- `"rough"`: Broken, high land: the heights rise, with sharp ridges. The relief shows mountain chains and hills.
- `"wooded"`: Forest: the land keeps its heights, and the biome becomes forest.
- `"rolling"`: Rolling land: low, rounded hills.
- `"flat"`: Flat land: the heights go down to low land, with few hills.

### Charge

A charge: a figure at one or more positions.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `charge` | string | yes |  | The charge, for example `mullet`, `crossHummetty`, or `inescutcheon`. |
| `divided` | [`Divided`](#divided) |  |  | With a division: as `divided` of an ordinary. |
| `p` | string | yes |  | The positions on the shield: one letter for each copy. `a` to `i` are a grid of 3 x 3 from the top left, `e` is the center, `j` to `q` are smaller grids, `y` is the canton, `z` the base, and `A` to `L` go around the border. |
| `reversed` | [`Flag`](#flag) |  |  | `1` or `true`: upside down. |
| `sinister` | [`Flag`](#flag) |  |  | `1` or `true`: turned to face the other side. |
| `size` | number |  |  | The size. Default: 1. |
| `t` | string | yes |  | The tincture. |
| `t2` | string |  |  | The second tincture of a charge with more colours. |
| `t3` | string |  |  | The third tincture of a charge with more colours. |

### Circle

A circle.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `at` | array of 2 number | yes |  | Center `[x, y]`, in units. |
| `r` | number | yes |  | Radius, in units. |

### Climate

Climate options.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `geography` | [`Geography`](#geography) |  |  | The size and the position of the map on the globe, as in FMG. If it is given, it sets the latitudes, and `latitude_range` has no effect. A value that it does not give comes from a random draw. |
| `latitude_range` | array of 2 number |  | `[30.0,55.0]` | Latitude of the south edge and of the north edge, in degrees. |
| `wind` | array of 6 number |  |  | Winds, one for each 30 degree latitude band from 90 north to 90 south: the direction to which the wind blows, in degrees clockwise from north. Default: `[225, 45, 225, 315, 135, 315]`. |

### Coa

A coat of arms, in the JSON shape of Azgaar's Fantasy Map Generator.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `charges` | array of [`Charge`](#charge) |  |  | The charges, from bottom to top. |
| `division` | [`Division`](#division) |  |  | The division of the field. |
| `ordinaries` | array of [`Ordinary`](#ordinary) |  |  | The ordinaries, from bottom to top. |
| `shield` | string |  |  | The shield shape, for example `heater`, `spanish`, or `round`. Default: `heater`. |
| `t1` | string | yes |  | The tincture of the field: a metal (`argent`, `or`), a colour (`gules`, `azure`, `sable`, `purpure`, `vert`, ...), a stain (`murrey`, `sanguine`, `tenné`), or a pattern `<pattern>-<tincture>-<tincture>[-<size>]`, for example `vair-azure-argent` or `semy_of_mullet-or-azure-small`. |

### CoastlineDetail

Coastline detail.

One of:

- `"smooth"`: Cut the corners of the cell edges.
- `"fractal"`: Add the fractal detail of Azgaar's Fantasy Map Generator.

### Divided

The part of a division that an ordinary or a charge is drawn on.

Type: `"field"`, `"division"`, `"counter"`

### Division

The division of the field.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `division` | string | yes |  | The division, for example `perPale`, `perFess`, or `perCross`. |
| `line` | string |  |  | The line of partition, for example `straight`, `wavy`, or `embattled`. Default: `straight`. |
| `t` | string | yes |  | The tincture of the second part. |

### EdgeKind

The terrain along a frame edge.

One of:

- `"sea"`: Open sea: the band is water, deeper at the edge.
- `"land"`: Land that goes on past the frame: the water of the band becomes low land.

### EmblemOptions

Options of the layer `emblems`: the coats of arms of the states, the provinces, and the places.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `hatching` | boolean |  | `false` | `true`: in `print-safe` mode, draw the emblems in black and white, with the heraldic hatching of Petra Sancta for each tincture. Each line is at least `min_line_width_pt` wide. `false`: `print-safe` mode draws no emblems. `colour` mode always draws the colours. |
| `show_all` | boolean |  | `false` | `true`: draw each group of emblems. `false`: as Azgaar's Fantasy Map Generator, hide a group when its emblems are smaller than 25 or larger than 300 map pixels at the zoom of the variant. Then a full map shows only the arms of the states. |

### Extras

Extras. A missing value means "not set".

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `cartouche` | [`Cartouche`](#cartouche) |  |  | Show a title cartouche. |
| `compass` | boolean |  |  | Show a compass. |
| `grid` | [`Grid`](#grid) |  |  | Show a grid. |
| `scale_bar` | boolean |  |  | Show a scale bar in config units. |

### Feature

A feature that an overlay adds. Give exactly one of `at`, `polyline`, and `polygon`.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `at` | array of 2 number |  |  | Position `[x, y]`, in units. |
| `id` | string | yes |  | Identifier: lower-case letters and digits, words joined by `-`. |
| `kind` | [`FeatureKind`](#featurekind) | yes |  | Kind of feature. |
| `name` | string |  |  | Name of the feature. |
| `polygon` | array of array of 2 number |  |  | Outline `[[x, y], ...]`, in units. |
| `polyline` | array of array of 2 number |  |  | Line `[[x, y], ...]`, in units. |

### FeatureKind

Kinds of overlay features.

Type: `"ruin"`, `"battle-site"`, `"border"`, `"wall"`, `"custom"`

### FinanceEntry

A change to the finance of a state. The entry finds its state by the point `at` or by the point pin `pin`. Give exactly one.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `at` | array of 2 number |  |  | A point `[x, y]` in the state. |
| `budget` | [`BudgetEntry`](#budgetentry) |  |  | The budget. |
| `market_fee` | number |  |  | The license fee a day from each market of the state. 0 or more. |
| `pin` | string |  |  | The id of a point pin in the state. |
| `poll_tax` | number |  |  | The poll tax a day for each population point. 0 or more. |
| `sales_tax` | number |  |  | The sales tax, a share of the price. 0 to 1. |
| `tariffs` | array of [`TariffEntry`](#tariffentry) |  |  | The tariffs. A list replaces the generated tariffs. |

### Flag

A flag of FMG: a boolean, or the number 0 or 1.

One of:

- boolean
- integer: 0 or 1.

### FontSizes

Font sizes, in points.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `other` | number |  | `7.0` | Scale bar, compass, and grid text. Default 7. |
| `place_city` | number |  | `8.0` | Cities and capitals. Default 8. |
| `place_town` | number |  | `7.0` | Towns, fortresses, ruins, and custom places. Default 7. |
| `place_village` | number |  | `6.0` | Villages. Default 6. |
| `region` | number |  | `8.5` | Names of pinned regions, for example seas and mountains. Default 8.5. |
| `river` | number |  | `6.5` | River names. Default 6.5. |
| `state` | number |  | `10.0` | State names. Default 10. |
| `title` | number |  | `12.0` | The map title in the cartouche. Default 12. |

### Fonts

Font families.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `labels` | string |  | `"IM Fell English"` | Family of the map labels. |
| `labels_sc` | string |  | `"IM Fell English SC"` | Small-capital family of the map labels, for example region names. |
| `other` | string |  | `"Caladea"` | Family of all other text: scale bar, compass letters, grid, notes. |
| `title` | string |  | `"IM Fell English"` | Family of the map title. |

### Frame

The frame of the world: its size and the direction of its axes.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `height` | number | yes |  | Height of the world, in units. Must be more than 0. |
| `origin` | [`Origin`](#origin) |  | `"top-left"` | The corner where x = 0 and y = 0. |
| `width` | number | yes |  | Width of the world, in units. Must be more than 0. |
| `x_axis` | [`XAxis`](#xaxis) |  | `"east"` | The direction in which x increases. Only `east`. |
| `y_axis` | [`YAxis`](#yaxis) |  | `"south"` | The direction in which y increases: `south` with origin `top-left`, `north` with origin `bottom-left`. |

### FrameEdges

Land or sea along the frame edges. The compass sides follow the axes of the frame.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `east` | [`EdgeKind`](#edgekind) |  |  | The terrain along the east edge. |
| `north` | [`EdgeKind`](#edgekind) |  |  | The terrain along the north edge. |
| `south` | [`EdgeKind`](#edgekind) |  |  | The terrain along the south edge. |
| `west` | [`EdgeKind`](#edgekind) |  |  | The terrain along the west edge. |
| `width` | number |  |  | The mean width of the band along an edge, in units. Fractal noise makes the inner side of the band a natural coast. More than 0. Default: a tenth of the smaller side of the frame. |

### Geography

The size and the position of the map on the globe.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `latitude` | number |  |  | The north-south position in percent. 0 to 100. 0 puts the north edge at the north pole, and 100 puts the south edge at the south pole. |
| `longitude` | number |  |  | The west-east position in percent. 0 to 100. |
| `map_size` | number |  |  | The map size in percent of the world. 1 to 100. 100 is the full globe from pole to pole. |

### GoodEntry

A change to the goods catalogue. An entry with the name of a good in the list changes the values that it gives, or removes the good with `remove`. An entry with a new name adds a good at the end of the list. The good ids follow the order of the list that results.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `biome_output` | object |  |  | Units for each rural population point, by biome id. |
| `carriage` | number |  |  | The price to carry one unit over a decima. 0 or more. |
| `clustering` | number |  |  | The chance that a deposit lands next to the one before it. 0 to 1. |
| `color` | string |  |  | The color, as `#rrggbb`. |
| `coverage` | map of name to number |  |  | How much of each need one unit covers: `food`, `utilities`, `construction`, `military`, `luxury`. |
| `culture_type` | map of name to number |  |  | The production factors by culture type, for example `Hunting`. |
| `deposit_yield` | number |  |  | The multiplier on the output of each deposit. 0 or more. |
| `deposits` | array of 2 number |  |  | The least and the most deposits on the map. |
| `development` | integer |  |  | The least development that a cell needs to make the good. 0 to 100. |
| `distribution` | string |  |  | The spread expression of the deposits, for example `biome(5, 6) && minHeight(30)`. An empty text gives no deposits. |
| `icon` | string |  |  | The icon id, for example `goods-wood`. |
| `name` | string | yes |  | The name of the good, for example `Wood`. |
| `recipes` | array of map of name to number |  |  | Alternative input sets: the amount of each good, by good name. Each recipe makes one unit. |
| `remove` | boolean |  |  | Remove the good from the list. |
| `tags` | array of string |  |  | Tags, for example `construction`, `fuel`, `ore`, `luxury`. |
| `unit` | string |  |  | The unit name, for example `pile`. |
| `value` | number |  |  | The base price. 0 or more. |

### Grid

A grid.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `step` | number | yes |  | Distance between grid lines, in units. |

### JourneyEntry

A change to the journeys. With `remove`, the entry removes the journey `id`. Else it adds a journey through its stops, in order: one segment for each pair of stops, with one transport.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `id` | integer |  |  | The journey to remove. |
| `name` | string |  |  | The name. Default: the first and the last stop, with an arrow. |
| `remove` | boolean |  |  | Remove the journey `id`. |
| `stops` | array of array of 2 number |  |  | The stops `[[x, y], ...]`, in units: the cell at each point. 2 or more. |
| `transport` | string |  |  | The name of the transport. Default: the first land transport. |

### LabelOptions

Label options.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `at` | array of 2 number |  |  | The middle of the label `[x, y]`, in units. The label goes here, level, before the other labels, which keep clear of it. Default: the placer finds a free place. |
| `hide` | boolean |  | `false` | `true`: do not show the label. |
| `size_pt` | number |  |  | Font size, in points. Default: the size of the label kind. |

### LabelRules

Rules for the labels. A name is "generated" when the config does not write it: the name of a pin, of a region, of a river, the `capital_of` of a pin, a rename, or a name of an overlay. Generation gives the other names to rivers, lakes, seas, islands, states, provinces, and places.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `check_generated` | boolean |  | `false` | `true`: the report of each variant has the check `no_generated_labels`. It fails when the variant shows a label with a generated name. Default `false`. |
| `curved` | boolean |  | `true` | `true`: an area label can follow a smooth curve along the middle of its area, with its letters spaced to fill part of the curve. `false`: only straight area labels. Default `true`. |
| `generated` | boolean |  | `true` | `false`: show no label with a generated name. Default `true`. |
| `halo` | boolean |  | `true` | `print-safe` mode: `true` draws a white box behind each label that does not follow a river or a line, so that the lines under it do not cross the text. A curved area label gets boxes along its curve. `false`: no box. The text never has a stroke, because a PDF printer turns stroked text into a Type 3 font. `colour` mode draws the halo of its style. Default `true`. |
| `max_angle` | number |  | `90.0` | The largest turn of an area label (a state, a region, a lake, a sea, an island, or an overlay feature), in degrees, in both render modes. A straight label turns 0 or 90 degrees. A curved label has no part at a larger angle to the level than this. `0` gives only level, straight labels. Default 90. |
| `unlabelled_symbols` | boolean |  | `true` | `false`: a place whose label the config leaves out has no symbol either. The config leaves out a label with a generated name when `generated` is `false`, and a label with `hide` in the label options of an overlay. A label that has no free place keeps its symbol. Default `true`. |

### Lattice

The lattice of the points of the first grid.

One of:

- `"square"`: Rows and columns of squares.
- `"hex-pointy"`: Hexagons with a point at the top. Odd rows move half a step.
- `"hex-flat"`: Hexagons with a flat top.
- `"diamond"`: Squares turned 45 degrees.
- `"triangle"`: Triangles that point up and down in turn.

### LayerName

Names of the map layers.

One of:

- `"coast"`, `"relief"`, `"rivers"`, `"regions"`, `"roads"`, `"places"`, `"labels"`
- `"features"`: The features that an overlay adds.
- `"ocean"`: The ocean: the water colour and the depth bands. In `print-safe` mode, the water lines along the coast.
- `"landmass"`: The land colour. Only `colour` mode.
- `"texture"`: A bitmap texture. The renderer has no bitmaps, so it draws nothing.
- `"heightmap"`: Height shading. Only `colour` mode.
- `"lakes"`: Lakes. Only `colour` mode.
- `"biomes"`: Biomes. Only `colour` mode.
- `"cells"`: The outlines of the cells.
- `"grid"`: A grid of hexagons or squares over the map.
- `"coordinates"`: Lines of latitude and longitude, with labels.
- `"compass"`: A compass rose.
- `"religions"`: Religions. Only `colour` mode.
- `"cultures"`: Cultures. Only `colour` mode.
- `"states"`: The areas of the states. Only `colour` mode.
- `"provinces"`: The areas of the provinces. Only `colour` mode.
- `"zones"`: Zones. Only `colour` mode. Draws nothing when the world has no zones.
- `"borders"`: The borders of states and provinces.
- `"routes"`: Roads and trails.
- `"temperature"`: Temperature. Only `colour` mode.
- `"coastline"`: The coastline.
- `"ice"`: Glaciers and icebergs. Only `colour` mode. Draws nothing when the world has no ice.
- `"goods"`: Goods: the output of the cells, the pies of the places, and the deposit icons (FMG `goods`). Only `colour` mode. Draws nothing when the world has no economy.
- `"markets"`: The markets: the area of each market and a pin at its center (FMG `markets`). Only `colour` mode. Draws nothing when the world has no markets.
- `"trade"`: Trade: the goods that each route link carried in the run, as lines with a width by the units. FMG animates the trade; inkland draws its traffic lines (FMG `trade`). Only `colour` mode. Draws nothing when no goods travel.
- `"precipitation"`: Precipitation. Only `colour` mode.
- `"population"`: Population. Only `colour` mode. Draws nothing when the world has no population.
- `"emblems"`: Coats of arms. Only `colour` mode. Draws nothing when the world has no emblems.
- `"burg-icons"`: The symbols of the places.
- `"military"`: The regiments of the states: a box in the colour of the state with the troops and the icon (FMG `military`). Only `colour` mode. Draws nothing when the world has no regiments.
- `"markers"`: Markers. Only `colour` mode. Draws nothing when the world has no markers.
- `"journeys"`: Journeys: the path of each journey, a curve for each segment in the colour of the journey (FMG `journeys`). Only `colour` mode. Draws nothing when the world has no journeys.
- `"rulers"`: Rulers: the default ruler of FMG across the largest landmass, with its length (FMG `rulers`). Only `colour` mode. Draws nothing when the world has no measurers.
- `"scale-bar"`: A scale bar.
- `"vignette"`: A dark edge around the map. Only `colour` mode.
- `"legend"`: A legend box. Only `colour` mode.

### LegendEntry

A legend box.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `at` | array of 2 number |  |  | The top left corner of the box `[x, y]`, in units. Default: the next free place in the bottom right of the map. |
| `items` | array of [`LegendItemEntry`](#legenditementry) |  | `[]` | The items, in order. |
| `name` | string | yes |  | The title of the box. |

### LegendItemEntry

One item of a legend box.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `color` | string | yes |  | The color of the swatch, as `#rrggbb`. |
| `text` | string | yes |  | The text of the item. |

### MarketEntry

A change to the market centers. `at` names the place nearest to the point. Without `remove` and `to`, the entry adds a market with this place as its center. With `remove`, it removes the market of this center. With `to`, it moves the market of this center to the place nearest to `to`.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `at` | array of 2 number | yes |  | A point `[x, y]`: the place nearest to it. |
| `remove` | boolean |  |  | Remove the market of the place. |
| `to` | array of 2 number |  |  | The new center: the place nearest to this point. |

### MeasurerEntry

A measurer: its kind and its points `[x, y]`, in units.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `kind` | [`MeasurerKind`](#measurerkind) | yes |  | The kind: `ruler`, `opisometer`, `route-opisometer`, or `planimeter`. |
| `points` | array of array of 2 number | yes |  | 2 or more points. A planimeter closes its points into an area. |

### MeasurerKind

The kind of a measurer (FMG `MeasurerType`).

Type: `"ruler"`, `"opisometer"`, `"route-opisometer"`, `"planimeter"`

### NoteEntry

A note of the map.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `name` | string | yes |  | The title of the note. |
| `note` | string |  | `""` | The text of the note. |

### Ordinary

An ordinary: a band, a cross, a border, and so on.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `divided` | [`Divided`](#divided) |  |  | With a division: `field` draws it only on the field, `division` only on the second part, `counter` in the tincture of the other part. |
| `line` | string |  |  | The line of its edges. Default: `straight`. |
| `ordinary` | string | yes |  | The ordinary, for example `pale`, `fess`, `bend`, `chief`, `cross`, `bordure`, or `canton`. |
| `t` | string | yes |  | The tincture. |

### Origin

The corner of the origin.

Type: `"top-left"`, `"bottom-left"`

### Overlay

An overlay: what one variant adds to, removes from, or renames in the base world. An overlay cannot change terrain, rivers, or coasts.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `add` | [`OverlayAdd`](#overlayadd) |  |  | Items that only this overlay shows. |
| `labels` | map of name to [`LabelOptions`](#labeloptions) |  |  | Label options, from `"<kind>:<id>"` to the options. The key of a state label is `"state:<id>"`, with the id of the point pin that is the capital of the state, for example `"state:solmere"`. |
| `remove` | array of string |  | `[]` | Items to hide, as `"<kind>:<id>"`, for example `"place:ostrey"`. |
| `rename` | map of name to string |  |  | New names, from `"<kind>:<id>"` to the name. |

### OverlayAdd

Items that an overlay adds.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `features` | array of [`Feature`](#feature) |  | `[]` | Features such as ruins, battle sites, and old borders. |
| `places` | array of [`PointPin`](#pointpin) |  | `[]` | Places. They do not change the terrain. |
| `roads` | array of [`RoadPin`](#roadpin) |  | `[]` | Roads, routed on the shared terrain. |

### Page

A page size.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `h_mm` | number | yes |  | Height, in millimetres. |
| `margin_mm` | number |  | `0.0` | Margin on all sides, in millimetres. |
| `w_mm` | number | yes |  | Width, in millimetres. |

### Pins

Hard constraints.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `areas` | array of [`AreaPin`](#areapin) |  |  | Soft areas with a character of the land, such as rough or wooded. An area has no fixed height range. It steers the heights, the biomes, and so the relief symbols, and fades out at its loose edge. |
| `points` | array of [`PointPin`](#pointpin) |  | `[]` | Places at exact coordinates. |
| `regions` | array of [`RegionPin`](#regionpin) |  | `[]` | Areas with a forced kind of terrain. |
| `rivers` | array of [`RiverPin`](#riverpin) |  | `[]` | Rivers with a fixed source, course, and mouth. |
| `roads` | array of [`RoadPin`](#roadpin) |  | `[]` | Roads between pinned places, with an optional target length. |

### PlaceKind

Kinds of places.

Type: `"city"`, `"town"`, `"village"`, `"ruin"`, `"fortress"`, `"custom"`

### PlaceOptions

Options for places that generation adds.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `min_distance_from_pins` | number |  |  | Smallest distance from a generated place to a pinned place, in units. Default: three times the mean cell width. |

### PlaceRef

A reference to a pinned place.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `place` | string | yes |  | The id of a point pin. |

### PointPin

A place at exact coordinates.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `at` | array of 2 number | yes |  | Position `[x, y]`, in units. The place is put here exactly. |
| `capital_of` | string |  |  | Name of a state. The state forms around this place, its capital. A later pin with the same name is a seat of the state: the state also grows from it, for example on the other side of a sea. |
| `claim` | array of array of 2 number |  |  | A soft claim of the state of this capital (`capital_of`): a polygon of 3 or more points `[x, y]`, in units. The state grows into it first, and the other states take its cells only at a high cost. The claims of a capital and its seats add together. Default: no claim. |
| `coa` | [`Coa`](#coa) |  |  | The coat of arms of this place, in the JSON shape of Azgaar's Fantasy Map Generator. It replaces the generated arms, and the generated arms of a province with this place as its seat take from it. Without `shield`, the arms get the generated shield shape. An overlay place does not use it. Default: generated arms. |
| `custom_kind` | string |  |  | Name of the kind. Required when `kind` is `custom`. |
| `expansionism` | number |  |  | How far the state of this capital (`capital_of`) grows, 0.1 to 10. It divides the cost of each cell that the state takes, so a bigger value gives a bigger state. It replaces the random expansionism of `world.size_variety`. On a seat, it counts only when the capital gives none. Default: random. |
| `id` | string | yes |  | Identifier: lower-case letters and digits, words joined by `-`. Unique across all pins. |
| `island` | boolean |  |  | `true`: the place can be inside a `sea` or `lake` region. The cell of the place becomes a small island of land. Only for `pins.points`. For a larger island, put a land region with a higher `priority` than the sea around the place. |
| `kind` | [`PlaceKind`](#placekind) | yes |  | Kind of place. |
| `name` | string |  |  | Name of the place. Default: a generated name. |
| `port` | boolean |  | `false` | `true`: the cell of the place touches the sea or a lake. |
| `state_coa` | [`Coa`](#coa) |  |  | The coat of arms of the state that this place is the capital of (`capital_of`), in the same shape as `coa`. The generated arms of the places and provinces of the state take from it. Default: generated arms. |

### Raster

A raster of heights that covers the full frame. The raster divides the frame into `cols` x `rows` equal rectangles. Each value is the height of one rectangle.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `cols` | integer | yes |  | Number of columns, from the low x edge to the high x edge. 2 or more. |
| `rows` | integer | yes |  | Number of rows, from the low y edge to the high y edge. 2 or more. |
| `values` | array of integer | yes |  | The heights on the scale 0 to 100, row by row. The first row is at the low y edge of the frame. In each row, the first value is at the low x edge. The list has `cols` x `rows` values. |

### RegimentEntry

A change to the regiments. With `add`, the entry adds a regiment of the state that holds `at`, at that point; it must give `units`. Else it changes the regiment nearest to `at`: its name, units, or fleet flag, moves it with `to`, splits it in two halves with `split`, or removes it with `remove`.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `add` | boolean |  |  | Add a new regiment. |
| `at` | array of 2 number | yes |  | A point `[x, y]`, in units. |
| `name` | string |  |  | The name. Default for a new regiment: the name that FMG gives. |
| `naval` | boolean |  |  | `true`: a fleet. |
| `remove` | boolean |  |  | Remove the regiment. |
| `split` | boolean |  |  | Split the regiment in two halves. The new one stands below. |
| `to` | array of 2 number |  |  | Move the regiment to this point `[x, y]`. Its base stays. |
| `units` | map of name to number |  |  | The troops of each unit, in people, by unit name. A unit that the entry does not name keeps its troops. |

### RegionKind

Kinds of forced terrain.

One of:

- `"sea"`, `"lake"`, `"mountains"`, `"hills"`, `"plains"`, `"marsh"`, `"forest"`, `"desert"`
- `"land"`: Land of any height. Without a `height`, the land of the template keeps its height, and the water of the template becomes low land: deeper water gives a little higher land. Use it to draw a coast on a template with much sea.

### RegionPin

An area with a forced kind of terrain. Give exactly one of `polygon`, `box`, and `circle`.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `box` | array of 4 number |  |  | Rectangle `[x0, y0, x1, y1]`, in units. |
| `circle` | [`Circle`](#circle) |  |  | Circle, in units. |
| `coast_detail` | integer |  |  | The number of noise layers of a rough outline. 1 to 8. Each layer adds bays and capes of half the size of the layer before it. 1 gives a smooth, wavy coast. 8 gives a very detailed coast. Has an effect only with a `roughness`. Default 4. |
| `edge` | number |  | `0.0` | Width of the soft edge outside the shape, in units. Heights blend from the forced range to the free terrain over this width. Inside the shape, the heights are always in the range. |
| `height` | array of 2 number |  |  | Height range `[min, max]` on the height scale 0 to 100 (sea level 20). Default: the range of the kind. |
| `id` | string | yes |  | Identifier: lower-case letters and digits, words joined by `-`. Unique across all pins. |
| `islets` | number |  |  | Not for `sea` and `lake`. How many small islands lie off a rough coast. 0 to 1. Most of them lie off the capes. 0: none. Has an effect only with a roughness. Default: 0.3 for `land`, 0 for the other kinds. |
| `kind` | [`RegionKind`](#regionkind) | yes |  | Kind of terrain in the area. |
| `landlocked` | boolean |  | `false` | Only for `sea` and `lake`. `false`: a `sea` must connect to the ocean at a frame edge. `true`: an inland sea or a large lake. |
| `name` | string |  |  | Name of the area, for its label. |
| `polygon` | array of array of 2 number |  |  | Outline `[[x, y], ...]`, in units. 3 or more points. The outline must not cross itself. |
| `priority` | integer |  |  | Where regions overlap, only the regions with the highest priority set the height range of a cell. Regions with the same priority share it: the cell gets a height in all of their ranges. Give an inner region a higher priority than the region around it, for example hills in a lowland. Default 0. |
| `roughness` | number |  |  | The largest distance, in units, by which fractal noise moves the outline in or out. The coast becomes natural, with bays and capes. The largest bays are about 4 times this size. A side of the outline on a frame edge does not move. 0: the outline is exact. Default: 0 for `sea` and `lake`; for the other kinds, 3 percent of the size of the shape (the square root of the area of its bounding box), at most 20, so that a box does not show as a rectangle. |
| `shelf` | number |  |  | Only for `sea` and `lake`. The width, in units, of the shallow water along the coast. The water is at the top of the height range at the coast and goes down to the bottom of the range at this distance from it. A side on a frame edge is not coast. More than 0. Default: the heights of the template, put into the range. |

### RegionRef

A reference to a pinned region.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `region` | string | yes |  | The id of a region pin. |

### ReliefOptions

The relief symbols of `print-safe` mode.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `density` | number |  | `1.0` | How dense the symbols are. 0 to 1. 1 is the full density. A smaller value puts the symbols further apart: 0.25 shows about a quarter of them. 0 shows none. |
| `hide` | array of [`ReliefSymbol`](#reliefsymbol) |  |  | The symbol kinds to leave out: `mountain`, `hill`, `tree` (the forest biomes), `marsh` (wetland), or `dune` (the deserts). |
| `terrain` | boolean |  |  | Both render modes. `true`: the symbols follow the terrain. Mountains stand in chains of overlapping peaks along the ridges, larger at the crest, with hills at their feet. Trees grow in groups, denser in the highland, and the symbols change a little in size. `false`: the placement of Azgaar's Fantasy Map Generator. Default `true`. |

### ReliefSymbol

A kind of relief symbol of `print-safe` mode.

Type: `"mountain"`, `"hill"`, `"tree"`, `"marsh"`, `"dune"`

### RenameEntry

A name or arms for a generated area. The entry finds its area by a point: the area of its kind that holds the cell at `at`, or at the point pin `pin`. Give exactly one of `at` and `pin`.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `at` | array of 2 number |  |  | A point `[x, y]` in the area, in units. |
| `coa` | [`Coa`](#coa) |  |  | The coat of arms of a state or a province, in the JSON shape of Azgaar's Fantasy Map Generator. Without `shield`, the arms keep the generated shield shape. |
| `color` | string |  |  | The color of a state, as `#rrggbb`. It replaces the generated color in the colour SVG and in `world.json`. Only for a state. |
| `full_name` | string |  |  | The new full name of a state or a province, for example `Kingdom of Ostrey`. Default: the full name with the old name changed to the new name. |
| `kind` | [`AreaKind`](#areakind) | yes |  | The kind of the area. |
| `name` | string |  |  | The new name. Not empty. |
| `pin` | string |  |  | The id of a point pin in the area, for example its capital. |

### Render

Render options for all variants.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `coast_echo` | boolean |  |  | Coast echo lines of `colour` mode. `true`: thin lines in the sea follow the coast. Each line is further from the last line and fainter. `print-safe` mode does not use it. |
| `coastline` | [`CoastlineDetail`](#coastlinedetail) |  |  | Coastline detail. `smooth` cuts the corners of the cell edges. `fractal` adds the fractal detail of Azgaar's Fantasy Map Generator. Default: `smooth` in `print-safe` mode, `fractal` in `colour` mode. |
| `depth_lines` | array of number |  |  | `print-safe` mode: the depths of the dashed depth lines in the ocean, below sea level on the height scale 0 to 100. Each line goes around the ocean at least that deep. Empty: no depth lines. Default `[6, 12]`. `colour` mode uses the style value `ocean.groups.oceanLayers.options.depths` instead. |
| `emblems` | [`EmblemOptions`](#emblemoptions) |  |  | Options of the layer `emblems`. |
| `extras` | [`Extras`](#extras) |  |  | Extras such as the scale bar and the compass. |
| `font_sizes_pt` | [`FontSizes`](#fontsizes) |  |  | Font sizes, in points. |
| `fonts` | [`Fonts`](#fonts) |  |  | Font families. The SVG names each family exactly as given here. The SVG never embeds a font. |
| `fonts_allowed` | array of string |  | `["Caladea","Inter","IM Fell English","IM Fell English SC"]` | Font families that the SVG can use. A family in `fonts` that is not in this list is an error. |
| `hand_drawn` | boolean |  |  | The hand-drawn look of `print-safe` mode. `true`: the lines have a small seeded wobble, rivers and coasts taper, and the dash patterns are less regular. No line becomes thinner than `min_line_width_pt`. `false`: the lines are exact. `colour` mode does not use it. |
| `hill_shading` | boolean |  |  | Hill shading of `colour` mode. `true`: the land is darker on the slopes that turn away from a light in the north-west, and lighter on the slopes that turn to it. `print-safe` mode does not use it. |
| `labels` | [`LabelRules`](#labelrules) |  |  | Rules for the labels of all variants. |
| `layers` | array of [`LayerName`](#layername) |  | see the type | Layers to draw. In `print-safe` mode the layers are drawn in this order, from bottom to top. In `colour` mode the renderer draws them in the layer order of Azgaar's Fantasy Map Generator. A layer that is only for `colour` mode draws nothing in `print-safe` mode. |
| `min_line_width_pt` | number |  | `0.5` | Smallest permitted line width, in points. |
| `mode` | [`RenderMode`](#rendermode) |  | `"print-safe"` | Render mode. `print-safe` is black on white only. `colour` draws the map as Azgaar's Fantasy Map Generator does, with the style in `style`. |
| `relief` | [`ReliefOptions`](#reliefoptions) |  |  | The relief symbols of `print-safe` mode: mountains, hills, trees, marsh, and dunes. `colour` mode uses the style value `relief.options.density` instead. |
| `river_contrast` | number |  |  | How strongly the width of a river follows its water, in both render modes. 0 to 2. 0: the widths of Azgaar's Fantasy Map Generator. A larger value makes the rivers with the most water wider and the small streams thinner. Default 1. |
| `style` | [`StyleName`](#stylename) |  |  | The style preset of `colour` mode. A preset gives the colours, the line widths, the fonts, and the effects of each layer. The presets come from Azgaar's Fantasy Map Generator. `print-safe` mode does not use it. |
| `style_overrides` | object |  |  | Changes to the style preset, for `colour` mode. Each key is the path of one value in the style tree of Azgaar's Fantasy Map Generator, for example `ocean.groups.base.attrs.fill` or `relief.options.density`. Each value is a string, a number, a boolean, or `null`. `null` removes the attribute. `docs/STYLES.md` gives the tree. |

### RenderMode

Render modes.

Type: `"print-safe"`, `"colour"`

### RiverPin

A river with a fixed source, course, and mouth. A river has no height: generation calculates the heights so that the river flows downhill.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `id` | string | yes |  | Identifier: lower-case letters and digits, words joined by `-`. Unique across all pins. |
| `mouth` | [`Anchor`](#anchor) | yes |  | Where the river ends: a point in water, a port place, or a region of kind `sea`, `lake`, or `marsh`. |
| `name` | string |  |  | Name of the river. |
| `source` | [`Anchor`](#anchor) | yes |  | Where the river starts: a point, a place, or a region. |
| `through` | array of [`Anchor`](#anchor) |  | `[]` | Points or places that the river passes, in order. |

### RoadKind

Kinds of roads.

Type: `"road"`, `"trail"`

### RoadPin

A road between two pinned places.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `from` | string | yes |  | The id of the place where the road starts. |
| `id` | string | yes |  | Identifier: lower-case letters and digits, words joined by `-`. Unique across all pins. |
| `kind` | [`RoadKind`](#roadkind) |  | `"road"` | Kind of road. |
| `target_length` | number |  |  | Wanted length of the road, in units. Optional. |
| `to` | string | yes |  | The id of the place where the road ends. |
| `tolerance` | number |  | `0.05` | Permitted difference from `target_length`, as a fraction (0.05 is 5 percent). |
| `via` | array of [`Waypoint`](#waypoint) |  | `[]` | Places (ids) or points `[x, y]` in units that the road passes, in order. |

### StyleName

The style presets of `colour` mode. Each preset is a style of Azgaar's Fantasy Map Generator.

Type: `"default"`, `"ancient"`, `"atlas"`, `"cinderwood"`, `"clean"`, `"cyberpunk"`, `"dark-seas"`, `"frostbite"`, `"gloom"`, `"ink"`, `"light"`, `"monochrome"`, `"night"`, `"pale"`, `"watercolor"`

### TariffEntry

An import rule of a state.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `goods` | array of string |  |  | The names of the goods that the rule is for. Default: all goods. |
| `rate` | [`TariffRate`](#tariffrate) | yes |  | The rate: a share of the price, or `embargo`. |
| `states` | array of string |  |  | The point pins in the states that the rule is for. Default: all states. |

### TariffRate

The rate of a tariff: a share of the price, or `embargo`.

One of:

- number: A share of the price, 0 or more.
- string: The word `embargo`: no import.

### Template

A heightmap template.

One of:

- [`TemplateName`](#templatename): A named template.
- an object with `steps`: A list of template steps, one step in each string, for example `"Hill 1 90-99 60-80 45-55"`.
- an object with `raster`: A raster of heights, for example from an imported map.
- an object with `image`: A PNG image of heights, as a raster. The path is relative to the folder of the config file. Black is height 0 and white is height 100. A colour image gives the brightness of each pixel. The image covers the full frame: its top row is at the low y edge. Like a `raster`, the lake stages do not change these heights. A change of the image does not change the config hash.

### TemplateName

The names of the built-in heightmap templates.

Type: `"volcano"`, `"high-island"`, `"low-island"`, `"continents"`, `"archipelago"`, `"atoll"`, `"mediterranean"`, `"peninsula"`, `"pangea"`, `"isthmus"`, `"shattered"`, `"taklamakan"`, `"old-world"`, `"fractious"`

### Towns

A count, or `auto`.

One of:

- [`AutoKeyword`](#autokeyword): Generation chooses the number from the land area.
- integer: A fixed number of towns.

### TransportDomain

The travel domain of a transport.

Type: `"land"`, `"water"`, `"air"`, `"stay"`

### TransportEntry

A change to the journey transports. An entry with the name of a transport in the list changes the values that it gives, or removes the transport with `remove`. An entry with a new name adds a transport; it must give `speed` and `domain`. The list must keep a transport of the domain `stay`.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `domain` | [`TransportDomain`](#transportdomain) |  |  | The travel domain. |
| `hours_per_day` | number |  |  | The hours of travel that one day sustains. More than 0, to 24. |
| `name` | string | yes |  | The name of the transport, for example `Rowboat`. |
| `remove` | boolean |  |  | Remove the transport from the list. |
| `speed` | number |  |  | The speed in km/h. 0 or more. |

### UnitEntry

A change to the military units. An entry with the name of a unit in the list changes the values that it gives, or removes the unit with `remove`. An entry with a new name adds a unit; it must give `kind`.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `crew` | number |  |  | The people of one unit. More than 0. |
| `goods` | array of string |  |  | The names of the goods that equip the unit. |
| `icon` | string |  |  | The glyph of the icon. |
| `kind` | [`UnitKind`](#unitkind) |  |  | The unit type. |
| `name` | string | yes |  | The name of the unit, for example `archers`. |
| `power` | number |  |  | The battle power of one unit. 0 or more. |
| `remove` | boolean |  |  | Remove the unit from the list. |
| `rural` | number |  |  | The percent of the rural people that serve. 0 to 100. |
| `separate` | boolean |  |  | `true` if the unit forms regiments of its own, as fleets do. |
| `urban` | number |  |  | The percent of the urban people that serve. 0 to 100. |

### UnitKind

The type of a military unit.

Type: `"levies"`, `"melee"`, `"ranged"`, `"mounted"`, `"machinery"`, `"naval"`

### Units

The distance unit.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `name` | string |  | `"mile"` | Name of the unit, for example `mile`. The report and the scale bar use this name. |
| `per_cell_hint` | number |  |  | Wanted mean width of a cell, in units. Optional. When it is given, it sets the number of cells and replaces `world.cells`. The `--cells` flag removes it. |

### Variant

One map to render.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `crop` | array of 4 number |  |  | Rectangle `[x0, y0, x1, y1]` of the world to show, in units. Default: the full frame. |
| `extras` | [`Extras`](#extras) |  |  | Extras for this variant. A value here replaces the value in `render.extras`. |
| `name` | string | yes |  | Name of the variant: lower-case letters and digits, words joined by `-`. It is also the stem of the output file names. |
| `overlay` | string |  |  | Name of the overlay to show. Default: no overlay. |
| `page` | [`Page`](#page) | yes |  | Page size. |

### Waypoint

A point that a road passes.

One of:

- string: The id of a point pin.
- array of 2 number: A point `[x, y]`, in units.

### World

Generation options.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `border_noise` | number |  |  | How much the state borders wander. 0 to 1. Fractal noise from the seed adds a cost to the growth of the states, so that a border does not follow a straight line. 0: the costs of Azgaar's Fantasy Map Generator. Default 0.5. |
| `border_terrain` | number |  |  | How much the state borders follow the terrain. 0 to 1. A river cell and a ridge cell cost more to enter, so a state stops at a river or at a ridge, and the border goes along it. 0: the costs of Azgaar's Fantasy Map Generator. Default 0.5. |
| `brushes` | array of [`Brush`](#brush) |  |  | Heightmap brush strokes, applied in order after the template and before the pins. |
| `cells` | integer |  | `10000` | Number of cells in the first grid. 1000 to 100000. |
| `climate` | [`Climate`](#climate) |  |  | Climate options. |
| `culture_set` | string |  | `"european"` | Name of the culture set: `world`, `european`, `oriental`, `english`, `antique`, `high-fantasy`, `dark-fantasy`, or `random`. |
| `cultures` | integer |  | `6` | Number of cultures. |
| `default_ruler` | boolean |  |  | `false`: no default ruler across the largest landmass. |
| `edges` | [`FrameEdges`](#frameedges) |  |  | Land or sea along each frame edge, applied after the brushes and before the pins. Default: the template decides. |
| `emblem_shape` | string |  |  | The shield shape of the coats of arms: `culture` (the shield of the culture of each item, as Azgaar's Fantasy Map Generator does), `state` (the shield of the state), `random` (a random shield for each culture), or one shield name for all, for example `heater`, `spanish`, `round`, or `banner`. |
| `fill_land` | boolean |  |  | `true`: all water of the template and the brushes becomes low land, 1 to 3 above sea level. Only the pinned `sea` and `lake` regions, and the lakes that rivers make, are water. Use it to keep the whole frame as land except the pinned seas. Default `false`. |
| `finance` | array of [`FinanceEntry`](#financeentry) |  |  | Changes to the finance of the states: taxes, the market fee, the budget that sets the state orders, and the tariffs. |
| `goods` | array of [`GoodEntry`](#goodentry) |  |  | Changes to the goods catalogue of Azgaar's Fantasy Map Generator, in order. Empty: the default catalogue of 71 goods. |
| `growth_rate` | number |  | `1.0` | How far cultures and states grow. 0.1 to 2. Cultures and states stop at a total cost that this value multiplies. |
| `jitter` | number |  |  | How far each point of the first grid can move from its lattice position, as a fraction of half the distance between neighbor points. 0 to 0.9. 0 gives a regular lattice. |
| `journeys` | array of [`JourneyEntry`](#journeyentry) |  |  | Changes to the journeys: add a journey, or remove one. |
| `lake_elevation_limit` | integer |  |  | A land low point becomes a lake when its water cannot reach other water through cells lower than its height plus this limit, on the height scale 0 to 100. 0 to 80. 80 makes no such lakes. |
| `lattice` | [`Lattice`](#lattice) |  |  | The lattice of the points of the first grid: `square`, `hex-pointy`, `hex-flat`, `diamond`, or `triangle`. |
| `legends` | array of [`LegendEntry`](#legendentry) |  |  | Legend boxes: a title and colored items. |
| `markets` | array of [`MarketEntry`](#marketentry) |  |  | Changes to the markets: add, move, or remove a market center. |
| `measurers` | array of [`MeasurerEntry`](#measurerentry) |  |  | Measurers after the default ruler: rulers, opisometers, and planimeters. |
| `military_units` | array of [`UnitEntry`](#unitentry) |  |  | Changes to the military units of Azgaar's Fantasy Map Generator, in order. Empty: the default roster of 6 units. |
| `notes` | array of [`NoteEntry`](#noteentry) |  |  | Notes of the map, as the Notes editor of Azgaar's Fantasy Map Generator writes them. |
| `places` | [`PlaceOptions`](#placeoptions) |  |  | Options for places that generation adds. |
| `provinces_ratio` | integer |  | `20` | Provinces for each 100 places of a state. 0 to 100. A state gets at least 2 provinces when it has 2 places or more. 100: each place is the seat of a province. |
| `regiments` | array of [`RegimentEntry`](#regimententry) |  |  | Changes to the regiments: add, change, move, split, or remove. |
| `religions` | integer |  | `4` | Number of religions. |
| `renames` | array of [`RenameEntry`](#renameentry) |  |  | Names and arms for generated states, cultures, religions, and provinces. Generation applies them after the coats of arms, so the later stages use the new names. |
| `river_limit` | integer |  |  | The most rivers that generation keeps, besides the pinned rivers. It keeps the rivers with the most water. 0: only the pinned rivers. Default: all rivers. |
| `sea_level` | integer |  | `20` | Sea level on the height scale 0 to 100. Cells below it are water. |
| `size_variety` | number |  | `4.0` | How much the sizes of cultures and states differ. 0 to 10. A bigger value gives a bigger random part of the expansionism. |
| `states` | integer |  | `5` | Number of states. A point pin with `capital_of` is the capital of its state; generation adds the other capitals as cities. 0 and no `capital_of`: the world has no states and no capitals. With `towns: 0` too, the only places are the pins. |
| `template` | [`Template`](#template) |  | `"continents"` | Heightmap template: a template name, or a list of steps. |
| `towns` | [`Towns`](#towns) |  | `"auto"` | Number of towns that generation adds, or `auto`. |
| `transports` | array of [`TransportEntry`](#transportentry) |  |  | Changes to the journey transports of Azgaar's Fantasy Map Generator, in order. Empty: the default list of 20 transports. |
| `zones` | array of [`ZoneEntry`](#zoneentry) |  |  | Changes to the zones: new zones, and changes to or removal of the generated zones. |

### XAxis

The direction of the x axis.

Type: `"east"`

### YAxis

The direction of the y axis.

Type: `"south"`, `"north"`

### ZoneEntry

A change to the zones. With a shape (`polygon`, `box`, or `circle`), the entry adds a zone of the cells in the shape; it must give `name` and `kind`. With `at`, it changes the generated zone that holds the point, or removes it with `remove`.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `at` | array of 2 number |  |  | A point `[x, y]` in the generated zone to change or to remove. |
| `box` | array of 4 number |  |  | The rectangle of a new zone `[x0, y0, x1, y1]`, in units. |
| `circle` | [`Circle`](#circle) |  |  | The circle of a new zone, in units. |
| `color` | string |  |  | The fill: a hatch pattern of FMG, for example `url(#hatch1)`, or a color as `#rrggbb`. |
| `kind` | string |  |  | The zone type, for example `Invasion`, `Disease`, or `Flood`. |
| `name` | string |  |  | The name of the zone. |
| `note` | string |  |  | The legend text of the zone. |
| `polygon` | array of array of 2 number |  |  | The outline of a new zone `[[x, y], ...]`, in units. |
| `remove` | boolean |  |  | Remove the zone at `at`. |

