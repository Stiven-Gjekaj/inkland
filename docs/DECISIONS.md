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
  travel days.
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
