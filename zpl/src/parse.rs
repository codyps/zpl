//! Parse ZPL into a series of commands.
//!
//! Only examines the internals of commands that must be parsed to continue the parsing process,
//! does not validate the contents of commands that affect formatting or other printer state not
//! related to parsing.

#[cfg(test)]
mod test;

/// Information we must track to parse the ZPL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseContext<'a> {
    prefixes: Prefixes,

    input: &'a [u8],
    position: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Prefixes {
    // `CC` changes this
    /// "format prefix"
    caret: u8,
    // `CT` changes this
    /// "control prefix"
    tilde: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element<'a> {
    BeforeFirstCommand(&'a [u8]),
    FormatCommand(&'a [u8]),
    ControlCommand(&'a [u8]),
}

impl<'a> ParseContext<'a> {
    pub fn from_bytes(input: &'a [u8]) -> Self {
        Self {
            input,
            position: 0,
            prefixes: Prefixes {
                caret: b'^',
                tilde: b'~',
            },
        }
    }

    // When called, the current position is at a prefix character, so will be skipped.
    fn scan_to_next_prefix(&self, skip: usize, prefixes: Prefixes) -> (usize, &'a [u8]) {
        let mut iter = self.input[(self.position + skip)..].iter();
        let mut position = self.position + skip;
        while let Some(v) = iter.next() {
            if v == &prefixes.caret || v == &prefixes.tilde {
                break;
            }

            position += 1;
        }

        (position, &self.input[self.position..position])
    }

    fn scan_element(&self) -> Option<(usize, Prefixes, Element<'a>)> {
        if self.position >= self.input.len() {
            return None;
        }

        let next_b = self.input[self.position];

        // We do minimal parsing to pull out prefix changes. Specifically: `^CC`, `~CC`,
        // `^CT`, `~CT`. We need to do this before scaning to the next prefix, so we must examine
        // the first few bytes of the command to see if it is one of these commands.
        //
        // We're pretty lenient at this stage: only the first 4 bytes of a prefix changing command
        // are considered, the remainder is ignored. We'll need to validate this behavior against
        // other impls and consider being stricter in checking.
        //
        // TODO: examine if we need to eat whitespace (or some other characters) before the prefix
        // character.
        if next_b == self.prefixes.caret {
            let (skip, prefixes) = self.parse_prefixes(&self.input[self.position..]);
            let (position, command) = self.scan_to_next_prefix(skip, prefixes);
            return Some((position, prefixes, Element::FormatCommand(command)));
        } else if next_b == self.prefixes.tilde {
            let (skip, prefixes) = self.parse_prefixes(&self.input[self.position..]);
            let (position, command) = self.scan_to_next_prefix(skip, prefixes);
            return Some((position, prefixes, Element::ControlCommand(command)));
        } else {
            let (position, command) = self.scan_to_next_prefix(1, self.prefixes);
            return Some((
                position,
                self.prefixes,
                Element::BeforeFirstCommand(command),
            ));
        }
    }

    fn parse_prefixes(&self, command: &[u8]) -> (usize, Prefixes) {
        if command.len() < 4 {
            // Not enough bytes to be a prefix command, ignore.
            return (1, self.prefixes);
        }

        if command[1] != b'C' {
            // Not a prefix command, ignore.
            return (1, self.prefixes);
        }

        if command[2] == b'C' {
            return (
                4,
                Prefixes {
                    caret: command[3],
                    tilde: self.prefixes.tilde,
                },
            );
        } else if command[2] == b'T' {
            return (
                4,
                Prefixes {
                    caret: self.prefixes.caret,
                    tilde: command[3],
                },
            );
        } else {
            // Not a prefix command, ignore.
            return (1, self.prefixes);
        }
    }

    pub fn next_element(&mut self) -> Result<Option<Element<'a>>, eyre::Report> {
        let element = self.scan_element();
        if let Some((position, prefixes, element)) = element {
            self.position = position;
            self.prefixes = prefixes;
            Ok(Some(element))
        } else {
            Ok(None)
        }
    }
}

impl<'a> Iterator for ParseContext<'a> {
    type Item = Result<Element<'a>, eyre::Report>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_element().transpose()
    }
}
