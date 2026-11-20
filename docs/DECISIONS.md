# Decisions

This file records the choices that the human makes.

## 2026-11-16

- The project name is `inkland`.
- The crates use the prefix `inkland-`. The CLI binary is `inkland`.
- The project license is `MIT OR Apache-2.0`.
- The project keeps the MIT notice of Azgaar's Fantasy Map Generator.
- The desktop app uses the Slint royalty-free license.
  The app shows the "Made with Slint" notice.
- The repository is public on GitHub as `Stiven-Gjekaj/inkland`.
- The agent does not push if the human does not ask.

## 2026-11-16: answers to the Phase 0 questions

### Pinned seas

- A pinned `sea` region must connect to the ocean by default.
- `"landlocked": true` on a `sea` or `lake` region permits an inland sea or a
  large lake.
- The book needs one sea: the Western Sea. It covers the full west edge
  (x 0 to 20), so it is the ocean.

### Pinned rivers

- A river has no fixed height. The tool calculates the river heights from the
  heights of the pinned areas, so that each river flows downhill.
- A river source can be an area. A river can pass through a pinned place. A
  river mouth can be a point, a place, or an area that is not sea.
- The book rivers:
  - The Dunn starts in the Grey Teeth, passes Kingsholm, and reaches the sea
    north of Corrantey.
  - The Brann starts in the Wolds, passes Brannock's Ford, and ends in the
    Low Fen.
  - The Corrin and the Tey reach the sea at Corrantey.
- The book area heights: the Grey Teeth are high, the Wolds are low hills,
  the Low Fen is at sea level or near it.

### Features that inkland does not make

- Journeys: no. The report gives the road lengths between places, for
  example "road A to B: 71 miles". The book uses these lengths to check the
  travel days. The report also gives the travel days on each pinned road
  for each land transport of `world.transports` (issue 33).
- Coats of arms: not now. Later, possibly, for the four factions (Gaunt, the
  Church, Corrantey, the Hush). The book build can draw arms itself.
- Economy, military, zones: no. The story fixes these.

### Page sizes and fonts

- Full page: 167 x 250 mm. Half page: 167 x 120 mm.
  The book is A4. With the 25 mm inside margin of Book One, the text area is
  171 mm wide.
- Map labels and titles: `font-family="IM Fell English"`, with the family
  name exactly so.
- Small capitals, for example region names: `IM Fell English SC`.
- All other text (scale bar, compass letters, grid, notes): Caladea (serif)
  or Inter (sans).
- inkland does not use EB Garamond.
- inkland does not embed fonts in the SVG. inkland does not change text to
  outlines. All text stays in real `<text>` elements.
- The book build loads the font files itself, so local builds and CI render
  the same.
- The font check of the book permits only Caladea, Inter and IM Fell English.
  Any other family fails the build.

## 2026-11-19: version numbers

- The work on the issues from the Maeris map (#23 to #45) is release
  `v1.2.0`. It adds new config options and changes the default look, so it
  is a minor version.
- A 3D view, in the desktop app and as an export of the terrain, is release
  `v2.0.0`. The two parts come in the same major version.

## 2026-11-20: the license is GPL-3.0-or-later

- inkland makes town maps with a port of TownGeneratorOS by Watabou.
  TownGeneratorOS is GPL-3.0, and a port of it is a derived work.
- So inkland changes from MIT OR Apache-2.0 to GPL-3.0-or-later.
- Releases up to v1.3.3 keep MIT OR Apache-2.0. The change applies from the
  next release.
- The desktop app uses Slint under its GPL-3.0-only option, not under the
  Slint Royalty-free License. So an app binary is GPL-3.0.
- Each commit of inkland has one author, so the author alone can change the
  license.
- The town links of level 1 (to watabou.github.io) stay. The town maps of
  the port draw offline, in inkland.

## 2026-11-20: version 2 is the GNU GPL and the town maps

- The change of the license to GPL-3.0-or-later, the town links, and the
  town maps are release `v2.0.0`.
- The 3D view (an app view and an export) is no longer version 2. It comes
  in a later major version.
