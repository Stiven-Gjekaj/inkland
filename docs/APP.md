# The inkland desktop app

The desktop app shows the map of a config and edits the config. It uses
the same library and the same edit commands as the CLI. Each edit is a
command on the config, and the app makes the world again from the config.
Thus the CLI can replay the edits of the app.

The app has the look and the controls of the web app of Azgaar's Fantasy
Map Generator (FMG, MIT): the map fills the window, a menu with tabs is at
the top left, and the editors are dialogs over the map.

## Run the app

    cargo run --release -p inkland-app -- path/to/config.json

Without a file, the app starts with a new map. Use Open to open a config
file.

On Linux, the app needs these system packages to build:
`libfontconfig1-dev libfreetype-dev libxkbcommon-dev libxkbcommon-x11-dev
libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev
libgl1-mesa-dev`. The CLI does not need them.

## A window image for checks

Set `INKLAND_APP_SNAPSHOT=FILE.png`. The app writes an image of its window
to the file when the first map shows, and then stops.

- `INKLAND_APP_SNAPSHOT_OPEN=TAB/TOOL,TOOL` opens a tab of the menu and
  the dialogs or tools of the Tools tab before the image. TAB is
  `layers`, `style`, `options`, `tools`, `about`, or `closed`. A TOOL is
  an id of the Tools tab, for example `burgs`, `regions`, `diplomacy`,
  `economy`, `goods`, `journeys`, `markers`, `markets`, `measurers`,
  `military`, `notes`, `stability`, `zones`, or `print`.
  Example: `INKLAND_APP_SNAPSHOT_OPEN=tools/burgs`.
- `INKLAND_APP_SNAPSHOT_TOWN=ROW` opens the Town Map dialog of a burg
  before the image. ROW is the id of a pin or `place:<n>`.
- `INKLAND_APP_SNAPSHOT_POINTER=x,y` puts the pointer at a point of the
  world (config units). The status bar and the Cell Details dialog then
  show the facts of that cell.

## The window

- The map fills the window. It shows the print SVG of the selected
  variant.
- The menu is at the top left. Click the arrow to open or close it. Drag
  its top edge to move it. The tabs are Layers, Style, Options, Tools, and
  About.
- The buttons under each tab are New Map, Open, Save, Save As, Script
  (Save Edit Script), Undo, Redo, and Zoom out.
- The editors are dialogs. Drag the title bar of a dialog to move it.
  Click a dialog to put it on top. Click the cross to close it.
- The status bar at the bottom gives the pointer position in config units,
  and the height, the land or water, the biome, the state, the culture,
  the religion, the province, and the nearest place of the cell under the
  pointer. It also gives the last message.

The world is made again on a worker thread after each edit. The map view
shows "Generating..." while this occurs.

## The map view

- Drag an empty part of the map to pan. A drag with the right or the
  middle button pans with each tool.
- Use the mouse wheel to zoom. The point under the pointer stays where it
  is.
- Zoom out (or the key 0) fits the page into the view.

## The Layers tab

- Layers preset: the FMG presets, for example "Political map" and
  "Physical map". A preset is one edit of `/render`. It does not change
  the coastline and the inkland layers (pinned areas and overlay items).
- Variant: the variant that the map shows.
- A button for each FMG layer. Click a button to show or hide the layer.
  A map layer is in `/render/layers`. The scale bar, the wind rose, and
  the grid are in `/render/extras`. A grey button is a layer that the
  renderer does not have.

The list of the layers, the presets, the style presets, the render modes,
and the tools is in `crates/inkland-app/src/catalog.rs`. A layer button
is available when the `LayerName` type of the config accepts its name. A
style preset is available when the config accepts it at `/render/style`.
Thus a layer or a style preset that the renderer adds becomes available
without a change of the app.

## The Style tab

The style preset, the render mode, the four font families, the font sizes
of the label kinds, the smallest line width, the scale bar, the wind rose,
the grid step, and the map title.

## The Options tab

The map seed, the map size (frame width and height), the number of cells,
the heightmap template, the sea level, the number of cultures, the culture
set, the number of states, the number of burgs (towns, or `auto`), the
number of religions, and the latitude range. Each change is a `set`
command, so undo and the edit script have it. "New" (and New Map, and F2)
sets a new random seed.

