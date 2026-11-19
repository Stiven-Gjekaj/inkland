# Notes: the renderer of colour mode

These notes describe `render.mode: "colour"`. The code is in
`crates/inkland-render/src/fmg/`.

## What it does

- Colour mode draws the map as Azgaar's Fantasy Map Generator (FMG) does.
  The layers, their order, their SVG groups, and their style attributes
  come from `src/renderers/draw-*.ts`, `src/renderers/layers.ts`, and
  `src/index.css` of FMG.
- `render.style` selects one of 15 presets (see `docs/STYLES.md`).
  `render.style_overrides` sets single values by their path.
- The renderer puts the world in a map of 1920 x 1080 FMG pixels. All
  layers draw in these pixels inside one group, `viewbox`, that scales
  the pixels to millimetres. So the line widths and the sizes of the
  styles keep the look of FMG.
- The layers are drawn in the FMG order. The `print-safe` names stand for
  FMG layers: `coast` is ocean, landmass, lakes, and coastline; `regions`
  is states and borders; `roads` is routes; `places` is burg icons.

## Layers and their FMG sources

| Layer | FMG source | Notes |
|---|---|---|
| ocean | `draw-ocean.ts`, `ocean-generator.ts`, `coastal-waves.ts`, `draw-coastal-bands.ts` | Base colour, depth rings on the first grid, the bitmap pattern of the style, waves, coastal bands. |
| landmass | `draw-landmass.ts` | A rect through the land mask. |
| texture | `draw-texture.ts` | The paper texture of the style. The SVG holds the image as a data URI. |
| heightmap | `heightmap/draw-heightmap.ts` | Height tints of the first grid in the scheme of the style. Contours are simpler: a line each 4 height units. Hachures as FMG, but the outer row of cells has none: the grid has no boundary points. |
| lakes | `draw-lakes.ts`, `lake-ripples.ts` | Groups by lake type, through the water mask. The ripples of the style, with a mask that keeps them off the shore. |
| coastline | `draw-coastline.ts`, `coastline-generator.ts` | Fractal detail as FMG, with the FMG noise and path builder. |
| biomes, states, provinces, cultures, religions | `draw-*.ts`, `isoline-fills.ts` | Cell areas with a water gap stroke, through the land mask. States have a blurred halo. |
| zones | `draw-zones.ts`, `hatches.ts` | The zones of `World::zones`, filled with the FMG hatch patterns. |
| borders | `draw-borders.ts` | Straight lines along the cell edges. |
| routes | `draw-routes.ts` | The drawn curves of the core (`curves::road_line`): a centripetal Catmull-Rom spline with a small seeded wander. Shared stretches of the network are drawn once. |
| rivers | `draw-rivers.ts`, `river-generator.ts` | Meandering and the width outline of FMG, with the width factor and source width of each river. |
| relief | `draw-relief-icons.ts`, `relief-generator.ts` | Poisson-disc placement with the FMG rules and biome pools; the 5 icon sets. Icons under a label are left out. |
| burg icons | `draw-burg-icons.ts` | Icon and anchor groups by kind. |
| ice | `draw-ice.ts` | Glaciers and icebergs of `World::ice`. |
| goods | `draw-goods.ts` | The overview of all goods: producing cells, pies on places, deposit icons. |
| markets | `draw-markets.ts` | The market areas, their borders, and a pin with the market glyph at each center. |
| trade | `draw-trade-traffic.ts` | The goods of the day on each route link. Static lines: no wagons and ships. |
| military | `draw-military.ts` | The regiment boxes in the state colours, with the FMG glyphs and the totals. |
| journeys | `draw-journeys.ts` | Catmull-Rom curves (alpha 0.5) of the segments, with start dots and arrows. |
| rulers | `draw-measurers.ts` | The default ruler with its length in config units. |
| cells, grid, coordinates | `draw-cells.ts`, `draw-grid.ts`, `draw-coordinates.ts` | Also in print-safe mode. |
| temperature, precipitation | `draw-temperature.ts`, `draw-precipitation.ts` | First grid. The wind arrows of `getWinds`. |
| population, markers | `draw-population.ts`, `draw-markers.ts` | Read `fmg::data`. |
| emblems | `draw-emblems.ts`, `renderers/emblems/*.ts` | Sizes, poles, collision, and hidden groups of FMG. See `docs/notes/emblems.md`. |
| compass | `index.html` | The FMG rose with its lines, through the water mask. |
| scale bar, vignette, legend | `draw-scalebar.ts`, `draw-vignette.ts`, `draw-legend.ts` | The legend lists the states, cultures, religions, or biomes. |
| labels | `labels-renderer.ts`, `label-groups.ts` | The inkland placer, with the fonts, colours, and case of the label groups. |

## What differs from FMG

- Labels keep the placer and the font sizes of inkland. A CSS
  `text-shadow` becomes a stroke halo under the text.
- Burg icons have an em of 2 x (city label size / capital label size), so
  that they grow with the larger labels of a page.
- The world has no colours. States get the greedy Set2 colouring of FMG,
  cultures the FMG palette, provinces and religions mixed colours, from the
  `decoration` seed.
- Named oceans, seas, and gulfs get optional labels in the `added` group.
- The colour checks (`no_opacity`, `no_masks_filters_rgba`,
  `black_and_white`, `min_line_width`, `font_families_allowed`) report
  `"applicable": false` in colour mode.
- The texture and the ocean pattern are data URIs in the SVG, so the file
  has all that it needs.
- Roads are smooth curves with a small seeded wander, not the
  Catmull-Rom curves of FMG. Both modes draw the same curve, and the
  report measures the length of the road along it.
- Not done: the trade animation (the SVG is still) and the fogging layer.

## Print-safe mode

- `render.coastline: "fractal"` draws the FMG coastline in black.
- The layers `zones`, `goods`, `markets`, `trade`, `military`,
  `journeys`, and `rulers` draw only in colour mode.
- `cells`, `grid`, `coordinates`, `coastline`, `ocean`, `borders`,
  `routes`, `burg-icons`, `scale-bar`, and `compass` have black and white
  drawings. `emblems` has a drawing with hatching when
  `render.emblems.hatching` is `true`. The other new layers draw nothing.
- The river width comes from `River::width`, the FMG width.
- Roads are the same curves as in colour mode, with the dashes of a road
  or a trail.
- `render.hand_drawn` (default `true`) gives the lines a hand-drawn look:
  - A seeded wobble of up to 0.16 mm moves each coast, water line, river,
    border, and road a little to each side. The ends of an open line do
    not move.
  - Rivers and coasts are in pieces of about 4 mm, each with its own
    width. A river becomes wider from 45 percent of its mouth width at
    its source to its mouth width. A coast is from 70 to 130 percent of
    its width along its length.
  - Each dash and each gap of a road or a border changes by up to 30
    percent. Each line has its own pattern.
  - No line is thinner than `min_line_width_pt`, and the print checks
    pass. The code is in `crates/inkland-render/src/hand.rs`.
  - Set it to `false` for exact lines.
