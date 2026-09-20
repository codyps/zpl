//! ^PA bidirectional layout (Zebra guide p. 315), UAX #9 rules L1–L4:
//! https://www.unicode.org/reports/tr9/#Reordering_Resolved_Levels
//! The embedded repertoire contains no combining glyphs; unknown characters
//! still fail font lookup. Mirror the supported ASCII pairs in odd-level runs.
use unicode_bidi::BidiInfo;
struct BidiData {
    skip_paired_brackets: bool,
    legacy_isolates: bool,
}
impl unicode_bidi::BidiDataSource for BidiData {
    fn bidi_class(&self, c: char) -> unicode_bidi::BidiClass {
        if self.legacy_isolates && matches!(c, '\u{2066}'..='\u{2069}') {
            unicode_bidi::BidiClass::L
        } else {
            unicode_bidi::bidi_class(c)
        }
    }
    fn bidi_matched_opening_bracket(
        &self,
        c: char,
    ) -> Option<unicode_bidi::data_source::BidiMatchedOpeningBracket> {
        if self.skip_paired_brackets {
            None
        } else {
            unicode_bidi::BidiDataSource::bidi_matched_opening_bracket(
                &unicode_bidi::HardcodedBidiData,
                c,
            )
        }
    }
}
pub(super) fn base_level(value: &str) -> Option<unicode_bidi::Level> {
    BidiInfo::new(value, None)
        .paragraphs
        .first()
        .map(|p| p.level)
}

pub(super) fn reorder(value: &str, skip_paired_brackets: bool, legacy_isolates: bool) -> String {
    let mut result = String::new();
    let mut offset = 0;
    for paragraph in value.split_inclusive('\n') {
        let text = paragraph.strip_suffix('\n').unwrap_or(paragraph);
        result.push_str(&reorder_range(
            value,
            offset..offset + text.len(),
            skip_paired_brackets,
            legacy_isolates,
        ));
        if paragraph.ends_with('\n') {
            result.push('\n');
        }
        offset += paragraph.len();
    }
    result
}

// Resolve the whole paragraph before applying line-specific L1/L2. Resolving
// each wrapped line independently loses surrounding neutral/embedding context.
pub(super) fn reorder_range(
    value: &str,
    range: std::ops::Range<usize>,
    skip_paired_brackets: bool,
    legacy_isolates: bool,
) -> String {
    let info = BidiInfo::new_with_data_source(
        &BidiData {
            skip_paired_brackets,
            legacy_isolates,
        },
        value,
        None,
    );
    let mut out = String::new();
    for para in &info.paragraphs {
        let line = range.start.max(para.range.start)..range.end.min(para.range.end);
        if line.is_empty() {
            continue;
        }
        let levels = info.reordered_levels(para, line.clone());
        let characters: Vec<_> = value
            .char_indices()
            .filter(|(i, _)| line.contains(i))
            .collect();
        let levels: Vec<_> = characters.iter().map(|(i, _)| levels[*i]).collect();
        for i in BidiInfo::reorder_visual(&levels) {
            let (_, mut c) = characters[i];
            if legacy_isolates && matches!(c, '\u{2066}'..='\u{2069}') {
                // Native isolates behave as class-L missing characters.
                out.push('\u{378}');
                continue;
            }
            // Directional controls affect order, never ink (UAX #9 section 2).
            if matches!(c, '\u{61c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            {
                continue;
            }
            if levels[i].is_rtl() {
                c = match c {
                    '(' => ')',
                    ')' => '(',
                    '[' => ']',
                    ']' => '[',
                    '{' => '}',
                    '}' => '{',
                    '<' => '>',
                    '>' => '<',
                    _ => c,
                };
            }
            out.push(c);
        }
    }
    out
}
