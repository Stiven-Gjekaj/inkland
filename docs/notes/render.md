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
| ocean | `draw-ocean.ts`, `ocean-generator.ts`, `coastal-waves.ts`, `draw-coastal-bands.ts` | Base colour, depth rings on the first grid, waves, coastal bands. No bitmap pattern. |
| landmass | `draw-landmass.ts` | A rect through the land mask. |
| texture | `draw-texture.ts` | Draws nothing: the textures are bitmaps. |
| heightmap | `heightmap/draw-heightmap.ts` | Height tints of the first grid in the scheme of the style. Contours are simpler: a line each 4 height units. No hachures. |
| lakes | `draw-lakes.ts` | Groups by lake type, through the water mask. No ripples. |
| coastline | `draw-coastline.ts`, `coastline-generator.ts` | Fractal detail as FMG, with the FMG noise and path builder. |
| biomes, states, provinces, cultures, religions | `draw-*.ts`, `isoline-fills.ts` | Cell areas with a water gap stroke, through the land mask. States have a blurred halo. |
| zones | `draw-zones.ts` | Reads `fmg::data::zones`; the world has no zones yet. |
| borders | `draw-borders.ts` | Straight lines along the cell edges. |
| routes | `draw-routes.ts` | Catmull-Rom curves (alpha 0.1). Shared stretches of the network are drawn once. |
| rivers | `draw-rivers.ts`, `river-generator.ts` | Meandering and the width outline of FMG, with the width factor and source width of each river. |
| relief | `draw-relief-icons.ts`, `relief-generator.ts` | Poisson-disc placement with the FMG rules and biome pools; the 5 icon sets. Icons under a label are left out. |
| burg icons | `draw-burg-icons.ts` | Icon and anchor groups by kind. |
| ice | `draw-ice.ts` | Glaciers and icebergs of `World::ice`. |
| cells, grid, coordinates | `draw-cells.ts`, `draw-grid.ts`, `draw-coordinates.ts` | Also in print-safe mode. |
| temperature, precipitation | `draw-temperature.ts`, `draw-precipitation.ts` | First grid. No wind arrows. |
| population, emblems, markers | `draw-population.ts`, `draw-emblems.ts`, `draw-markers.ts` | Read `fmg::data`; the world has no such data yet. |
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
- Not done: bitmap textures and ocean patterns, heightmap hachures, lake
  ripples, wind arrows, journeys, goods, markets, military, and the
  fogging layer.

## Print-safe mode

- `render.coastline: "fractal"` draws the FMG coastline in black.
- `cells`, `grid`, `coordinates`, `coastline`, `ocean`, `borders`,
  `routes`, `burg-icons`, `scale-bar`, and `compass` have black and white
  drawings. The other new layers draw nothing.
- The river width comes from `River::width`, the FMG width.