## The Tools tab

The buttons open the editors, in the groups of FMG. A grey button is an
FMG tool that inkland does not have yet.

- **Burgs.** The point pins (bold) and the places that generation adds.
  Select a pin to change its name, its type, and the port flag, or to
  remove it. "Pin" makes a generated place a pin at the same position,
  with the same name. "Add on the map" adds a pin at each click.
  "Draw town map" opens the Town Map dialog with the offline town map of
  the last row that you clicked: its walls, wards, streets, and buildings,
  from the port of TownGeneratorOS. "Colour" switches between the colours
  of the original and black and white, and "Save SVG" writes the map. The
  CLI draws the same map with `inkland town`. "Open online" opens the town
  on watabou.github.io, in the web browser: the city generator for a
  capital, a city, or a town, and the village generator for a village or
  a hamlet, as in Azgaar's Fantasy Map Generator.
- **Heightmap.** The template, the sea level, and the brush. Click or drag
  on the map to raise or lower the land. The list shows the brush strokes;
  "Remove" removes one stroke.
- **Regions.** The pinned areas. Select an area to change its name, kind,
  height range, and soft edge, or to remove it. "Draw" draws a new area
  as a box or a circle (drag) or a polygon (click the points; click the
  first point, double-click, or press Enter to close it).
- **Rivers.** The pinned rivers. "Draw" makes a new river: click the
  source, the points that the river passes, and the mouth. A click near a
  burg pin uses the burg. Double-click or press Enter at the mouth.
- **Routes.** The pinned roads. "Draw" makes a new road: click a burg pin,
  the points that the road passes, and the end burg pin. Select a road to
  set the wanted length and the tolerance.
- **Labels.** The font size of each label kind, and the label options
  (hide, size) of each pinned item in the shown variant. The options of an
  item go in the overlay of the variant. If the variant has no overlay,
  the app adds an overlay with the name of the variant.
- **Units.** The distance unit, the cell width hint, the scale bar, the
  wind rose, and the grid step.
- **Variants.** The variants (name, overlay, page size, margin, crop) and
  the overlays. Add a copy of a variant, remove a variant, add or remove
  an overlay.
- **Submap.** Drag a rectangle on the map to crop the shown variant, or to
  add a new variant that shows only the rectangle.
- **States, Cultures, Religions, Provinces.** The lists of the shown
  world, with the number of cells and burgs. These lists do not edit.
- **Emblems.** The coat of arms of a state, a province, or a burg of the
  shown world, with its image and a short text. "Set custom" writes the
  JSON text of the arms to the config: `coa` of a burg pin, or
  `state_coa` of the pin that is the capital of a state. Each change is a
  `set` command, so undo and the edit script have it. "Use generated"
  removes the custom arms. A province and a generated burg have no custom
  arms.
- **Diplomacy.** The relation of each state to the selected state, with
  the FMG relation color and a description, and the relations history
  (the chronicle). Click a state to see the relations to it.
- **Economy.** The economy of the selected state: the economy type, the
  people, the inequality, the trade balance, the day balance, the
  prosperity, the product, the wealth, the sectors, the taxes, the
  expenses, the main exports and imports, and the trade partners. The
  fields under the table set the sales tax, the poll tax, the market fee,
  the military budget, and the tariff (a share of the price, or
  `embargo`) of the state.
- **Goods.** The goods, with the color, the type (raw or made), the unit,
  the units made in a day, the units in stock in all markets, and the base
  price.
- **Journeys.** The journeys (type, start, end, distance, speed, total
  time) and the segments of the selected journey. "Draw" makes a new
  journey: click the stops, then double-click or press Enter. The field
  gives the name.
- **Markers.** The markers, with the name, the type, and the legend. Click
  a marker to read its full legend.
- **Markets.** The markets, with the color, the state, the cells, the
  burgs, the stock, the sales, the purchases, and the value. "Add on the
  map" adds a market: click the burg of its center. Select a market and
  click "Move" to move its center to another burg.
