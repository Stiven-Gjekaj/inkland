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
  - 72 goods icons (SVG files), from `src/assets/icons/goods`, with no
    change. Each file names its author and its license in its `desc`
    element. 65 files have the license CC BY 3.0
    (<https://creativecommons.org/licenses/by/3.0>). They come from the
    Noun Project, and Azgaar edited some of them. 7 files have the license
    CC0 1.0 (public domain). In inkland:
    `crates/inkland-render/assets/fmg/icons/goods`.
  - 7 paper textures (JPEG files) and 5 ocean patterns (PNG files) that the
    style presets name, from `public/images/textures` and `public/images`,
    with no change. In inkland: `crates/inkland-render/assets/fmg/images`.
  - The SVG filters, the compass rose, the grid patterns, and the hatch
    patterns, from `src/index.html`, and the biome colours and relief pools, from
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
  - 169 charge files (SVG), from `src/assets/icons/charges`, with no
    change. Each file names its author and its license in its `metadata`
    element. In inkland: `crates/inkland-render/assets/emblems/charges`.
    - 108 files have the license CC0 1.0 (public domain). Authors: Azgaar,
      Celbusro, Fedor Taran, Gothika, Inkscaper, Leonid 2, Madboy74,
      Marmelad, Rama, Zorlot, spax89, toa267, and V. A. Lyakhor.
    - 61 files have an attribution, share-alike, or GNU FDL license. These
      files stay under their own licenses, not under the license of
      inkland. A change to a CC BY-SA, Free Art, or GNU FDL file must keep
      the same license. The full text of the GNU FDL 1.3 is in
      `crates/inkland-render/assets/emblems/GFDL-1.3.txt`, and each release
      package has a copy. The files and their authors:
      - CC BY 1.0 (<https://creativecommons.org/licenses/by/1.0>):
        - `tools/pincers.svg` by Yorick
        - `tools/scissors2.svg` by Yorick
      - CC BY 4.0 (<https://creativecommons.org/licenses/by/4.0>):
        - `arms/lanceWithBanner.svg` by Jean-Paul Gibert
        - `conventional/gear.svg` by Jean-Paul GIBERT
      - CC BY-SA 2.5 (<https://creativecommons.org/licenses/by-sa/2.5>):
        - `miscellaneous/snowflake.svg` by Lokal_Profil
      - CC BY-SA 3.0 (<https://creativecommons.org/licenses/by-sa/3.0>):
        - `agriculture/scythe.svg` by Spedona
        - `architecture/lighthouse.svg` by Tretinville
        - `architecture/windmill.svg` by Christie L. Ward
        - `arms/gladius.svg` by Christie L. Ward
        - `arms/grenade.svg` by Jean-Paul Gibert
        - `arms/helmetCorinthian.svg` by Chatsam
        - `arms/rapier.svg` by Christie L. Ward
        - `bugs/ladybird.svg` by Christie L. Ward
        - `conventional/comet.svg` by Chatsam
        - `fishes/salmon.svg` by Pierre Huttin
        - `miscellaneous/chalice2.svg` by Anenja
        - `miscellaneous/hourglass.svg` by Syryatsu
        - `miscellaneous/lyre.svg` by Milenioscuro
        - `molluscs/kraken.svg` by Abjiklam
        - `molluscs/polypus.svg` by Christie L. Ward
        - `ornaments/mantle.svg` by Bluebear2
        - `ornaments/ribbon3.svg` by Gaeser
        - `people/archer.svg` by Leonid 2
        - `people/cossack.svg` by Alex Tora
        - `plants/thistle.svg` by Sodacan
        - `seafaring/drakkar.svg` by Spedona
        - `seafaring/shipWheel.svg` by Lidingo
        - `tools/fan.svg` by Mukai
        - `tools/saw.svg` by Adalric67
        - `tools/scalesHanging.svg` by Wikibelgiaan
      - CC BY-SA 4.0 (<https://creativecommons.org/licenses/by-sa/4.0>):
        - `arms/helmetGreat.svg` by Paladium
        - `arms/trident.svg` by Wilfredor
        - `beasts/ratRampant.svg` by Jacques63
        - `bodyparts/bone.svg` by Heralder
        - `bodyparts/crossedBones.svg` by Heralder
        - `bugs/spider.svg` by MostEpic
        - `bugs/wasp.svg` by Celbusro
        - `miscellaneous/amphora.svg` by Baseluna014
        - `miscellaneous/bookOpen.svg` by unknown author
        - `miscellaneous/drum.svg` by Erlenmeyer
        - `miscellaneous/feather.svg` by Madboy74
        - `miscellaneous/scrollClosed.svg` by Jacques63
        - `miscellaneous/violin.svg` by Jpgibert
        - `ornaments/ribbon1.svg` by Eugenio Hansen, OFS
        - `ornaments/ribbon2.svg` by Eugenio Hansen, OFS
        - `ornaments/ribbon5.svg` by Eugenio Hansen, OFS
        - `ornaments/ribbon6.svg` by Eugenio Hansen, OFS
        - `ornaments/ribbon8.svg` by Eugenio Hansen, OFS
        - `people/hermit.svg` by Jacques63
        - `tools/trowel.svg` by Jacques63
      - Free Art License 1.3 (<http://artlibre.org/licence/lal/en>):
        - `miscellaneous/bookClosed2.svg` by Tzeentch
      - GNU FDL 1.3 (<https://www.gnu.org/licenses/fdl-1.3.html>):
        - `agriculture/millstone.svg` by Madboy74
        - `agriculture/ploughshare.svg` by Madboy74
        - `architecture/castle2.svg` by Madboy74
        - `architecture/palace.svg` by Xavigivax
        - `architecture/portcullis.svg` by Ssire
        - `arms/lanceHead.svg` by Orror
        - `beasts/stagLodgedRegardant.svg` by Sodacan
        - `ornaments/ribbon7.svg` by Rinaldum
        - `tools/hook.svg` by Madboy74
        - `tools/ladder2.svg` by Bruno Vallette
    - inkland does not include the other 184 charge files of FMG: 183 have
      the license CC BY-NC-SA 3.0, which does not permit commercial use,
      and 1 has no license.
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

## The icon font of the desktop app

`crates/inkland-app/ui/fonts/fmg-icons.ttf` is the icon font of Azgaar's
Fantasy Map Generator (`src/assets/icons.css`), converted from WOFF2 to TTF
with no other change. It is a Fontello build of Font Awesome glyphs, and
FMG gives the Font Awesome license for it.

- Font Awesome Free, Copyright (c) Fonticons, Inc.
  (<https://fontawesome.com>).
- The font: SIL Open Font License 1.1. The icons: CC BY 4.0
  (<https://creativecommons.org/licenses/by/4.0/>).
- `crates/inkland-app/ui/fonts/OFL.txt` holds the license text.

## Font metrics

`crates/inkland-render/src/metrics_data.rs` holds numbers from four font
files: the advance width of each character from U+0020 to U+024F, the cap
height, and the descender. `examples/font_metrics.rs` reads them. The
metrics do not need the font files. The section "Fonts of the PNG preview"
lists the font files that inkland includes.

- IM Fell English and IM Fell English SC, by Igino Marini. SIL Open Font
  License 1.1.
- Caladea, by Carolina Giovagnoli and Andrés Torresi (Huerta Tipográfica).
  Apache License 2.0. The Caladea files of the PNG preview come from Google
  Fonts, which gives them under the SIL Open Font License 1.1.
- Inter, by Rasmus Andersson. SIL Open Font License 1.1.

## Fonts of the PNG preview

The PNG preview has font files in the program, so that the previews of the
style presets and of the print defaults do not use a fallback font. Arial,
Courier New, Georgia, and Times New Roman, which some presets name, have no
free license. inkland does not include them. The
files are in `crates/inkland-render/assets/fonts/`. The `inkland` binary
and the app include them. The SVG output names each font by its family. It
does not embed a font.

- Source: <https://github.com/google/fonts>, the directory `ofl/`.
  Arima Madurai is no longer in that directory. Its files come from the
  commit `bc09b2c5cc6066c7ab7c1b11e6b6c0a598609d44` of the same
  repository.
- License: SIL Open Font License 1.1 for each font. The files have no
  changes.
- Each font directory has the license file `OFL.txt` of the font, with
  its copyright notice.

Each font, its author, its directory, and its copyright notice:

- Almendra SC, by Ana Sanfelippo. `almendrasc/`: AlmendraSC-Regular.ttf.
  Copyright (c) 2011-2012, Ana Sanfelippo (anasanfe@gmail.com), with Reserved Font Name 'Almendra'
- Arima Madurai, by Natanael Gama and Joana Correia. `arimamadurai/`: ArimaMadurai-Regular.ttf, ArimaMadurai-Bold.ttf.
  Copyright 2015 The Arima Project Authors (info@ndiscovered.com)
- Caladea, by Carolina Giovagnoli and Andrés Torresi (Huerta Tipográfica). `caladea/`: Caladea-Regular.ttf, -Bold, -Italic, -BoldItalic.
  Copyright 2012 The Caladea Project Authors (https://github.com/huertatipografica/Caladea)
- Cinzel, by Natanael Gama. `cinzel/`: Cinzel[wght].ttf.
  Copyright 2020 The Cinzel Project Authors (https://github.com/NDISCOVER/Cinzel)
- Comfortaa, by Johan Aakerlund. `comfortaa/`: Comfortaa[wght].ttf.
  Copyright 2011 The Comfortaa Project Authors (https://github.com/alexeiva/comfortaa), with Reserved Font Name "Comfortaa".
- Eagle Lake, by Astigmatic. `eaglelake/`: EagleLake-Regular.ttf.
  Copyright (c) 2012, Brian J. Bonislawsky DBA Astigmatic (AOETI) (astigma@astigmatic.com), with Reserved Font Names "Eagle Lake"
- Gloria Hallelujah, by Kimberly Geswein. `gloriahallelujah/`: GloriaHallelujah.ttf.
  Copyright (c) 2010, Kimberly Geswein (kimberlygeswein.com)
- Great Vibes, by Robert Leuschke. `greatvibes/`: GreatVibes-Regular.ttf.
  Copyright 2015 The Great Vibes Pro Project Authors (https://github.com/googlefonts/great-vibes)
- Iceberg, by Cyreal. `iceberg/`: Iceberg-Regular.ttf.
  Copyright (c) 2011, Cyreal (www.cyreal.org), with Reserved Font Name "Iceberg".
- IM Fell English, by Igino Marini. `imfellenglish/`: IMFeENrm28P.ttf, IMFeENit28P.ttf.
  Copyright (c) 2010, Igino Marini (mail@iginomarini.com)
- IM Fell English SC, by Igino Marini. `imfellenglishsc/`: IMFeENsc28P.ttf.
  Copyright (c) 2010, Igino Marini (mail@iginomarini.com)
- Lugrasimo, by The DocRepair Project and Astigmatic. `lugrasimo/`: Lugrasimo-Regular.ttf.
  Copyright 2023 The Lugrasimo Project Authors (https://github.com/docrepair-fonts/lugrasimo-fonts).
- Orbitron, by Matt McInerney. `orbitron/`: Orbitron[wght].ttf.
  Copyright 2018 The Orbitron Project Authors (https://github.com/theleagueof/orbitron), with Reserved Font Name: "Orbitron"
- Pirata One, by Rodrigo Fuenzalida and Nicolas Massi. `pirataone/`: PirataOne-Regular.ttf.
  Copyright (c) 2012, Rodrigo Fuenzalida, Nicolas Massi (www.taip.com.ar / abc.taip.com.ar), with Reserved Font Name 'Pirata'
- Snowburst One, by Annet Stirling. `snowburstone/`: SnowburstOne-Regular.ttf.
  Copyright (c) 2012 by Sorkin Type Co (www.sorkintype.com), with Reserved Font Name 'Snowburst'
- Tapestry, by Robert Leuschke. `tapestry/`: Tapestry-Regular.ttf.
  Copyright 2003-2021 The Tapestry Project Authors (https://github.com/googlefonts/tapestry)
- Underdog, by Sergey Steblina and Jovanny Lemonad. `underdog/`: Underdog-Regular.ttf.
  Copyright (c) 2012, Sergey Steblina (sergey@steblina.com), Jovanny Lemonad (lemonad@jovanny.ru), with Reserved Font Name 'Underdog'
- UnifrakturMaguntia, by j. 'mach' wust. `unifrakturmaguntia/`: UnifrakturMaguntia-Book.ttf.
  Copyright (c) 2010, j. 'mach' wust, with Reserved Font Name UnifrakturMaguntia. Copyright (c) 2009, Peter Wiegel.

The text of the SIL Open Font License 1.1 follows. The `OFL.txt` file of
each font has the same license text after its copyright notice.

```text
-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded,
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```

## TownGeneratorOS

The town maps of inkland come from a port of TownGeneratorOS by Watabou
(<https://github.com/watabou/TownGeneratorOS>), the open version of the
Medieval Fantasy City Generator. The port is in
`crates/inkland-core/src/town/`. It follows the generator of the original
in Rust: the Voronoi patches, the walls and gates, the streets, the wards,
and their buildings. It does not include the drawing code, the user
interface, or the assets of the original.

TownGeneratorOS is licensed under the GNU General Public License, version 3.
The full text of the license is in `LICENSE`.

## Rust crates

The `inkland` binary includes Rust crates under MIT, Apache-2.0, ISC,
BSD-2-Clause, BSD-3-Clause, Unicode-3.0, and Zlib licenses. The desktop app
also includes Slint under its GPL-3.0-only option. `deny.toml` lists the
licenses that the build accepts. To list each crate with its license, run
`cargo deny list`.
