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
| `font_dirs` | array of string |  | `[]` | Directories with font files. Only the PNG preview uses them. |
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

### Cartouche

A title cartouche.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `title` | string | yes |  | The title text. |

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

### CoastlineDetail

Coastline detail.

One of:

- `"smooth"`: Cut the corners of the cell edges.
- `"fractal"`: Add the fractal detail of Azgaar's Fantasy Map Generator.

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

### Geography

The size and the position of the map on the globe.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `latitude` | number |  |  | The north-south position in percent. 0 to 100. 0 puts the north edge at the north pole, and 100 puts the south edge at the south pole. |
| `longitude` | number |  |  | The west-east position in percent. 0 to 100. |
| `map_size` | number |  |  | The map size in percent of the world. 1 to 100. 100 is the full globe from pole to pole. |

### Grid

A grid.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `step` | number | yes |  | Distance between grid lines, in units. |

### LabelOptions

Label options.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `hide` | boolean |  | `false` | `true`: do not show the label. |
| `size_pt` | number |  |  | Font size, in points. Default: the size of the label kind. |

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
- `"precipitation"`: Precipitation. Only `colour` mode.
- `"population"`: Population. Only `colour` mode. Draws nothing when the world has no population.
- `"emblems"`: Coats of arms. Only `colour` mode. Draws nothing when the world has no emblems.
- `"burg-icons"`: The symbols of the places.
- `"markers"`: Markers. Only `colour` mode. Draws nothing when the world has no markers.
- `"scale-bar"`: A scale bar.
- `"vignette"`: A dark edge around the map. Only `colour` mode.
- `"legend"`: A legend box. Only `colour` mode.

### Origin

The corner of the origin.

Type: `"top-left"`, `"bottom-left"`

### Overlay

