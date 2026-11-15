//! Quality comparison cases mapped onto Scryer's production two-sided ladder
//! `ladder_against_incumbent` (acquisition/admission.rs), which walks the
//! profile's decision pipeline (tier, revision, language gates).
//!
//! Suites (corpus/reference/<app>/quality/):
//! - `son_quality_index_compare`  SON-QUALITY-0023..0032, `rad_quality_index_compare` RAD-QUALITY-0031..0040
//! - `son_quality_model_compare`  SON-QUALITY-0078..0083, `rad_quality_model_compare` RAD-QUALITY-0085..0090
//! - `son_revision_compare`       SON-QUALITY-0084..0095, `rad_revision_compare`      RAD-QUALITY-0091..0102
//!
//! # Translation
//!
//! - first/left -> candidate, second/right -> held incumbent. Actual = sign of
//!   `LadderOutcome` (Won 1, Lost -1, ReachedScore 0); the Debug of the outcome
//!   is the failure detail (it names the deciding gate).
//! - Tier: `quality_tier_index` (production `normalize_quality_tier` joins the
//!   profile spelling `1080P` and the release spelling `1080p`).
//! - Index compare: `QualityIndex.Index` -> position in a tier list; arr counts
//!   up from worst, Scryer stores best first, so the tiers are
//!   `["480P","720P","1080P"]` and index i is label `["1080p","720p","480p"][i]`
//!   (against resolution on purpose, so a resolution sort would fail).
//!   `group_index` has no Scryer term (every member of a tier is the same
//!   tier).
//! - Model compare: `profile_items` -> `quality_tiers`: allowed items only,
//!   reversed to best first, groups expanded, items without source or lines in
//!   `super::QUALITIES` skipped, first occurrence kept per `<lines>P` (one tier
//!   per resolution; Radarr's BR-DISK puts `1080P` first, no verdict changes).
//!   A quality group is one tier because its members share a resolution.
//! - Revision: `{version, real}` -> release-name tokens (`REAL.` x real, then
//!   `PROPER.` when version is 2) in a one-template title with group GRP; the
//!   revision and group come from `parse_release_metadata` (the D39 fold). The
//!   harness does not use `version - 1 + real` here. Version 1 with real 1 is
//!   a bare `REAL.` token, which Scryer parses as revision 0 (see Reds). Both
//!   sides share the group GRP because Scryer's revision step compares
//!   releases of one group only, unless both are one known exact quality
//!   (P30b; this suite sets no quality key; same convention as
//!   decision_delay's PROPER_GROUP). The failure detail carries both parsed revisions.
//!
//! # Reflexive greens (assert only that the ladder is reflexive)
//! - Index compare SON 0029/0031, RAD 0037/0039 (rgo=false, sides differ only
//!   in the dropped `group_index`). Kept green: they pair with the rgo=true
//!   reds to show Scryer always behaves as respect_group_order=false.
//! - Model compare SON 0082, RAD 0089 (HDTV-720p vs WEBDL-720p): both become
//!   tier `720P`, because a Scryer tier is a resolution. Deriving the label
//!   through the parser adds nothing: its quality is the resolution token the
//!   harness writes into the title.
//!
//! # Weak greens (pass because a bare REAL parses to revision 0, see Reds)
//! - Revision SON 0091, RAD 0098 (REAL vs REAL): 0 vs 0 on both sides.
//! - Revision SON 0086/0089, RAD 0093/0096: the bare-REAL side parses to 0
//!   (reference 1); they pass only because the REAL.PROPER side parses to 2.
//!
//! # Reds
//! - No order inside a tier (Scryer has no quality groups); the ladder gives
//!   ReachedScore (0) where the reference gives +/-1: SON 0024 0026 0083,
//!   RAD 0032 0034 0090.
//! - A bare REAL parses to revision 0: scryer-release-parser enrichment.rs
//!   counts REAL only directly before PROPER/REPACK/RERIP, while the reference
//!   counts every REAL match (Sonarr QualityParser.cs:702, Radarr :766). The
//!   ladder gives 0 where the reference gives +/-1: SON 0084 0087, RAD 0091 0094.
//!
//! # Ignored keys
//! - `respect_group_order`: no Scryer term (no order inside a tier); drives the reds.
//! - `group_index`, `note`.
//!
//! # Unmapped (tests: []), with reasons
//! - RAD-QUALITY-0103: compares a revision with a C# null (Revision.cs:103). A
//!   Scryer revision is a plain i32 and cannot be missing. The suite skips it
//!   before `wants()`. (Its Sonarr twin SON-QUALITY-0096 is out of scope.)

