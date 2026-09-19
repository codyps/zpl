# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/codyps/zpl/releases/tag/zpl-v0.1.0) - 2026-09-19

### Other

- Add an explicit specification profile and default to ZD621
- Make printer preview departures selectable through a ZD621 profile
- Pin printer accuracy and correct shape and barcode rendering
- enforce consistent TOML formatting with Taplo
- Add RasterOutput destination trait and rasterize_into
- Fix workspace Clippy warnings and Rust formatting
- Update workspace dependencies and remove unused parser crates
- simplify rendering exports and group raster APIs
- Extract font sampling into zpl-font-extract crate
- Remove obsolete ZPL prototypes and fix macro warning
- License workspace crates under OSL-3.0
- Rename generic image comparison crate to raster-diff
- Add original local barcode encoders and decoder tests
- Add path-based rendering, captured font extraction, and PNG comparison
- Implement lossless ZPL command-stream parsing with reference coverage
- move things around, start zpl api
