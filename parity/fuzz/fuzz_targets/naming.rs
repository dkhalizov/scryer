//! Naming never emits an unsafe path component, whatever the template and
//! token values. Same promise as crates/scryer-application/tests/props_naming.rs
//! (keep `unsafe_reason` in step with it). Idempotence is checked only away
//! from a leading device name: that case is the known red in props_naming.
#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use scryer_application::parity_seams::render_title_folder_template;
use scryer_application::{render_rename_template, sanitize_filesystem_component};

const ILLEGAL: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];
// rename.rs truncates to GENERATED_COMPONENT_MAX_BYTES (240) minus the
// 24-byte suffix reserve. Disarming a device name the cut exposed can add one
// '_' after it ("CONa" + ".e"*212 -> "CON_.eee…", 217 bytes), hence + 1.
const MAX_COMPONENT_BYTES: usize = 240 - 24 + 1;
const TOKEN_NAMES: &[&str] = &["title", "year", "episode_title", "quality", "group", "ext"];

fn unsafe_reason(component: &str) -> Option<String> {
    if let Some(ch) = component
        .chars()
        .find(|ch| ILLEGAL.contains(ch) || *ch <= '\u{1f}')
    {
        return Some(format!("illegal char {ch:?}"));
    }
    if component.ends_with('.') || component.ends_with(char::is_whitespace) {
        return Some("trailing dot or whitespace".into());
    }
    if component.starts_with(char::is_whitespace) {
        return Some("leading whitespace".into());
    }
    if matches!(component, "." | "..") {
        return Some("dot component".into());
    }
    let stem = component.split('.').next().unwrap_or_default().trim_end();
    if RESERVED.iter().any(|name| stem.eq_ignore_ascii_case(name)) {
        return Some(format!("reserved device name {stem:?}"));
    }
    None
}

fn starts_with_device_name(value: &str) -> bool {
    RESERVED.iter().any(|name| {
        value
            .get(..name.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
    })
}

fuzz_target!(
    |input: (String, String, Vec<String>, Vec<(String, String)>)| {
        let (raw, template, values, extra) = input;
        // Known token names get fuzzed values; arbitrary extra keys on top.
        let mut tokens: BTreeMap<String, String> = TOKEN_NAMES
            .iter()
            .map(|name| (*name).to_string())
            .zip(values)
            .collect();
        tokens.extend(extra);

        let once = sanitize_filesystem_component(&raw);
        if let Some(reason) = unsafe_reason(&once) {
            panic!("sanitize: {reason} for {raw:?} -> {once:?}");
        }
        if !starts_with_device_name(&once) {
            let twice = sanitize_filesystem_component(&once);
            assert_eq!(twice, once, "sanitize not idempotent for {raw:?}");
        }

        let file = render_rename_template(&template, &tokens);
        if let Some(reason) = unsafe_reason(&file) {
            panic!("render_rename_template: {reason} for {template:?} -> {file:?}");
        }
        assert!(file.len() <= MAX_COMPONENT_BYTES, "{} bytes for {template:?}", file.len());

        // Any template: every rendered component is safe. A '/' inside braces
        // (`{ext/}`) is not a separator, so depth is only checked below.
        check_folder(&template, &tokens, None);
        // No brace group spans a '/' (as props_naming builds them): depth kept.
        if template.split('/').all(braces_balanced) {
            check_folder(&template, &tokens, Some(template.split('/').count()));
        }
    }
);

fn braces_balanced(piece: &str) -> bool {
    let mut open = 0usize;
    for ch in piece.chars() {
        match ch {
            '{' => open += 1,
            '}' => match open.checked_sub(1) {
                Some(rest) => open = rest,
                None => return false,
            },
            _ => {}
        }
    }
    open == 0
}

fn check_folder(template: &str, tokens: &BTreeMap<String, String>, depth: Option<usize>) {
    let folder = render_title_folder_template(template, tokens);
    let rendered: Vec<&str> = folder.split('/').collect();
    if let Some(depth) = depth {
        assert_eq!(
            rendered.len(),
            depth,
            "depth changed: {template:?} -> {folder:?}"
        );
    }
    for component in rendered {
        assert!(
            !component.is_empty(),
            "empty component: {template:?} -> {folder:?}"
        );
        if let Some(reason) = unsafe_reason(component) {
            panic!(
                "render_title_folder_template: {reason} in {component:?}: {template:?} -> {folder:?}"
            );
        }
        assert!(
            component.len() <= MAX_COMPONENT_BYTES,
            "{} bytes: {template:?}",
            component.len()
        );
    }
}
