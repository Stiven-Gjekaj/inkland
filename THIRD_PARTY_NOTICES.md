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

## Rust crates

The `inkland` binary includes Rust crates under MIT, Apache-2.0, ISC,
BSD-2-Clause, BSD-3-Clause, and Unicode-3.0 licenses. `deny.toml` lists the
licenses that the build accepts. To list each crate with its license, run
`cargo deny list`.