use super::{QUALITIES, record};
use scryer_application::parity_seams::{
    CandidateFacts, Incumbent, LadderOutcome, ladder_against_incumbent, quality_tier_index,
};
use scryer_application::{
    AudioLanguageRank, QualityProfileCriteria, builtin_default_quality_profile, parse_release_metadata,
};
use scryer_parity::{Suite, corpus};
use serde_json::Value;

const GROUP: &str = "GRP";
const INDEX_LABELS: [&str; 3] = ["1080p", "720p", "480p"];

fn held(tier_index: Option<usize>, revision: i32, group: Option<String>) -> Incumbent {
    Incumbent {
        quality_key: None,
        tier_index,
        revision,
        file_id: String::new(),
        file_path: String::new(),
        release_group: group,
        score: 0,
        language: AudioLanguageRank::default(),
        covers: vec![],
        created_at: String::new(),
    }
}

type Side = (Option<usize>, i32, Option<String>);

fn ladder(criteria: &QualityProfileCriteria, first: Side, second: Side) -> (i64, String) {
    let candidate =
        CandidateFacts::new(first.0, first.1, 0).with_profile_policy(first.2.as_deref(), criteria);
    let outcome = ladder_against_incumbent(candidate, &held(second.0, second.1, second.2));
    let sign = match outcome {
        LadderOutcome::Won(_) => 1,
        LadderOutcome::Lost(_) => -1,
        LadderOutcome::ReachedScore => 0,
    };
    (sign, format!("{outcome:?}"))
}

fn cases(app: &str, file: &str, op: &str) -> Vec<Value> {
    let all: Vec<Value> = corpus(&format!("corpus/reference/{app}/quality/{file}.json"));
    all.into_iter().filter(|c| c["input"]["op"] == op).collect()
}

fn id(c: &Value) -> &str {
    c["id"].as_str().unwrap()
}

// ── QualityIndex.CompareTo ──────────────────────────────────────────────────

fn index_suite(name: &str, app: &str) {
    let Some(mut suite) = Suite::open(name) else {
        return;
    };
    let mut criteria = builtin_default_quality_profile().criteria;
    criteria.quality_tiers = vec!["480P".into(), "720P".into(), "1080P".into()];
    for c in cases(app, "quality_index_compare", "quality_index_compare") {
        if !suite.wants(id(&c)) {
            continue;
        }
        let g = &c["input"]["given"];
        let side = |s: &str| -> Side {
            let i = g[s]["index"].as_u64().unwrap() as usize;
            (quality_tier_index(&criteria, Some(INDEX_LABELS[i])), 0, Some(GROUP.to_string()))
        };
        let (actual, detail) = ladder(&criteria, side("left"), side("right"));
        record(&mut suite, id(&c), c["expect"]["result"].as_i64().unwrap(), actual, &detail);
    }
    suite.finish();
}

#[test]
fn son_quality_index_compare() {
    index_suite("scryer-application::parity_quality::son_quality_index_compare", "sonarr");
}

#[test]
fn rad_quality_index_compare() {
    index_suite("scryer-application::parity_quality::rad_quality_index_compare", "radarr");
}

// ── Revision ────────────────────────────────────────────────────────────────

fn quality_row(name: &str) -> &'static (&'static str, Option<&'static str>, bool, Option<u32>) {
    QUALITIES
        .iter()
        .find(|q| q.0 == name)
        .unwrap_or_else(|| panic!("quality `{name}` missing from QUALITIES"))
}

/// Resolution lines of a quality that has both a source and lines in the table.
fn lines_of(name: &str) -> Option<u32> {
    let row = quality_row(name);
    row.1.and(row.3)
}

/// Parsed (revision, group) of a release carrying the case's revision tokens.
fn revision_of(rev: &Value) -> (i32, Option<String>) {
    let real = rev["real"].as_u64().unwrap();
    let tokens = "REAL.".repeat(real as usize)
        + match rev["version"].as_u64().unwrap() {
            1 => "",
            2 => "PROPER.",
            v => panic!("harness table: version {v}"),
        };
    let parsed = parse_release_metadata(&format!("Parity.Show.S01E01.{tokens}1080p.WEB-DL.x264-{GROUP}"));
    assert_eq!(parsed.release_group.as_deref(), Some(GROUP), "template must parse the group");
    (parsed.revision, parsed.release_group)
}

