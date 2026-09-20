# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/codyps/zpl/releases/tag/zpl-font-extract-v0.1.0) - 2026-09-20

### Other

- match native Unicode composition and formatting
- separate resident S from graphic symbols
- add native resident fonts T U and V
- scan active edges and render long text fields accurately
- capture native font 0 dimension strikes
- support single-column pages for wide glyphs
- honor scalable font minimum and whole-dot FO baseline
- capture font zero defaults and field override sizes
- supply missing 28x14 automatic-hyphenation glyphs
- fix common text sizes and field-block paragraph layout
- make font-0 anchor controls pixel exact at 40x22
- add native Q and R preset fonts
- add native preset P font and rotated origin compatibility
- add Unicode font strikes and advanced-text defaults
- sample the graphic-symbol face with visible probes
- preserve valid advancing blank glyphs
- Match resident fonts, barcode captions and justified text to the printer
- Add an explicit specification profile and default to ZD621
- simplify rendering exports and group raster APIs
- Extract font sampling into zpl-font-extract crate
