# Command-stream parser coverage

## Contract

`zpl::parse::ParseContext` frames a **complete byte buffer** into borrowed, unmodified command slices. Joining `Element::as_bytes()` reproduces every input byte on success, including whitespace, unknown commands, non-UTF-8 bytes, and binary payloads. Leading non-command data is returned separately. Inter-command whitespace belongs to the preceding command; single-byte control-code alternatives are separate elements.

This is command-stream parsing, not typed parameter validation, decompression, rendering, or printer-state emulation. Recognition of a mnemonic does not prove its operands are valid or that a particular firmware supports it. The unfinished internal `Command` enum is not used as an allowlist.

## Framing rules

| Input | Behavior |
| --- | --- |
| Ordinary commands, including network and RFID | Preserve bytes through the next active command boundary. No mnemonic allowlist. |
| `^A`, `^A@`, font selectors, omitted operands | Retain raw font command; do not split its mnemonic. |
| `^CC` / `~CC`, `^CT` / `~CT`, `^CD` / `~CD` | Consume exactly one following syntax byte, even if it is whitespace or a prefix. Apply the change immediately. |
| STX / ETX / SI | Return single-byte alternatives for `^XA` / `^XZ` / `^FS` as `ControlCharacter`. |
| `^GFB` / `^GFC` | Consume the transmitted byte count (`b`), not the decoded raster size (`c`). Payload prefixes and control codes are inert. |
| `~DY` modes B / C | Consume the declared binary byte count (`t`), including any apparent commands in the payload. |
| ASCII graphics and legacy font downloads | Preserve encoded bytes. An active command boundary can abort an ASCII download. |
| `:B64:` / `:Z64:` download data | Keep the encoded body, terminating colon, and four CRC digits together. Do not mistake decoded-size fields for wire lengths. |
| `~DB` encoded glyph subfields | Preserve multiple encoded subfields within the font command. |
| `^FD`, `^FV`, `^FX`, `^FH` escapes | Frame raw input; do not turn decoded field text into commands. Comments end at a command boundary, not exclusively `^FS`. |

The download length calculations use checked arithmetic and never allocate from the declared size. Missing syntax operands, incomplete binary headers, invalid lengths, truncated binary data, and incomplete encoded envelopes produce an error with a byte offset. Iteration stops at the failed command; its remainder is not reinterpreted as text commands. A malformed graphic cannot thereby invent a later prefix change.

## Initial and final syntax

```rust
use zpl::parse::{ParseContext, Syntax};

let input = b"/XA/FO10;20/FDHello/FS/XZ";
let syntax = Syntax {
    format_prefix: b'/',
    control_prefix: b'!',
    delimiter: b';',
};
let mut parser = ParseContext::with_syntax(input, syntax);
let elements = parser.by_ref().collect::<Result<Vec<_>, _>>()
    .expect("complete, well-framed ZPL");
let final_syntax = parser.syntax();
```

`from_bytes` starts with `^`, `~`, and `,`. Use `with_syntax` when printer configuration differs. State persists across labels. A caller may carry final syntax to the next **complete** stream; this is not an incremental network-chunk parser. If a download spans network reads, assemble its complete bytes before parsing.

## Evidence and limits

The reference-inventory test checks boundaries for all 223 command spellings plus the appendix's `~DL`, under default and remapped syntax. Those specimens exercise command framing; many intentionally omit operands. Dedicated tests cover binary payloads containing every possible byte, changes hidden inside binary data, truncation at each payload offset, encoded envelopes, syntax changes, single-byte alternatives, and all existing repository fixtures. Deterministic arbitrary-byte tests check progress, round trips, and panic freedom over sampled inputs.

These checks establish the supported framing rules, not a proof that every possible firmware input is handled. In particular:

- ZBI programs, SGD, EPL, CPCL, printer responses, and transport packet envelopes are separate languages/protocols.
- Firmware resets, stored formats, or external SGD settings may alter state in ways a standalone byte buffer cannot determine.
- Unknown commands are retained; undocumented binary payload conventions cannot be inferred from a mnemonic alone.
- Prefix/operand collisions and identical format/control prefixes can be ambiguous. With identical prefixes, classification prefers format commands. Use distinct printer-compatible syntax characters.
- B64/Z64 CRC values, base64 padding, decoded sizes, proprietary compressed-binary content, and command parameter ranges are not validated.
- Lowercase text is preserved, but special framing follows the guide's uppercase command spellings.

## Run checks

```sh
direnv exec . cargo test -p zpl
direnv exec . cargo run -p zpl --example zpl-parse -- test-data/cc.zpl
```

The inspection example reads files and reports framing errors without executing any ZPL commands.
