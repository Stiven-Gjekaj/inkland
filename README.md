# inkland

inkland is a deterministic fantasy map engine in Rust.
It makes print-ready black-and-white SVG maps from a JSON config.
You can pin places, regions, rivers, and roads.
The engine fits the world around the pins and never moves them.
The same seed and config give the same bytes on each machine.

The main use is the maps of a D&D book.
A chapter can cite a fact from a map, and a machine can check that fact.

## Status

Milestone 5 is complete. The tool makes terrain, climate, biomes, rivers,
cultures, places, states, and provinces around pinned places, regions, and
rivers. It writes a black-and-white SVG page, a world file, a report, and a
PNG preview. A pinned place stays at its exact position, on land. A pinned
region gets heights in its range. A pinned river passes its points in order,
flows downhill, and ends at its mouth. Roads, labels, and overlays come in
later milestones.

```sh
cargo run -p inkland-cli -- generate --config fixtures/golden/society-book-layout.json --out-dir out --preview out/preview.png
cargo run -p inkland-cli -- generate --config fixtures/configs/full.json --dry-run
cargo run -p inkland-cli -- schema
cargo run -p inkland-cli -- version
```

`generate` writes these files in the output directory:

- `world.json`: the base world that all variants share.
- `<variant>.svg`: the map page of each variant.
- `<variant>.report.json`: the report of each variant.
- With `--preview FILE.png`: a PNG preview, 1024 pixels on the longer side.
  With more than one variant, the file name gets the variant name.

`generate --dry-run` checks the config and the pins, prints a report, and
writes nothing.

All output is JSON on stdout. An error has a stable code and a JSON Pointer
to the bad value. See [the error codes](docs/ERRORS.md).

- [Specification](docs/SPEC.md)
- [Plan](docs/PLAN.md)
- [Architecture of the original program](docs/ARCHITECTURE.md)
- [Decisions](docs/DECISIONS.md)
- [Error codes](docs/ERRORS.md)

## Attribution

inkland is a rebuild of
[Azgaar's Fantasy Map Generator](https://github.com/Azgaar/Fantasy-Map-Generator)
by Max Haniyeu (Azgaar), under the MIT license.
inkland re-implements the algorithms. It does not copy the code.
See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## License

inkland is licensed under either of these, at your option:

- MIT license ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
