# Land and sea for a book map

This guide tells how to draw the land and the sea of a book map with the
config. `docs/CONFIG.md` gives each field.

## Three ways to describe the land

- A **template** makes the free terrain: a named template, a list of
  steps, a `raster`, or an `image`.
- **Region pins** (`pins.regions`) force a kind of terrain in a shape:
  sea, lake, land, hills, mountains, and so on. A region has a height
  range.
- **Character areas** (`pins.areas`) steer the land softly. An area has a
  character, a loose outline, and no height range.

The order is: the template, the brushes, `world.edges`, the character
areas, then the region pins. A region sets the height range, and an area
inside it shapes the land in that range.

## Draw a coast

Start from a template with much sea. Then draw each continent as a `land`
region. A `land` region keeps the land of the template and makes its water
low land.

```json
"world": { "template": { "steps": ["Hill 60 2-6 5-95 5-95", "Smooth 2"] } },
"pins": { "regions": [
  { "id": "maeris", "kind": "land", "roughness": 10,
    "polygon": [[60, 40], [240, 50], [220, 160], [80, 150]] }
] }
```

- `roughness` moves the outline in and out, by at most this many units.
  The coast gets large bays (about 12 times the roughness) and small coves
  and capes. A domain warp curls them.
- `coast_detail` (1 to 8, default 4) sets how fine the coves are.
- `islets` (0 to 1, default 0.3 for `land`) puts small islands off the
  coast, most of them off the capes.
- A side of an outline on a frame edge does not move.
- Near a port, the noise fades out, so the coast of the port stays where
  you draw it.

A `sea` region inside a `land` region needs a higher `priority` than the
land, for example `"priority": -1` on the land. Without it, the two
regions conflict.

A land region without a `roughness` gets 3 percent of its size, so a `box`
does not show as a rectangle. Give `"roughness": 0` for an exact outline.
A `sea` or a `lake` region is exact by default.

## Use a height image

Give the path of a PNG file, relative to the config file:

```json
"world": { "template": { "image": "heights.png" } }
```

Black is height 0 and white is height 100. Sea level is 20 by default, so
a gray of 51 is the coast. The image covers the full frame.

## Islands in a pinned sea

A place in a `sea` region is an error, with two exceptions:

- A land region with a higher `priority` than the sea holds the place.
  The region is the island.
- `"island": true` on a point pin makes the cell of the place a small
  island.

```json
"regions": [
  { "id": "bay", "kind": "sea", "polygon": [[320, 280], [380, 280], [385, 305], [318, 298]] },
  { "id": "isle", "kind": "hills", "priority": 1, "height": [22, 28],
    "circle": { "at": [340, 290], "r": 6 } }
],
"points": [
  { "id": "portvale", "kind": "town", "at": [344, 290], "port": true }
]
```

## The sea

- `shelf` on a `sea` or `lake` region: the water is shallow along the
  coast and deep at this distance from it.
- `height` on a water region sets its depth range, below sea level.
- `world.deep_sea: true` sinks the seabed of the template: each height
  below sea level becomes 0.4 of it. Without it, the hills of the seabed
  of a sea template show as pale shallows in open ocean. Use `shelf` on a
  sea region for shallow water along its coast.
- `world.edges` puts open sea or land along a frame edge:
  `{ "north": "sea", "west": "land", "width": 60 }`. Land at an edge goes
  on past the frame.
- In `colour` mode, the style option
  `ocean.groups.oceanLayers.options.depths` draws depth layers from the
  heights. In `print-safe` mode, `render.depth_lines` draws dashed depth
  lines.

## Character areas

An area tells what the land is like, not how high it is:

| `character` | The heights | The biomes and the relief |
|---|---|---|
| `rough` | Higher, with sharp ridges | Mountain chains along the ridges, hills at their feet |
| `wooded` | No change | Forest |
| `rolling` | Low, rounded hills | Hills here and there |
| `flat` | Low land | Few symbols |

```json
"areas": [
  { "id": "iron-wilds", "character": "rough",
    "polygon": [[600, 300], [760, 280], [800, 420], [640, 460]] },
  { "id": "greenwood", "character": "wooded",
    "circle": { "at": [330, 430], "r": 60 }, "strength": 0.8 },
  { "id": "the-levels", "character": "flat", "box": [900, 600, 1100, 760],
    "edge": 40 }
]
```

- `strength` (0 to 1, default 1) sets how strong the character is.
- `edge` sets the width of the soft edge. The default is a fifth of the
  size of the shape.
- An area changes only land. The coast stays where it is.

## Borders, rivers, and relief

- `world.border_noise` (0 to 1, default 0.5): the state borders wander.
- `world.border_terrain` (0 to 1, default 0.5): the state borders follow
  the rivers and the ridges.
- `render.river_contrast` (0 to 2, default 1): the main rivers are wider,
  and the small streams are thinner.
- `render.relief.terrain` (default `true`): in both render modes, the
  mountains stand in chains along the ridges, and the trees grow in
  groups. `false` gives the placement of Azgaar's Fantasy Map Generator.

Give 0 (or `false`) to each of these options to get the look of the
versions before them.

## The largest river

With `render.river_contrast`, the river with the most water is much wider
than the others. Generation chooses it from the terrain, so it can be in a
land that the story does not care about. Before you finish a map:

1. Find the widest river on the map. In `world.json`, each river in
   `rivers` has its `name`, its water flow (`flux`), and its `width`.
   `inkland query --world world.json --at X,Y` gives the name of the
   river nearest to a point.
2. Pin the main river of the story in `pins.rivers`, and give it
   `"main": true`. It is then at least as wide as the largest other river.
3. If a generated river is still too wide, give `render.river_contrast` a
   lower value, or keep fewer generated rivers with `world.river_limit`.

## The heights of the relief symbols

The heights below are on the scale 0 to 100. The values are for the
default `world.sea_level` of 20. With another sea level, add the
difference.

`print-safe` mode:

| Height | Symbol |
|---|---|
| 65 to 100 | mountain |
| 42 to 64 | hill |
| 20 to 41 | the symbol of the biome: tree, marsh, dune, or none |

`colour` mode:

| Height | Symbol |
|---|---|
| 71 to 100 | mountain, with snow at -1 C or colder |
| 50 to 70 | hill |
| 20 to 49 | the symbol of the biome |

With `render.relief.terrain` (the default), the ridges come first, in
both modes. A ridge cell is a land cell that is a local top: it is
at least 1 higher than the mean of its neighbours, and at most 2 of its neighbours
are higher or as high. It must also be at height 35 or more, and in the
highest 15 percent of the land of the map. Each ridge cell gets a
mountain, and each cell next to a ridge gets a hill. So a small upland
of heights 36 to 46 shows peaks, and land below 35 shows no mountain.

To get hills and no peaks, keep the land below 35, or below the highest
15 percent of the map. To get peaks, give the region a range that goes
above 35, with a top above the rest of the land.
