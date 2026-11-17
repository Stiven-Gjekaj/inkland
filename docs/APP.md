# The inkland desktop app

The desktop app shows the map of a config and edits the config. It uses
the same library and the same edit commands as the CLI. Each edit is a
command on the config, and the app makes the world again from the config.
Thus the CLI can replay the edits of the app.

## Run the app

    cargo run --release -p inkland-app -- path/to/config.json

Without a file, the app starts with a new map. Use File > Open to open a
config file.

On Linux, the app needs these system packages to build:
`libfontconfig1-dev libfreetype-dev libxkbcommon-dev libxkbcommon-x11-dev
libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev
libgl1-mesa-dev`. The CLI does not need them.

To check that the app starts and draws a map, set
`INKLAND_APP_SNAPSHOT=FILE.png`. The app writes an image of its window to
the file when the first map shows, and then stops.

## The window

- The map view is in the middle. It shows the print SVG of the selected
  variant.
- The tools, the variant list, and the layers are on the left.
- The pin editor and the print preview are on the right.
- The status bar gives the pointer position in config units, and the
  height, the land or water, and the biome of the cell under the pointer.
  It also gives the last message.

The world is made again on a worker thread after each edit. The map view
shows "Generating..." while this occurs.

## The map view

- Drag with the select tool on an empty part of the map to pan. A drag
  with the right or the middle button pans with each tool.
- Use the mouse wheel to zoom. The point under the pointer stays where it
  is. The image is sharp again a short time after the zoom.
- View > Show the Full Page fits the page into the view.

## The tools

- **Select and move pins.** Click a point pin to select it. Drag a pin to
  move it. The app sends `move_pin`.
- **Add point pin.** Click the map to add a point pin of the kind in
  "New pin kind". The app sends `add_pin`. The id is `pin-1`, `pin-2`,
  and so on.
- **Heightmap brush.** Click the map to add a brush stroke. Drag to add a
  stroke each time the pointer moves half the radius. The app sends
  `brush`. Set the radius (config units), the strength, and Raise or
  Lower in the brush panel.

Press Escape to go back to the select tool.

## The pin editor

The Pins tab lists the point pins of the config. Click a pin in the list to
select it. For the selected pin:

- Type a name and click Rename (or press Enter). The app sends `rename`.
  An empty name removes the name, so that the name is generated.
- Click Remove. The app sends `remove_pin`.

## Variants and layers

- The Variant list selects the variant that the map view shows.
- A layer check box shows or hides a layer. The app sets `/render/layers`
  with a `set` command, so the change is in the config and in the edit
  script.

## The print preview

The Print preview tab lists each check of the report of the shown variant,
with PASS or FAIL. A failed check gives its count.

## Undo, redo, and the edit script

- Undo and Redo are buttons and items of the Edit menu.
- The shortcuts are Ctrl+Z (undo), Ctrl+Shift+Z and Ctrl+Y (redo). On
  macOS, use Cmd.
- The app refuses an edit that makes the config invalid. The config does
  not change, and the status bar gives the error.

## Files

- File > Save writes the config to its file. File > Save As writes it to a
  new file.
- File > Save Edit Script writes the history as an edit script:
  `{"version": 1, "commands": [...]}`. The script starts at the config that
  File > Open read. Replay it with the CLI:

      inkland edit --config config.json --script edits.json -o edited.json
      inkland edit --world world.json --script edits.json -o new-world.json

  The result is the same config and the same `world.json` as in the app.

## License

The app uses Slint under the Slint Royalty-free License 2.0. Help > About
shows the AboutSlint widget, as this license requires. The inkland code is
MIT OR Apache-2.0.