- **Measurers.** The measurers: the type, the number of points, and the
  length. Generation adds the default ruler. "Draw" makes a new measurer:
  click its points, then double-click or press Enter. The field gives the
  kind (`ruler`, `opisometer`, `route-opisometer`, or `planimeter`).
  Under the measurers, the legend boxes. "Draw" adds a legend box at a
  click. The field gives the title and the items, as
  `Title; #rrggbb text; #rrggbb text`.
- **Military.** The forces of each state by unit, the total with the
  crew, the people, the rate, the war alert, the upkeep, and the power.
  Under them, the regiments of the selected state. "Draw" adds a regiment
  at a click in a state. The field gives the troops as
  `Unit=people, Unit=people`. Select a regiment to move it, or to set its
  troops.
- **Notes.** The notes of the regiments, the markers, and the zones, and
  the text of the selected note.
- **Stability.** The parts of the stability of the selected state, the
  total, and the provinces, least stable first.
- **Zones.** The zones, with the type, the cells, the area, the people,
  and the hatch pattern of the fill. "Draw" makes a new zone: click its
  outline, then double-click or press Enter. The field gives the name and
  the type, as `Name; Type`.
- Each edit of these dialogs adds one entry to a list of the world part
  of the config (`world.markets`, `world.regiments`, `world.zones`, and
  the other lists). The entry finds its item by a point, so the edit comes
  back after each regeneration. Diplomacy, Military, Stability, and
  Economy share one select of the state.
- The Add group of the Tools tab starts the same draw tools: Market,
  Regiment, Zone, Journey, Measurer, and Legend.
- **Cells.** The Cell Details dialog. It shows the facts of the cell
  under the pointer, with the good of the deposit and the market.
- **Print.** The checks of the report of the shown variant, with PASS or
  FAIL.

The marks of the open editors show on the map: the outlines of the pinned
areas, the courses of the pinned rivers and roads, and the crop. The
selected item is red.

## Hotkeys

The hotkeys are the FMG hotkeys that make sense in the app. A hotkey does
not operate while a text field has the focus. On macOS, use Cmd in place
of Ctrl.

| Key | Action |
| --- | --- |
| Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y | Undo, redo |
| Ctrl+S, Ctrl+Shift+S, Ctrl+O | Save, Save As, Open |
| F2 | New map with a new seed |
| Tab | Open or close the menu |
| Escape | Stop the drawing and the tool; then close the dialogs and the menu |
| Enter | Finish the drawing |
| Delete | Remove the selected burg pin |
| Shift+H, T, Z, V, U, L, Q, S, C, R, P, E, W, Y | Heightmap, Burgs, Regions, Rivers, Routes, Labels, Units, States, Cultures, Religions, Provinces, Cells, Variants, Emblems |
| Shift+D, G, J, K, M, O | Diplomacy, Goods, Journeys, Markers, Military, Notes |
| Shift+1, Shift+4, Shift+5 | Add a burg, a river, a route |
| A layer key (for example V, F, I, L, /) | Show or hide the layer |
| + and -, 0 | Zoom in and out, show the full page |
| Arrow keys | Pan |
| [ and ] | Make the brush smaller or larger |

## Undo, redo, and the edit script

- The app refuses an edit that makes the config invalid. The config does
  not change, the fields show the values of the config again, and the
  status bar gives the error.
- Save writes the config to its file. Save As writes it to a new file.
- Script (Save Edit Script) writes the history as an edit script:
  `{"version": 1, "commands": [...]}`. The script starts at the config that
  Open read. Replay it with the CLI:

      inkland edit --config config.json --script edits.json -o edited.json
      inkland edit --world world.json --script edits.json -o new-world.json

  The result is the same config and the same `world.json` as in the app.
- A `world.json` keeps only the part of the config that makes the world.
  It does not keep the variants, the overlays, the render options, or the
  font directories. Thus `--world` refuses a script that edits one of
  them, with the error `EDIT_OUTSIDE_WORLD`. Replay such a script with
  `--config`. When you save such a script, the status bar tells you to
  use `--config`.

## License

The app uses Slint under the Slint Royalty-free License 2.0. The About tab
shows the AboutSlint widget, as this license requires. The inkland code is
MIT OR Apache-2.0. The look of the app follows FMG, which is MIT.
