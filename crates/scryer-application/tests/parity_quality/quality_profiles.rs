//! Stream `quality_profiles`: the arr default-profile seeding rule against
//! Scryer's "no profiles stored, supply the built-ins" rule.
//!
//! Suites: `son_quality_profiles` (SON-QUALITY-0033, 0034) and
//! `rad_quality_profiles` (RAD-QUALITY-0023, 0024): cases with
//! `input.op == "default_profiles"` in
//! `corpus/reference/<app>/quality/quality_profile_service.json` (a top-level
//! JSON array). Other ops in the same file are skipped before `wants()`.
//!
//! Scryer function under test: `parity_seams::ensure_quality_profiles_exist`
//! (settings/runtime/quality_profiles.rs:45), applied by
//! `load_quality_profile_settings` (:338) whenever profiles are loaded.
//!
//! # Translation table
//!
//! - Reference repository `All()` returning n profiles -> the stored list
//!   passed in (n profiles).
//! - `expect.profiles_inserted` (Insert calls) -> number of returned profiles
//!   whose id is not among the input ids.
//! - Reference `Builder<QualityProfile>` existing profiles -> clones of
//!   `builtin_8k_profile` re-identified `existing-1..n` (harness glue; any
//!   valid body works, ids must not collide with Scryer's default ids).
//!
//! # Ignored keys
//!
//! `input.given.custom_formats` (the reference only feeds format items;
//! Scryer's default rule does not look at custom formats), `note`.
//!
//! # Caveat
//!
//! Real startup seeding lives in the binary crate:
//! `crates/scryer/src/settings_bootstrap.rs:1828` `normalize_quality_profile_settings`
//! persists 1080p, 4k and anime through `merge_default_quality_profiles`
//! (:1936). That merge also adds new defaults when every stored profile is an
//! untouched system default, which the reference deliberately does not do.
//! The crate is bin-only, so a test cannot reach either. A fix for 0033/0023
//! must change both places, or the green would not reflect a fresh install.
//!
//! # Unmapped (tests: []), with reasons
//!
//! - `delay_profile_reorder` (SON 0017-0021, RAD 0017-0021): concept absent.
//!   Delay profiles carry a user-set `priority` (acquisition/delay_profile.rs:121)
//!   and `upsert_delay_profile` (settings/runtime/quality_profiles.rs:904) stores
//!   it unchanged; no move-after or renumber operation exists.
//! - `profile_delete` (SON 0035-0037, RAD 0025-0028): the guard
//!   `ensure_quality_profiles_are_unreferenced` (quality_profiles.rs:479, called
//!   from `delete_quality_profile` :842) is a private async `AppUseCase` method
//!   over stores; no pure core takes lists, so a visibility seam is not enough.
//! - `acceptable_languages` (RAD 0029-0030): concept absent. A Scryer profile
//!   has no profile language; `criteria.required_audio_languages` is a user
//!   filter, not derived from format scores.

use scryer_application::{QualityProfile, builtin_8k_profile, parity_seams::ensure_quality_profiles_exist};
use scryer_parity::{Suite, corpus};
use serde_json::Value;

fn default_profiles(suite_name: &str, file: &str) {
    let Some(mut suite) = Suite::open(suite_name) else { return };
    let cases: Vec<Value> = corpus(file);
    for case in &cases {
        if case["input"]["op"] != "default_profiles" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let n = case["input"]["given"]["existing_profiles"].as_i64().unwrap();
        let existing: Vec<QualityProfile> = (1..=n)
            .map(|i| {
                let mut p = builtin_8k_profile();
                p.id = format!("existing-{i}");
                p.name = format!("Existing {i}");
                p
            })
            .collect();
        let input_ids: Vec<String> = existing.iter().map(|p| p.id.clone()).collect();
        let out = ensure_quality_profiles_exist(existing);
        let inserted = out.iter().filter(|p| !input_ids.contains(&p.id)).count() as i64;
        let ids: Vec<&str> = out.iter().map(|p| p.id.as_str()).collect();
        super::record(
            &mut suite,
            id,
            case["expect"]["profiles_inserted"].as_i64().unwrap(),
            inserted,
            &format!("ensure_quality_profiles_exist returned {ids:?}"),
        );
    }
    suite.finish();
}

#[test]
fn son_quality_profiles() {
    default_profiles(
        "scryer-application::parity_quality::son_quality_profiles",
        "corpus/reference/sonarr/quality/quality_profile_service.json",
    );
}

#[test]
fn rad_quality_profiles() {
    default_profiles(
        "scryer-application::parity_quality::rad_quality_profiles",
        "corpus/reference/radarr/quality/quality_profile_service.json",
    );
}
