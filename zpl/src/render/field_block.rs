//! ^FB line layout. Zebra Programming Guide pp. 186–187:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use super::{compatibility::Compatibility, font};

pub(super) type Line = (String, bool, bool, bool);
#[derive(Default)]
struct Word {
    text: String,
    markers: Vec<(usize, bool)>,
}
struct Part<'a> {
    text: &'a str,
    hyphen: bool,
}
struct Layout {
    font: font::Font,
    w: f64,
    h: f64,
    width: f64,
    indent: f64,
    compatibility: Compatibility,
    lines: Vec<Line>,
    line: String,
    overflow_line: bool,
}
impl Layout {
    fn measure(&self, text: &str) -> Result<f64, String> {
        font::width_for(self.font, text, self.w, self.h)
    }
    fn limit(&self) -> f64 {
        let width = self.width
            - if self.lines.is_empty() {
                0.
            } else {
                self.indent
            };
        if width < 0. && self.compatibility.block_indent_printer_layout {
            f64::INFINITY
        } else {
            width
        }
    }
    fn prefix(&self) -> String {
        if self.line.is_empty() {
            String::new()
        } else {
            format!("{} ", self.line)
        }
    }
    fn flush(&mut self, hyphen: bool, forced: bool) {
        self.lines
            .push((std::mem::take(&mut self.line), false, hyphen, forced));
        self.overflow_line = false;
    }
    fn hyphen(&self) -> char {
        if self.compatibility.block_hyphenation_printer_layout {
            '\u{ad}'
        } else {
            '-'
        }
    }
    fn normal(&mut self, mut word: &str) -> Result<(), String> {
        if self.overflow_line {
            self.line = format!("{}{word}", self.prefix());
            return Ok(());
        }
        let mut splitting = false;
        while !word.is_empty() {
            let limit = self.limit();
            let word_width = self.measure(word)?;
            splitting |= word_width > limit;
            if !splitting
                || (!self.compatibility.block_hyphenation_printer_layout && word_width <= limit)
            {
                let next = format!("{}{word}", self.prefix());
                if !self.line.is_empty() && self.measure(&next)? > limit {
                    self.flush(false, false);
                    // An intact word already fitted the old line width. The
                    // printer keeps that decision after moving it past an indent.
                    if self.compatibility.block_indent_printer_layout {
                        self.line = word.to_string();
                        break;
                    }
                    continue;
                }
                self.line = next;
                break;
            }
            // Captured automatic hyphenation reserves AD space even for the
            // final remainder and requires a strict fit. The specification
            // profile uses a normal hyphen and permits an exact fit.
            let hyphen = self.hyphen();
            let prefix = self.prefix();
            let mut cut = 0;
            for end in word.char_indices().map(|(i, c)| i + c.len_utf8()) {
                let trial = format!("{prefix}{}{hyphen}", &word[..end]);
                let advance = self.measure(&trial)?;
                let fits = if self.compatibility.block_hyphenation_printer_layout {
                    advance < limit
                } else {
                    advance <= limit
                };
                if !fits {
                    break;
                }
                cut = end;
            }
            if cut == 0 {
                if self.line.is_empty() {
                    if self.compatibility.block_narrow_printer_layout {
                        let end = word.chars().next().unwrap().len_utf8();
                        let chunk = &word[..end];
                        let with_hyphen = format!("{chunk}{hyphen}");
                        let paint = self.measure(&with_hyphen)? <= limit;
                        self.line = if paint {
                            with_hyphen
                        } else {
                            chunk.to_string()
                        };
                        self.flush(paint, true);
                        word = &word[end..];
                        continue;
                    }
                    return Err("field block too narrow for a character and hyphen".into());
                }
                self.flush(false, false);
                continue;
            }
            if cut == word.len() {
                self.line = format!("{prefix}{word}");
                break;
            }
            self.line = format!("{prefix}{}{hyphen}", &word[..cut]);
            self.flush(true, false);
            word = &word[cut..];
        }
        Ok(())
    }
    fn choice(&self, parts: &[Part<'_>]) -> Result<Option<(usize, String, bool)>, String> {
        let mut prefix = String::new();
        let mut found = None;
        for (i, part) in parts[..parts.len() - 1].iter().enumerate() {
            prefix.push_str(part.text);
            let hyphen = if part.hyphen {
                self.hyphen().to_string()
            } else {
                String::new()
            };
            if self.measure(&format!("{}{prefix}{hyphen}", self.prefix()))? <= self.limit() {
                found = Some((i, prefix.clone(), part.hyphen));
            }
        }
        Ok(found)
    }
    fn marked(&mut self, mut parts: &[Part<'_>]) -> Result<(), String> {
        let native = self.compatibility.block_soft_hyphen_printer_layout;
        let mut previous_limit = None;
        loop {
            while parts.first().is_some_and(|p| p.text.is_empty()) {
                parts = &parts[1..];
            }
            if parts.is_empty() {
                return Ok(());
            }
            let clean: String = parts.iter().map(|p| p.text).collect();
            if self.overflow_line {
                self.line = format!("{}{clean}", self.prefix());
                return Ok(());
            }
            if parts.len() == 1 {
                return self.normal(parts[0].text);
            }
            if !self.line.is_empty() && self.measure(&format!("{} ", self.line))? >= self.limit() {
                self.flush(false, false);
            }
            let limit = self.limit();
            let first = parts[0].text;
            // ZD621 keeps an earlier width decision across a marker break.
            // If indentation then makes that segment too wide, it emits the
            // whole remainder and subsequent words without further wrapping.
            if native
                && previous_limit
                    .is_some_and(|old| self.measure(first).is_ok_and(|w| w > limit && w <= old))
            {
                self.line = clean;
                self.overflow_line = true;
                return Ok(());
            }
            if self.measure(&format!("{}{clean}", self.prefix()))? <= limit {
                self.line = format!("{}{clean}", self.prefix());
                return Ok(());
            }
            if self.measure(first)? > limit {
                self.normal(first)?;
                parts = &parts[1..];
                previous_limit = None;
                continue;
            }
            let mut choice = self.choice(parts)?;
            if choice.is_none() {
                if !self.line.is_empty() || (native && previous_limit.is_none()) {
                    self.flush(false, false);
                }
                if native && self.measure(first)? > self.limit() {
                    self.line = clean;
                    self.overflow_line = true;
                    return Ok(());
                }
                choice = self.choice(parts)?;
            }
            let (i, prefix, kind) = choice.unwrap_or_else(|| (0, first.to_string(), false));
            let limit = self.limit();
            let paint_hyphen = kind && (!native || self.measure(parts[i + 1].text)? <= limit);
            self.line = format!(
                "{}{prefix}{}",
                self.prefix(),
                if paint_hyphen {
                    self.hyphen().to_string()
                } else {
                    String::new()
                }
            );
            self.flush(paint_hyphen, false);
            parts = &parts[i + 1..];
            if native && parts.len() == 1 && self.measure(parts[0].text)? <= limit {
                self.line = parts[0].text.to_string();
                return Ok(());
            }
            previous_limit = Some(limit);
        }
    }
}

pub(super) fn wrap(
    font: font::Font,
    value: &str,
    size: (f64, f64),
    dimensions: (f64, f64),
    compatibility: Compatibility,
    encoding: u8,
) -> Result<Vec<Line>, String> {
    let mut paragraphs: Vec<Vec<Word>> = vec![Vec::new()];
    let mut word = Word::default();
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('\\') => {
                    if encoding != 13 && !compatibility.block_backslash_without_ci13 {
                        return Err("field-block backslash requires ^CI13".into());
                    }
                    chars.next();
                    word.text.push('\\');
                    continue;
                }
                Some('&') => {
                    chars.next();
                    if !word.text.is_empty() || !word.markers.is_empty() {
                        paragraphs
                            .last_mut()
                            .unwrap()
                            .push(std::mem::take(&mut word));
                    }
                    paragraphs.push(Vec::new());
                    continue;
                }
                Some(next)
                    if next.is_ascii_alphanumeric()
                        || compatibility.block_soft_hyphen_printer_layout =>
                {
                    let hyphen = *next != '(';
                    if !hyphen {
                        chars.next();
                    }
                    word.markers.push((word.text.len(), hyphen));
                    continue;
                }
                _ => {}
            }
        }
        if c.is_whitespace() {
            if !word.text.is_empty() || !word.markers.is_empty() {
                paragraphs
                    .last_mut()
                    .unwrap()
                    .push(std::mem::take(&mut word));
            }
        } else {
            word.text.push(c);
        }
    }
    if !word.text.is_empty() || !word.markers.is_empty() {
        paragraphs.last_mut().unwrap().push(word);
    }
    let mut layout = Layout {
        font,
        w: size.0,
        h: size.1,
        width: dimensions.0,
        indent: dimensions.1,
        compatibility,
        lines: Vec::new(),
        line: String::new(),
        overflow_line: false,
    };
    for (i, paragraph) in paragraphs.iter().enumerate() {
        for (word_index, word) in paragraph.iter().enumerate() {
            if compatibility.block_narrow_printer_layout
                && word_index > 0
                && layout.line.is_empty()
                && layout.measure(" ")? >= layout.limit()
            {
                layout.flush(false, false);
            }
            if word.markers.is_empty() {
                layout.normal(&word.text)?;
            } else {
                let mut parts = Vec::new();
                let mut start = 0;
                for &(end, hyphen) in &word.markers {
                    parts.push(Part {
                        text: &word.text[start..end],
                        hyphen,
                    });
                    start = end;
                }
                parts.push(Part {
                    text: &word.text[start..],
                    hyphen: false,
                });
                layout.marked(&parts)?;
            }
        }
        layout.flush(false, false);
        layout.lines.last_mut().unwrap().1 = i + 1 < paragraphs.len();
    }
    Ok(layout.lines)
}