fn revision_suite(name: &str, app: &str) {
    let Some(mut suite) = Suite::open(name) else {
        return;
    };
    let criteria = builtin_default_quality_profile().criteria;
    let tier = quality_tier_index(&criteria, Some("1080p"));
    for c in cases(app, "revision_compare", "revision_compare") {
        let g = &c["input"]["given"];
        if g["first"].is_null() || g["second"].is_null() || !suite.wants(id(&c)) {
            continue;
        }
        let (r1, g1) = revision_of(&g["first"]);
        let (r2, g2) = revision_of(&g["second"]);
        let (sign, outcome) = ladder(&criteria, (tier, r1, g1), (tier, r2, g2));
        let detail = format!("{outcome}; parsed revision {r1} vs {r2}");
        let e = &c["expect"];
        if e.get("sign").is_some() {
            assert_eq!(e.as_object().unwrap().len(), 1, "{}", id(&c));
            record(&mut suite, id(&c), e["sign"].as_i64().unwrap(), sign, &detail);
        } else {
            // Operator form: exactly the six operators, all derived from one sign.
            let mut keys: Vec<&str> = e.as_object().unwrap().keys().map(String::as_str).collect();
            keys.sort();
            assert_eq!(keys, ["eq", "ge", "gt", "le", "lt", "ne"], "{}", id(&c));
            let actual = [sign == 0, sign >= 0, sign > 0, sign <= 0, sign < 0, sign != 0];
            let expected = ["eq", "ge", "gt", "le", "lt", "ne"].map(|k| e[k].as_bool().unwrap());
            record(&mut suite, id(&c), expected, actual, &detail);
        }
    }
    suite.finish();
}

#[test]
fn son_revision_compare() {
    revision_suite("scryer-application::parity_quality::son_revision_compare", "sonarr");
}

#[test]
fn rad_revision_compare() {
    revision_suite("scryer-application::parity_quality::rad_revision_compare", "radarr");
}

// ── QualityModelComparer ────────────────────────────────────────────────────

/// Allowed profile items, worst-first in the corpus, as best-first `<lines>P` tiers.
fn profile_tiers(items: &[Value]) -> Vec<String> {
    let mut names = vec![];
    for item in items.iter().rev() {
        if !item["allowed"].as_bool().unwrap() {
            continue;
        }
        match item["items"].as_array() {
            Some(members) => names.extend(
                members
                    .iter()
                    .filter(|m| m["allowed"].as_bool().unwrap())
                    .map(|m| m["quality"].as_str().unwrap().to_string()),
            ),
            None => names.push(item["quality"].as_str().unwrap().to_string()),
        }
    }
    let mut tiers: Vec<String> = vec![];
    for n in names {
        if let Some(lines) = lines_of(&n) {
            let t = format!("{lines}P");
            if !tiers.contains(&t) {
                tiers.push(t);
            }
        }
    }
    tiers
}

fn model_suite(name: &str, app: &str) {
    let Some(mut suite) = Suite::open(name) else {
        return;
    };
    for c in cases(app, "quality_model_compare", "compare") {
        if !suite.wants(id(&c)) {
            continue;
        }
        let g = &c["input"]["given"];
        let mut criteria = builtin_default_quality_profile().criteria;
        criteria.quality_tiers = profile_tiers(g["profile_items"].as_array().unwrap());
        let side = |s: &Value| -> Option<Side> {
            lines_of(s["quality"].as_str().unwrap()).map(|lines| {
                let (rev, group) = revision_of(&s["revision"]);
                (quality_tier_index(&criteria, Some(&format!("{lines}p"))), rev, group)
            })
        };
        let (Some(first), Some(second)) = (side(&g["first"]), side(&g["second"])) else {
            suite.record(id(&c), false, "quality has no Scryer equivalent (no source or resolution)");
            continue;
        };
        let (actual, detail) = ladder(&criteria, first, second);
        record(&mut suite, id(&c), c["expect"]["sign"].as_i64().unwrap(), actual, &detail);
    }
    suite.finish();
}

#[test]
fn son_quality_model_compare() {
    model_suite("scryer-application::parity_quality::son_quality_model_compare", "sonarr");
}

#[test]
fn rad_quality_model_compare() {
    model_suite("scryer-application::parity_quality::rad_quality_model_compare", "radarr");
}
