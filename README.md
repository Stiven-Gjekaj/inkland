# inkland

inkland is a deterministic fantasy map engine in Rust.
It makes print-ready black-and-white SVG maps from a JSON config.
You can pin places, regions, rivers, and roads.
The engine fits the world around the pins and never moves them.
The same seed and config give the same bytes on each machine.

The main use is the maps of a D&D book.
A chapter can cite a fact from a map, and a machine can check that fact.

## Status

Phase 0 (research and plan) is complete. No code exists yet.

- [Specification](docs/SPEC.md)
- [Plan](docs/PLAN.md)
- [Architecture of the original program](docs/ARCHITECTURE.md)
- [Decisions](docs/DECISIONS.md)

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
