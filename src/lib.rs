use bytes::Bytes;

pub struct Zpl {}

enum Commands {
    /// Specify Font for use in text field
    A,
    /// `A@`, specify font with full name for use in text field
    Aat,

    /// Aztec barcode
    B0,
    /// Code 11 (USD-8) barcode
    B1,
    B2,
    B3,
    B4,
    B5,
    B7,
    B8,
    B9,
    BA,
    BB,
    BC,
    BD,
    BE,
    BF,
    BI,
    BJ,
    BK,
    BL,
    BM,

    XA,
    XB,
    XF,
    XG,
    XS,
    XZ,
}

pub struct ParseContext {
    // `CC` changes this
    prefix: u8,
}

impl Default for ParseContext {
    fn default() -> Self {
        Self { prefix: b'^' }
    }
}

impl ParseContext {
    pub fn next_token(&mut self, bytes: &mut Bytes) -> Result<Option<Command>, eyre::Report> {
        // all commands start with '^', return Err if it's not found
        if let Some(first) = bytes.first() {
            if first != self.prefix {
                // TODO: location info
                return Err(eyre::eyre!(
                    "Invalid command, expected prefix '{}' found '{}'",
                    self.prefix,
                    first
                ));
            }
        } else {
            return Ok(None);
        }

        let rem = bytes[1..].iter();

        if let Some(c1) = rem.next() {
            match c1 {
                b'X' => {
                    if let Some(c2) = rem.next() {
                        match c2 {
                            b'A' => {
                                bytes.advance(3);
                                return Ok(Some(Command::XA))
                            }
                            b'B' => {
                                bytes.advance(3);
                                return Ok(Some(Command::XB))
                            }
                            b'F' => {
                                bytes.advance(3);
                                return Ok(Some(Command::XF))
                            }
                            b'G' => {
                                bytes.advance(3);
                                return Ok(Some(Command::XG))
                            }
                            b'S' => {
                                bytes.advance(3);
                                return Ok(Some(Command::XS))
                            }
                            b'Z' => {
                                bytes.advance(3);
                                return Ok(Some(Command::XZ)),
                            }
                            _ => {
                                return Err(eyre::eyre!(
                                    "Invalid command, prefix `^X` needs suffix 'A', 'B', 'F', 'G', 'S', or 'Z' found '^X{}'",
                                    c2
                                ));
                            }
                        }
                    } else {
                        // Truncated command?
                        return Ok(None);
                    }
                }
                b'


            }
        } else {
            todo!()
        }

        Ok(None)
    }
}
