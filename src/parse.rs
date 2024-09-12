//! Parse ZPL into a series of commands.
//!
//! Only examines the internals of commands that must be parsed to continue the parsing process,
//! does not validate the contents of commands that affect formatting or other printer state not
//! related to parsing.

/// Information we must track to parse the ZPL.
pub struct ParseContext<'a> {
    // `CC` changes this
    prefix_caret: u8,
    // `CT` changes this
    prefix_tilde: u8,

    input: &'a [u8],
    position: usize,
}

impl<'a> ParseContext<'a> {
    pub fn from_bytes(input: &'a [u8]) -> Self {
        Self {
            input,
            position: 0,
            prefix_caret: b'^',
            prefix_tilde: b'~',
        }
    }

    pub fn eat_whitespace(&mut self) {
        let mut iter = self.input[self.position..].iter();
        while let Some(b) = iter.next() {
            if b.is_ascii_whitespace() {
                self.position += 1;
            } else {
                break;
            }
        }
    }

    pub fn next_raw_command(&mut self) -> Result<Option<RawCommand<'a>>, eyre::Report> {
        self.eat_whitespace();

        let mut iter = self.input[self.position..].iter();
        let mut position = self.position;
        let v = match iter.next() {
            Some(v) => v,
            None => return Ok(None),
        };

        if v == &self.prefix_caret || v == &self.prefix_tilde {
            position += 1;
        } else {
            return Err(eyre::eyre!("expected prefix, got {:?}", v));
        }

        // scan until we hit another command prefix
        while let Some(v) = iter.next() {
            if v == &self.prefix_caret || v == &self.prefix_tilde {
                break;
            }

            position += 1;
        }

        // ether found next command or got to end without finding terminator, assume terminated.
        let command = &self.input[self.position..position];
        Ok(Some(RawCommand { command }))
    }

    pub fn next_command(&mut self) -> Result<Option<Command>, eyre::Report> {
        let raw_command = self.next_raw_command()?;
        let raw_command = match raw_command {
            Some(v) => v,
            None => return Ok(None),
        };

        let command = self.parse_command(raw_command)?;
        Ok(Some(command))
    }
}

pub struct RawCommand<'a> {
    command: &'a [u8],
}
