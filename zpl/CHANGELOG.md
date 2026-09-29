# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/codyps/zpl/releases/tag/zpl-v0.1.0) - 2026-09-29

### Added

- *(render)* add ZQ610 Plus preview profile and native regressions

### Fixed

- *(render)* match the SurePost printer preview
- add a description
- *(render)* preserve barcode origins around interpretation text
- *(pdf417)* match standalone numeric compaction transitions
- *(render)* quantize Code 39 wide elements before placement
- *(render)* match ZD621 ellipse scan conversion exactly

### Other

- *(release)* add repository and documentation metadata
- *(tests)* skip unused barcode report hashes
- *(tests)* accelerate raster accuracy checks
- *(barcodes)* complete model-specific preview accuracy audit
- specific publish registry in Cargo.toml causes release-plz to choke, just use default
- separate crate API docs from README doctests
- add tested crate quick-start READMEs
- *(release)* restrict published crates
- *(zpl)* exclude asset provenance from crate
- *(zpl)* update sanitized capture hash
- *(zpl)* narrow the published crate
- Update dependencies and CI tools to current compatible releases
- match encoding-specific printer ESC and DEL behavior
- reproduce native tab stops and block spacing
- gate the complete native conformance corpus
- preserve unstable native empty QR evidence
- cap native bitmap fonts and oversized barcode captions
- capture native shipping and typography font sizes
- preserve QR magnification in subsequent barcode width
- reproduce printer automatic QR mask selection
- recognize supported resident fonts in fallback checks
- match layout-specific text control processing
- match retail-caption placement at label edges
- match Code 93 control-substitute captions
- match native text-field NUL processing
- implement legacy character-image remapping
- decode documented Windows code pages
- match native Unicode composition and formatting
- separate resident S from graphic symbols
- add native resident fonts T U and V
- scan active edges and render long text fields accurately
- pin aligned-width barcode printer captures
- capture native font 0 dimension strikes
- honor scalable font minimum and whole-dot FO baseline
- capture font zero defaults and field override sizes
- match blank rows caused by oversized field-block spaces
- preserve repeated spaces in printer field blocks
- supply missing 28x14 automatic-hyphenation glyphs
- fix common text sizes and field-block paragraph layout
- make font-0 anchor controls pixel exact at 40x22
- add native Q and R preset fonts
- add native preset P font and rotated origin compatibility
- match ZD621 unavailable resident font fallback
- concatenate numbered fields and extract substrings
- support serialization masks on the initial label
- resolve inline numbered fields before drawing
- support advanced text properties and bidi layout
- add Unicode font strikes and advanced-text defaults
- support initial serial-number fields
- normalize retail barcode data before encoding
- support bounded text blocks with ZD621 compatibility
- support field direction inside text blocks
- implement field direction and measured ZD621 anchors
- support page mirroring with an independent preview option
- match ZD621 extended QR Model 1 versions
- support QR structured append and mixed manual segments
- support QR manual Kanji encoding
- match printer field-block limits and overflow placement
- implement field-block soft-hyphen markers
- match native backslash glyphs and block alignment
- add captured cent glyphs to resident font strikes
- decode literal field-block backslash escapes
- match printer text origins and edge clamping
- quantize centered block lines before rotation
- reproduce narrow field-block fallback
- match graphic origins and nominal dimensions
- honor graphic box minimum and default dimensions
- implement automatic field-block word hyphenation
- match printer rounding of justified word positions
- reproduce ZD621 graphic symbols and field transitions
- sample the graphic-symbol face with visible probes
- support the complete resident H font
- support the complete resident G font
- support the complete resident E font
- support field-block overflow and explicit line endings
- add resident F and correct explicit bitmap captions
- Support resident fonts B/C and match bitmap text placement
- Preserve short-glyph padding at rotated barcode edges
- Match rotated barcode height at nonpositive label edges
- Match printer barcode ink at negative label edges
- Support Code 93 checksum interpretation and printer formatting
- Match linear barcode captions and Code 11 element widths
- Use the captured FT boundary across linear barcodes
- Match ZD621 retail barcode captions and FT boundaries
- Match ZD621 Code 39 interpretation and baseline placement
- *(qr)* pin printer mask invariance across position and scale
- make printer QR mask analysis reproducible
- *(render)* add printer QR byte-mode selection controls
- *(render)* isolate QR encoding from printer mask selection
- *(render)* pin 200 additional printer ellipse sizes
- Improve ZD621 unequal-axis ellipse scan conversion
- Pin independent ellipse and QR printer accuracy controls
- Use the printer circle curve for equal-axis ellipses
- Match ZD621 circle scan conversion
- Match ZD621 rounded box raster curves
- Honor barcode power-up defaults and omitted BY operands
- Match TLC39 additional-field sizing and numeric transitions
- Match TLC39 short numeric compaction
- Match ZD621 rounded-box inner geometry
- Match ZD621 DataBar finder separator departures
- Mirror DataBar Expanded separators with their data rows
- Optimize QR automatic data across encoding segments
- Match ZD621 DataBar long-weight preview encoding
- Match TLC39 compaction and printer component geometry
- Preserve printer curve controls and document remaining accuracy gaps
- Match MaxiCode compaction and explicit ZD621 preview behavior
- Implement DataBar Expanded AI compression
- Implement Data Matrix compaction and printer escape handling
- Implement Aztec text compaction and correct symbol sizing
- Correct DataBar retail dimensions and UPC-E input handling
- Match composite barcode dimensions to the ZD621 preview
- Match resident fonts, barcode captions and justified text to the printer
- Correct CODABLOCK rows and printer preview text layout
- Support remaining barcode modes and validation commands
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
