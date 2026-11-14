use html_escape::decode_html_entities;
use unicode_normalization::UnicodeNormalization;

pub(crate) const MAX_INPUT_BYTES: usize = 4096;

#[derive(Debug, Clone)]
pub(crate) struct SanitizedInput {
    pub(crate) value: String,
    pub(crate) hints: Vec<String>,
}

pub(crate) fn sanitize_input(raw: &str) -> SanitizedInput {
    let mut hints = Vec::new();
    let truncated = truncate_utf8(raw, MAX_INPUT_BYTES, &mut hints);
    let html_decoded = decode_common_entities(truncated, &mut hints);
    let normalized = html_decoded.nfkc().collect::<String>();
    let cleaned = normalized
        .chars()
        .filter_map(|ch| sanitize_char(ch, &mut hints))
        .collect::<String>();
    // fork (P17): drop a leading site tag, like the arrs' WebsitePrefixRegex.
    let cleaned = match website_prefix_len(&cleaned) {
        Some(len) => {
            hints.push("website_prefix_stripped".to_string());
            cleaned[len..].to_string()
        }
        None => cleaned,
    };
    let punctuation_normalized = cleaned.replace('&', " and ").replace(':', " ");

    SanitizedInput {
        value: punctuation_normalized,
        hints,
    }
}

/// Byte length of a leading site tag: `www.site.com - `, `[ site.co.uk ] - `,
/// `(site.com) ` (Sonarr/Radarr WebsitePrefixRegex, `Naruto-Kun.Hu` exempt).
/// `None` when there is no tag or stripping it would leave nothing.
pub(crate) fn website_prefix_len(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let close = match b.first() {
        Some(b'[') => Some(b']'),
        Some(b'(') => Some(b')'),
        _ => None,
    };
    let skip = |i: usize, f: fn(u8) -> bool| i + b[i..].iter().take_while(|&&c| f(c)).count();
    let start = if close.is_some() {
        skip(1, |c| c == b' ')
    } else {
        0
    };
    let host_end = skip(start, |c| {
        c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.')
    });
    let labels: Vec<&str> = s[start..host_end].split('.').collect();
    let www = labels.len() > 2 && labels[0].eq_ignore_ascii_case("www");
    let rest = &labels[usize::from(www)..];
    if !(2..=3).contains(&rest.len())
        || rest[0].is_empty()
        || rest[0].eq_ignore_ascii_case("naruto-kun")
        || !rest[1..]
            .iter()
            .all(|l| l.len() >= 2 && l.bytes().all(|c| c.is_ascii_alphabetic()))
    {
        return None;
    }
    // ponytail: a host needs `www.` or short lower-case TLDs so `Show.Name -
    // S01E01` and `[Dr.Stone]` stay title text. Ceilings: a lower-case
    // `csi.ny - 1x01` still strips; a no-www `.info`/`Site.World` host is
    // not stripped (stays unmatched, as before). Upgrade: a TLD list.
    let short_lower = rest[1..]
        .iter()
        .all(|l| l.len() <= 3 && l.bytes().all(|c| c.is_ascii_lowercase()));
    if !www && !short_lower {
        return None;
    }
    let mut end = host_end;
    if let Some(close) = close {
        end = skip(end, |c| c == b' ');
        if b.get(end) != Some(&close) {
            return None;
        }
        end += 1;
    } else if skip(end, |c| matches!(c, b' ' | b'-')) - end < 2 {
        return None;
    }
    end = skip(end, |c| matches!(c, b' ' | b'-'));
    (end < b.len()).then_some(end)
}

fn decode_common_entities(raw: &str, hints: &mut Vec<String>) -> String {
    let mut decoded = raw.to_string();
    let mut changed = false;

    // Real-world feeds occasionally arrive double-encoded (for example
    // `&amp;amp;`). Decode a bounded number of times so we normalize common
    // garbage without turning sanitize into an open-ended rewrite loop.
    for _ in 0..2 {
        let next = decode_html_entities(&decoded).into_owned();
        if next == decoded {
            break;
        }
        decoded = next;
        changed = true;
    }

    if changed {
        hints.push("html_entity_decoded".to_string());
    }

    decoded
}

fn truncate_utf8<'a>(raw: &'a str, max_bytes: usize, hints: &mut Vec<String>) -> &'a str {
    if raw.len() <= max_bytes {
        return raw;
    }

    let mut boundary = max_bytes;
    while boundary > 0 && !raw.is_char_boundary(boundary) {
        boundary -= 1;
    }
    hints.push("input_truncated".to_string());
    &raw[..boundary]
}

fn sanitize_char(ch: char, hints: &mut Vec<String>) -> Option<char> {
    if is_zero_width(ch) {
        hints.push("zero_width_stripped".to_string());
        return None;
    }
    if is_bidi_control(ch) {
        hints.push("bidi_control_stripped".to_string());
        return None;
    }
    Some(ch)
}

fn is_zero_width(ch: char) -> bool {
    matches!(
        ch,
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'
    )
}

fn is_bidi_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{202A}'
            | '\u{202B}'
            | '\u{202C}'
            | '\u{202D}'
            | '\u{202E}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
    )
}
