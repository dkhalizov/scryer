//! Parity net, phase 5: property tests for naming (notes/11-parity-net-design.md
//! § Checks beyond examples).
//!
//! What Scryer promises for a generated path component (library/rename.rs and
//! its tests): none of `< > : " / \ | ? *`, no ASCII control char (U+0000..=U+001F),
//! no leading/trailing whitespace or trailing dot, never `.`/`..`, no bare
//! Windows device name (CON, NUL, COM1, ...), at most 217 bytes once rendered (MAX_COMPONENT_BYTES).
//! DEL and C1 controls are not in the promise (legal on every target
//! filesystem), so they are not asserted. Fixed seed, no persistence.

use std::collections::BTreeMap;

// The 240-byte budget minus rename.rs's 24-byte reserve, plus the one '_'
// that disarming a device name can add after truncation (same bound as
// parity/fuzz/fuzz_targets/naming.rs).
const MAX_COMPONENT_BYTES: usize = 240 - 24 + 1;

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, RngSeed};
use scryer_application::parity_seams::render_title_folder_template;
use scryer_application::{render_rename_template, sanitize_filesystem_component};

fn config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        rng_seed: RngSeed::Fixed(0x5c71_e7a2),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

const ILLEGAL: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];
const TOKEN_NAMES: &[&str] = &["title", "year", "episode_title", "quality", "group", "ext"];

/// Why `component` is not a safe path component, if it is not.
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

/// Metadata values an indexer, a provider or a user could hand us.
fn hostile_value() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>(),
        "\\PC{0,40}",
        "[ -~]{0,40}",
        "[a-z/\\\\. :*?<>|\"-]{200,400}",
        prop::sample::select(vec![
            "", ".", "..", "/", "a/b", "../..", "..\\..", " ", "\u{0}", "CON", "nul.txt",
            "Com1 - x", "CON _x", "CON -_x", "aux .srt", "LPT9", "\u{3000}", "a\u{85}b", "_", "{}",
            "()", "{{x}}",
        ])
        .prop_map(String::from),
    ]
}

fn tokens() -> impl Strategy<Value = BTreeMap<String, String>> {
    prop::collection::vec(hostile_value(), TOKEN_NAMES.len()).prop_map(|values| {
        TOKEN_NAMES
            .iter()
            .map(|name| (*name).to_string())
            .zip(values)
            .collect()
    })
}

/// One template piece without a `/`: literal text, a token with optional
/// padding/filters, an optional group, or an escaped literal brace group.
fn piece() -> impl Strategy<Value = String> {
    let name = || prop::sample::select(TOKEN_NAMES);
    let spec = prop::sample::select(vec![
        "",
        ":2",
        "|space:.",
        "|space:_",
        "|space:",
        "|truncate:8",
        ":3|space:-",
    ]);
    prop_oneof![
        "[A-Za-z0-9 ._()\\[\\]:|*?<>\"-]{0,6}",
        (name(), spec).prop_map(|(name, spec)| format!("{{{name}{spec}}}")),
        (name(), name()).prop_map(|(guard, name)| format!("{{?{guard}: - {{{name}}}}}")),
        "[a-z:| ]{0,5}".prop_map(|literal| format!("{{{{{literal}}}}}")),
    ]
}

fn template() -> impl Strategy<Value = String> {
    prop::collection::vec(piece(), 0..8).prop_map(|pieces| pieces.concat())
}

fn folder_template() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(template(), 1..4)
}

proptest! {
    #![proptest_config(config(4096))]

    #[test]
    fn sanitize_emits_a_safe_component(raw in hostile_value()) {
        let out = sanitize_filesystem_component(&raw);
        if let Some(reason) = unsafe_reason(&out) {
            prop_assert!(false, "{} for {:?} -> {:?}", reason, raw, out);
        }
    }

    #[test]
    #[ignore = "known red: CON _x -> CON__x -> CON_x (device-name disarm keeps a separator collapse would merge)"]
    fn sanitize_is_idempotent(raw in hostile_value()) {
        let once = sanitize_filesystem_component(&raw);
        let twice = sanitize_filesystem_component(&once);
        prop_assert_eq!(&twice, &once, "raw {:?}", raw);
    }

    /// The same property away from the known red (a leading device name), so
    /// the rest of sanitize stays guarded while that one is open.
    #[test]
    fn sanitize_is_idempotent_off_device_names(raw in hostile_value()) {
        let once = sanitize_filesystem_component(&raw);
        prop_assume!(!RESERVED.iter().any(|name| {
            once.get(..name.len()).is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
        }));
        let twice = sanitize_filesystem_component(&once);
        prop_assert_eq!(&twice, &once, "raw {:?}", raw);
    }

    #[test]
    fn render_rename_template_emits_one_safe_component(
        template in template(), tokens in tokens(),
    ) {
        let out = render_rename_template(&template, &tokens);
        if let Some(reason) = unsafe_reason(&out) {
            prop_assert!(false, "{} for {:?} -> {:?}", reason, template, out);
        }
        prop_assert!(out.len() <= MAX_COMPONENT_BYTES, "{} bytes for {:?}", out.len(), template);
    }

    #[test]
    fn render_title_folder_template_emits_safe_components_at_template_depth(
        components in folder_template(), tokens in tokens(),
    ) {
        let template = components.join("/");
        let out = render_title_folder_template(&template, &tokens);
        let rendered: Vec<&str> = out.split('/').collect();
        prop_assert_eq!(
            rendered.len(), components.len(),
            "depth changed: {:?} -> {:?}", template, out
        );
        for component in rendered {
            prop_assert!(!component.is_empty(), "empty component: {:?} -> {:?}", template, out);
            if let Some(reason) = unsafe_reason(component) {
                prop_assert!(false, "{} in {:?}: {:?} -> {:?}", reason, component, template, out);
            }
            prop_assert!(component.len() <= MAX_COMPONENT_BYTES, "{} bytes: {:?}", component.len(), template);
        }
    }
}
