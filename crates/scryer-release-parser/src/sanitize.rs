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
    // fork (P35 D): a reversed scene name is read forwards (arr order: before
    // the site tag).
    let cleaned = match reversed_title(&cleaned) {
        Some(forwards) => {
            hints.push("reversed_title".to_string());
            forwards
        }
        None => cleaned,
    };
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

/// fork (P35 D): `name` read forwards when it is a reversed scene name: a
/// segment other than the last is `p027`, `p0801` or a reversed episode
/// marker (`10E10S`), as Sonarr's case-sensitive ReversedTitleRegex
/// `(?:^|[-._ ])(p027|p0801|\d{2,3}E\d{2}S)[-._ ]`. A media extension stays
/// at the end.
fn reversed_title(name: &str) -> Option<String> {
    let marker = |seg: &str| {
        let b = seg.as_bytes();
        let n = b.len();
        seg == "p027"
            || seg == "p0801"
            || ((6..=7).contains(&n)
                && b[..n - 4].iter().all(u8::is_ascii_digit)
                && b[n - 4] == b'E'
                && b[n - 3..n - 1].iter().all(u8::is_ascii_digit)
                && b[n - 1] == b'S')
    };
    let segments: Vec<&str> = name.split(['-', '.', '_', ' ']).collect();
    if !segments[..segments.len() - 1].iter().any(|seg| marker(seg)) {
        return None;
    }
    let stem = strip_media_extension(name);
    Some(
        stem.chars()
            .rev()
            .chain(name[stem.len()..].chars())
            .collect(),
    )
}

/// fork (P35): extensions the arrs' RemoveFileExtension strips (media files
/// plus `.par2`/`.nzb`); any other dotted tail is part of the name.
const MEDIA_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "ts", "m2ts", "wmv", "mov", "m4v", "mpg", "mpeg", "webm", "par2", "nzb",
];

/// `name` without one trailing media extension (`.mkv`, `.par2`, ...).
pub(crate) fn strip_media_extension(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, ext)) if MEDIA_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)) => stem,
        _ => name,
    }
}

/// fork (P35): a name the arrs refuse before parsing (Sonarr
/// ValidateBeforeParsing + RejectHashedReleasesRegexes, Parser.cs:471-512,
/// a superset of Radarr's): password/yEnc posts, names with no letter or
/// digit, and hash-shaped stems.
#[must_use]
pub fn is_hashed_release_name(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    if (lower.contains("password") && lower.contains("yenc"))
        || !raw.chars().any(char::is_alphanumeric)
    {
        return true;
    }
    let s = strip_media_extension(raw);
    let b = s.as_bytes();
    let all = |r: &[u8], f: fn(&u8) -> bool| r.iter().all(f);
    let shaped = |letters: usize, upper: bool| {
        b.len() == letters + 3
            && all(
                &b[..letters],
                if upper {
                    u8::is_ascii_uppercase
                } else {
                    u8::is_ascii_lowercase
                },
            )
            && all(&b[letters..], u8::is_ascii_digit)
    };
    let backup = s.strip_prefix("Backup_").is_some_and(|r| {
        let digits = r.bytes().take_while(u8::is_ascii_digit).count();
        let t = r[digits..].as_bytes();
        digits >= 5
            && t.len() == 6
            && t[0] == b'S'
            && all(&t[1..3], u8::is_ascii_digit)
            && t[3] == b'-'
            && all(&t[4..], u8::is_ascii_digit)
    });
    b.iter().take_while(|c| c.is_ascii_alphanumeric()).count() >= 24
        || shaped(11, true)
        || shaped(12, false)
        || backup
        || s == "123"
        || s.eq_ignore_ascii_case("abc")
        || s.eq_ignore_ascii_case("b00bs")
        || (b.len() >= 7
            && b[..3].eq_ignore_ascii_case(b"abc")
            && matches!(b[3], b'-' | b'_' | b'.' | b' ')
            && b[4..7].eq_ignore_ascii_case(b"xyz"))
        || (b.len() == 9
            && all(&b[..6], u8::is_ascii_digit)
            && b[6] == b'_'
            && all(&b[7..], u8::is_ascii_digit))
}
