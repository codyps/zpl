# Comparison with other ZPL parsers

Source inspection on 2026-09-14, pinned to the commits below. These are observations about the inspected parser paths, not a claim that any project implements every Zebra firmware behavior. The other projects were not used as authoritative conformance oracles.

## BinaryKits.Zpl (C#)

Commit: `25edadaa867472941b7cc0a84802a3e473b4a3b2`.

[`ZplAnalyzer.SplitZplCommands`](https://github.com/BinaryKits/BinaryKits.Zpl/blob/25edadaa867472941b7cc0a84802a3e473b4a3b2/src/BinaryKits.Zpl.Viewer/ZplAnalyzer.cs) walks a string with mutable caret/tilde characters, then dispatches to individual command analyzers. It removes vertical whitespace and normalizes changed prefixes back to `^`/`~`. Its splitter handles `CC`/`CT`, but does not read binary lengths or track `CD` in that path. [`DownloadObjectsZplCommandAnaylzer`](https://github.com/BinaryKits/BinaryKits.Zpl/blob/25edadaa867472941b7cc0a84802a3e473b4a3b2/src/BinaryKits.Zpl.Viewer/CommandAnalyzers/DownloadObjectsZplCommandAnaylzer.cs) converts the data field from hexadecimal.

**Takeaway:** explicit syntax state followed by command-specific interpretation is a reasonable separation. Its text normalization and prefix-only splitting do not meet our lossless binary-stream contract.

## zpl-toolchain (Rust)

Commit: `3da58518c1013fffd46d2147b0927a1d5b4aeab5`.

The [`parser`](https://github.com/trevordcampbell/zpl-toolchain/blob/3da58518c1013fffd46d2147b0927a1d5b4aeab5/crates/core/src/grammar/parser.rs) operates on `&str`, uses command tables/trie lookups, has distinct normal/field/raw-data modes, and tracks prefix/delimiter changes. Changes retokenize the remaining source. Its `parse_raw_data` path stops at a leader token; it does not consume a declared binary byte count there. Unknown commands receive diagnostics. The [design decision](https://github.com/trevordcampbell/zpl-toolchain/blob/3da58518c1013fffd46d2147b0927a1d5b4aeab5/docs/adr/0003-opcode-trie-and-parser-lookups.md) explains longest-match recognition for typed opcode parsing.

**Takeaway:** tables and a trie become useful when identifying every command and validating operands. Our narrower framer does not need them: it preserves ordinary commands without assigning typed meaning, and only interprets the few constructs that change boundaries. A text-only input type would exclude arbitrary download bytes.

## ZPLr (TypeScript)

Commit: `590f03c58c79663be37708cd97f8cff5eb0d797e`.

[`documentParser.ts`](https://github.com/le2ni/zplr/blob/590f03c58c79663be37708cd97f8cff5eb0d797e/src/core/documentParser.ts) tracks initial/final syntax, treats `A` and `A@` specially, and uses `binaryCommandEnd` for format-prefix `GF` and control-prefix `DY`. For B/C modes it takes counts from the same header fields we use. It also recognizes STX, ETX, and SI. Its malformed-header path returns to ordinary splitting, and a truncated binary length is clamped to the remaining string. [Tests](https://github.com/le2ni/zplr/blob/590f03c58c79663be37708cd97f8cff5eb0d797e/src/core/documentParser.test.ts) explicitly exercise these choices.

**Takeaway:** this is the closest framing design to ours. The control-code alternatives exposed a real omission in our initial work; they were confirmed against Zebra pages 204, 370, and 375 and added. We deliberately report uncertain binary boundaries rather than silently clamping or interpreting payload bytes as commands.

## Resulting design

Keep the byte-oriented, borrowed-slice state machine. It preserves the source, handles explicit syntax changes in place, and treats binary payloads as opaque counted regions. Do not add a parser-generator dependency just to find command boundaries. Retain a separate reference inventory and tests so accepting an arbitrary mnemonic is not confused with implementing its semantics.

The comparison supports this architecture, but does not prove universal compatibility. Our stricter malformed-download behavior is an intentional API choice. Firmware-specific ambiguities, unknown binary formats, and rendering/parameter validation remain outside the [documented contract](parser-coverage.md).

## External fixture check

The new `zpl-parse` example was run against every `.zpl`/`.zpl2` file in these pinned checkouts, without executing their commands:

| Source | Files framed successfully | Input bytes |
| --- | ---: | ---: |
| BinaryKits.Zpl | 61 / 61 | 60,615 |
| zpl-toolchain | 5 / 5 | 3,976 |
| ZPLr | 5 / 5 | 3,736 |

[The manifest](zpl-corpus-manifest.tsv) records paths, revisions, sizes, and SHA-256 hashes. This checks our parser against independent example inputs. It is **not** a differential run of the other parsers, nor proof that labels render identically. The foreign sources and dependencies are not required by the offline Cargo test suite.
