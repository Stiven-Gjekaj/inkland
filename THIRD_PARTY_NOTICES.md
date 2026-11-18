# Third-party notices

inkland re-implements the algorithms of Azgaar's Fantasy Map Generator.
Some data tables come from it. This file keeps its license notice.

## Azgaar's Fantasy Map Generator

- Source: <https://github.com/Azgaar/Fantasy-Map-Generator>
- Commit read: `74136292b8012abe0d57a11ac7780ddc43d9433d`
- License: MIT (full text below, without changes)
- Data tables copied into inkland:
  - The step lists of the 14 heightmap templates, from
    `src/data/heightmap-templates.ts`. In inkland:
    `crates/inkland-core/src/heightmap/templates.rs`.
  - The blob and line fall-off tables, from
    `src/model/physical/heightmap-generator.ts`. In inkland:
    `BLOB_POWER` and `LINE_POWER` in
    `crates/inkland-core/src/heightmap/mod.rs`.
  - The default temperatures, the default winds, and the precipitation
    factor of each latitude band, from
    `src/model/physical/temperature-generator.ts`,
    `src/model/physical/precipitation-generator.ts`, and
    `src/model/options/options-model.ts`. In inkland:
    `crates/inkland-core/src/climate.rs`.
  - The biome list and the moisture-temperature biome matrix, from
    `src/model/physical/biomes-generator.ts`. In inkland:
    `crates/inkland-core/src/biomes.rs`.
  - The habitability of each biome, from
    `src/model/physical/biomes-generator.ts`. In inkland:
    `HABITABILITY` in `crates/inkland-core/src/biomes.rs`.
  - The 43 name bases, from `src/data/name-bases.ts`, and the culture names
    of five culture sets, from `src/model/society/cultures-generator.ts`. In
    inkland: `crates/inkland-core/src/names/data.rs`.
  - The step sizes of the square, hexagon, diamond, and triangle lattices,
    from `src/model/graph/lattice.ts`. In inkland: `LatticeInfo::plan` in
    `crates/inkland-core/src/grid.rs`.
  - The whole-world chances and the size draws of the templates, from
    `src/model/physical/coordinates.ts`. In inkland:
    `crates/inkland-core/src/coordinates.rs`.
  - The river constants (the least flux of a river, the width factors, the
    length progression, and the cut limit) and the lake evaporation
    formula, from `src/model/physical/river-generator.ts` and
    `src/model/physical/lakes.ts`. In inkland:
    `crates/inkland-core/src/rivers.rs`.
  - The river types and their weights, and the list of adjectives for
    feature names, from `src/model/physical/river-generator.ts` and
    `src/model/physical/features-generator.ts`. In inkland:
    `crates/inkland-core/src/geonames.rs`.
  - The 15 style presets, from `src/model/styles/default-styles.json` and
    `public/styles/*.json`. In inkland: `crates/inkland-render/assets/fmg/styles`.
  - The relief, place, and port icons (SVG files), from
    `src/assets/icons/relief`, `burgs`, and `ports`. The place icons of the
    set `watabou` are by Watabou (Oleg Dolya); each file names its author.
    In inkland: `crates/inkland-render/assets/fmg/icons`.
  - The SVG filters, the compass rose, and the grid patterns, from
    `src/index.html`, and the biome colours and relief pools, from
    `src/model/physical/biomes-generator.ts`. In inkland:
    `crates/inkland-render/src/fmg/`.
  - The emblem generator data: tinctures, divisions, lines, ordinaries,
    charge positions, charge categories and charge data, the charges of
    each culture type, and the shield types, from `src/data/emblems/*.ts`.
    In inkland: `crates/inkland-core/src/emblems/data.rs`. The shield of
    each culture of the culture sets, from
    `src/model/society/cultures-generator.ts`. In inkland:
    `crates/inkland-core/src/culture_sets.rs`.
  - The emblem drawing data: the shield shapes and charge positions, the
    division and ordinary templates, the lines of partition, the tincture
    colours, and the field patterns, from `src/renderers/emblems/*.ts`.
    FMG ports them from Armoria (<https://github.com/Azgaar/Armoria>), by
    the same author. In inkland:
    `crates/inkland-render/src/emblem/data.rs`.
  - 108 charge files (SVG), from `src/assets/icons/charges`. inkland
    keeps only the files with the license CC0 1.0 (public domain); each
    file names its author and its license in its `metadata` element, and
    the files have no change. Authors: Azgaar, Celbusro, Fedor Taran,
    Gothika, Inkscaper, Leonid 2, Madboy74, Marmelad, Rama, Zorlot,
    spax89, toa267, and V. A. Lyakhor. In inkland:
    `crates/inkland-render/assets/emblems/charges`. The other 245 charge
    files of FMG have other licenses (CC BY-NC-SA 3.0, CC BY-SA, GNU FDL,
    CC BY, Free Art License) or no license. inkland does not include
    them.
- Test data copied into inkland:
  - The test maps `tests/fixtures/1.112.1.map` and `1.154.0.map`, with gzip
    and no other change. In inkland: `fixtures/azgaar/*.map.gz`. They come
    from the original under the MIT license below.

```text
MIT License

Copyright 2017-2024 Max Haniyeu (Azgaar), azgaar.fmg@yandex.com

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

You can produce, without restrictions, any derivative works from the original
software and even reap commercial benefits from the sale of the secondary product.
The derivates include created maps, map images, screenshots, videos, and other materials.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Font metrics

`crates/inkland-render/src/metrics_data.rs` holds numbers from four font
files: the advance width of each character from U+0020 to U+024F, the cap
height, and the descender. `examples/font_metrics.rs` reads them. inkland
does not include the font files or their glyph outlines.

- IM Fell English and IM Fell English SC, by Igino Marini. SIL Open Font
  License 1.1.
- Caladea, by Carolina Giovagnoli and Andrés Torresi (Huerta Tipográfica).
  Apache License 2.0.
- Inter, by Rasmus Andersson. SIL Open Font License 1.1.

## Rust crates

The `inkland` binary includes Rust crates under MIT, Apache-2.0, ISC,
BSD-2-Clause, BSD-3-Clause, Unicode-3.0, and Zlib licenses. `deny.toml` lists the
licenses that the build accepts. To list each crate with its license, run
`cargo deny list`.
