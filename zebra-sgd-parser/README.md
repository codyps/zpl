# zebra-sgd-parser

Original parser for Zebra Set/Get/Do (SGD) requests and SGD JSON. This crate
contains no transport or execution policy. `zebra-sgd` remains the read-only
network client; `zebra-simulator` uses this parser to implement its server.

```rust
use zebra_sgd_parser::{parse_sgd, parse_json_prefix, Command};

let get = parse_sgd(b"! U1 getvar \"device.location\"\r\n")?;
assert!(matches!(get, Command::Get { .. }));
let (batch, consumed) = parse_json_prefix(
    br#"{}{"device.location":"desk", "device.friendly_name":null}"#,
)?.expect("complete JSON request");
# Ok::<(), zebra_sgd_parser::ParseError>(())
```

`parse_sgd` accepts one complete line with optional CRLF. Operands are quoted
literal strings; JSON escaping is specific to JSON. Extra operands, missing
quotes, empty names, invalid UTF-8, and control characters are errors.

`parse_json_prefix` returns `None` for incomplete input and the byte count of
one complete request for coalesced streams. Requests use Zebra's `{}` introducer;
null values become gets and strings become sets. It preserves operation order
and rejects duplicate keys, non-object requests, and other value types. Branch
and report names are parsed like any other name; execution belongs to callers.
JSON does not invent an encoding for `do` commands. Bound stream buffering in
the caller (the simulator caps requests at 1 MiB).

References: [Zebra SGD command structure](https://docs.zebra.com/us/en/printers/software/zpl-pg/about-sgd-printer-commands.html)
and [Link-OS Programming Guide Addendum](https://techdocs.zebra.com/link-os/2-13/webservices/content/link-os-pg-addendum-en.pdf),
P1058886-001, pp. 25–26, examples 1–5.

```sh
cargo test --locked -p zebra-sgd-parser
```