An overlay: what one variant adds to, removes from, or renames in the base world. An overlay cannot change terrain, rivers, or coasts.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `add` | [`OverlayAdd`](#overlayadd) |  |  | Items that only this overlay shows. |
| `labels` | map of name to [`LabelOptions`](#labeloptions) |  |  | Label options, from `"<kind>:<id>"` to the options. |
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
| `capital_of` | string |  |  | Name of a state. The state forms around this place, its capital. |
| `custom_kind` | string |  |  | Name of the kind. Required when `kind` is `custom`. |
| `id` | string | yes |  | Identifier: lower-case letters and digits, words joined by `-`. Unique across all pins. |
| `kind` | [`PlaceKind`](#placekind) | yes |  | Kind of place. |
| `name` | string |  |  | Name of the place. Default: a generated name. |
| `port` | boolean |  | `false` | `true`: the cell of the place touches the sea or a lake. |

### Raster

A raster of heights that covers the full frame. The raster divides the frame into `cols` x `rows` equal rectangles. Each value is the height of one rectangle.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `cols` | integer | yes |  | Number of columns, from the low x edge to the high x edge. 2 or more. |
| `rows` | integer | yes |  | Number of rows, from the low y edge to the high y edge. 2 or more. |
| `values` | array of integer | yes |  | The heights on the scale 0 to 100, row by row. The first row is at the low y edge of the frame. In each row, the first value is at the low x edge. The list has `cols` x `rows` values. |

### RegionKind

Kinds of forced terrain.

Type: `"sea"`, `"lake"`, `"mountains"`, `"hills"`, `"plains"`, `"marsh"`, `"forest"`, `"desert"`

### RegionPin

An area with a forced kind of terrain. Give exactly one of `polygon`, `box`, and `circle`.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `box` | array of 4 number |  |  | Rectangle `[x0, y0, x1, y1]`, in units. |
| `circle` | [`Circle`](#circle) |  |  | Circle, in units. |
| `edge` | number |  | `0.0` | Width of the soft edge outside the shape, in units. Heights blend from the forced range to the free terrain over this width. Inside the shape, the heights are always in the range. |
| `height` | array of 2 number |  |  | Height range `[min, max]` on the height scale 0 to 100 (sea level 20). Default: the range of the kind. |
| `id` | string | yes |  | Identifier: lower-case letters and digits, words joined by `-`. Unique across all pins. |
| `kind` | [`RegionKind`](#regionkind) | yes |  | Kind of terrain in the area. |
| `landlocked` | boolean |  | `false` | Only for `sea` and `lake`. `false`: a `sea` must connect to the ocean at a frame edge. `true`: an inland sea or a large lake. |
| `name` | string |  |  | Name of the area, for its label. |
| `polygon` | array of array of 2 number |  |  | Outline `[[x, y], ...]`, in units. 3 or more points. The outline must not cross itself. |

### RegionRef

A reference to a pinned region.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `region` | string | yes |  | The id of a region pin. |

### Render

Render options for all variants.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `coastline` | [`CoastlineDetail`](#coastlinedetail) |  |  | Coastline detail. `smooth` cuts the corners of the cell edges. `fractal` adds the fractal detail of Azgaar's Fantasy Map Generator. Default: `smooth` in `print-safe` mode, `fractal` in `colour` mode. |
| `extras` | [`Extras`](#extras) |  |  | Extras such as the scale bar and the compass. |
| `font_sizes_pt` | [`FontSizes`](#fontsizes) |  |  | Font sizes, in points. |
| `fonts` | [`Fonts`](#fonts) |  |  | Font families. The SVG names each family exactly as given here. The SVG never embeds a font. |
| `fonts_allowed` | array of string |  | `["Caladea","Inter","IM Fell English","IM Fell English SC"]` | Font families that the SVG can use. A family in `fonts` that is not in this list is an error. |
| `layers` | array of [`LayerName`](#layername) |  | see the type | Layers to draw. In `print-safe` mode the layers are drawn in this order, from bottom to top. In `colour` mode the renderer draws them in the layer order of Azgaar's Fantasy Map Generator. A layer that is only for `colour` mode draws nothing in `print-safe` mode. |
| `min_line_width_pt` | number |  | `0.5` | Smallest permitted line width, in points. |
| `mode` | [`RenderMode`](#rendermode) |  | `"print-safe"` | Render mode. `print-safe` is black on white only. `colour` draws the map as Azgaar's Fantasy Map Generator does, with the style in `style`. |
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

### Template

A heightmap template.

One of:

- [`TemplateName`](#templatename): A named template.
- an object with `steps`: A list of template steps, one step in each string, for example `"Hill 1 90-99 60-80 45-55"`.
- an object with `raster`: A raster of heights, for example from an imported map.

### TemplateName

The names of the built-in heightmap templates.

Type: `"volcano"`, `"high-island"`, `"low-island"`, `"continents"`, `"archipelago"`, `"atoll"`, `"mediterranean"`, `"peninsula"`, `"pangea"`, `"isthmus"`, `"shattered"`, `"taklamakan"`, `"old-world"`, `"fractious"`

### Towns

A count, or `auto`.

One of:

- [`AutoKeyword`](#autokeyword): Generation chooses the number from the land area.
- integer: A fixed number of towns.

### Units

The distance unit.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `name` | string |  | `"mile"` | Name of the unit, for example `mile`. The report and the scale bar use this name. |
| `per_cell_hint` | number |  |  | Wanted mean width of a cell, in units. Optional. When it is given, it sets the number of cells and replaces `world.cells`. |

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
| `brushes` | array of [`Brush`](#brush) |  |  | Heightmap brush strokes, applied in order after the template and before the pins. |
| `cells` | integer |  | `10000` | Number of cells in the first grid. 1000 to 100000. |
| `climate` | [`Climate`](#climate) |  |  | Climate options. |
| `culture_set` | string |  | `"european"` | Name of the culture set: `world`, `european`, `oriental`, `english`, `antique`, `high-fantasy`, `dark-fantasy`, or `random`. |
| `cultures` | integer |  | `6` | Number of cultures. |
| `growth_rate` | number |  | `1.0` | How far cultures and states grow. 0.1 to 2. Cultures and states stop at a total cost that this value multiplies. |
| `jitter` | number |  |  | How far each point of the first grid can move from its lattice position, as a fraction of half the distance between neighbor points. 0 to 0.9. 0 gives a regular lattice. |
| `lake_elevation_limit` | integer |  |  | A land low point becomes a lake when its water cannot reach other water through cells lower than its height plus this limit, on the height scale 0 to 100. 0 to 80. 80 makes no such lakes. |
| `lattice` | [`Lattice`](#lattice) |  |  | The lattice of the points of the first grid: `square`, `hex-pointy`, `hex-flat`, `diamond`, or `triangle`. |
| `places` | [`PlaceOptions`](#placeoptions) |  |  | Options for places that generation adds. |
| `religions` | integer |  | `4` | Number of religions. |
| `sea_level` | integer |  | `20` | Sea level on the height scale 0 to 100. Cells below it are water. |
| `size_variety` | number |  | `4.0` | How much the sizes of cultures and states differ. 0 to 10. A bigger value gives a bigger random part of the expansionism. |
| `states` | integer |  | `5` | Number of states. |
| `template` | [`Template`](#template) |  | `"continents"` | Heightmap template: a template name, or a list of steps. |
| `towns` | [`Towns`](#towns) |  | `"auto"` | Number of towns that generation adds, or `auto`. |

### XAxis

The direction of the x axis.

Type: `"east"`

### YAxis

The direction of the y axis.

Type: `"south"`, `"north"`

