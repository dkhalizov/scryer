//! Kani proofs for pure functions (parity net, design § Checks beyond examples).
//!
//! Built only by `cargo kani`; never part of a normal build. Every proof calls
//! the product function as written and carries `kani::cover!` checks so a
//! vacuous proof (no reachable input) shows up as an UNSATISFIED cover.
//!
//! Status with Kani 0.68 / CBMC 6.11 (2026-11-13). Run with
//! `cargo kani -p scryer-application -Z unstable-options -Z stubbing
//! --harness-timeout 8m --harness kani_proofs::<prefix>`.
//! - VERIFIED in seconds: `video_dimensions_*`, `video_dimension_labels_read_back`
//!   (one explicit call per label), `ownership_requires_a_recorded_folder_{none,empty}`.
//! - What times out is a symbolic choice between strings: symbolic label
//!   bytes (`resolution_*`), a pick among concrete strings (`cutoff_*`,
//!   `quality_cutoff_met_*`; also a `for` over a label array or a merged
//!   `None`/`Some("")`), and symbolic paths. Single concrete string calls
//!   into the quality code verify in seconds. `cutoff_*` / `resolution_*`
//!   stay as stated (symbolic profiles and labels); split into concrete
//!   calls they would only repeat the sampled cases in `tests/props_quality.rs`.
//! - Ownership with a recorded folder times out even on concrete paths with
//!   the ASCII stubs below (symex in `Path::components`, memchr, memcmp).
//!   `ownership_stays_inside_recorded_folder` holds since P13 (the old
//!   counterexample, recorded `/..`, folder `//..`, was a UNC reading of `//` on
//!   POSIX; see `ownership_double_slash_parent_*`); checked exhaustively on 7225
//!   pairs by `folder_ownership` tests, Kani still cannot reach a verdict.

use crate::acquisition::decision_helpers::resolution_rank;
use crate::decision_pipeline::{DecisionGate, DecisionPipeline};
use crate::folder_ownership::title_owns_folder;
use crate::media::release_labels::quality_from_video_dimensions;
use crate::quality_profile::{
    QualityProfileCriteria, quality_meets_or_exceeds_cutoff, quality_tier_index, resolution_lines,
};
use crate::title_folder_rules::{path_is_strictly_within, stored_path_is_inside_folder};

// ---------------------------------------------------------------------------
// quality_from_video_dimensions
// ---------------------------------------------------------------------------

const DIMENSION_LABELS: [&str; 7] = ["480p", "576p", "720p", "1080p", "1440p", "2160p", "4320p"];

/// 0 for `None`, then 1..=7 in the documented label order.
fn label_rank(label: Option<&str>) -> usize {
    match label {
        None => 0,
        Some(label) => {
            let position = DIMENSION_LABELS.iter().position(|known| *known == label);
            assert!(position.is_some(), "label outside the documented set");
            position.unwrap() + 1
        }
    }
}

fn any_dimension() -> Option<i32> {
    if kani::any() { Some(kani::any()) } else { None }
}

/// Total over every `i32` (and `None`) width/height, label set as documented.
/// The `None`-as-0 assert only pins the documented contract (the product's
/// first line is `unwrap_or_default`); the label-set and non-positive-pair
/// asserts carry the proof.
#[kani::proof]
#[kani::unwind(9)]
fn video_dimensions_total_and_label_set() {
    let width = any_dimension();
    let height = any_dimension();
    let label = quality_from_video_dimensions(width, height);
    let _ = label_rank(label);
    assert_eq!(
        label,
        quality_from_video_dimensions(Some(width.unwrap_or(0)), Some(height.unwrap_or(0)))
    );
    // A label needs a positive dimension; a non-positive pair is unlabelled.
    if width.unwrap_or(0) <= 0 && height.unwrap_or(0) <= 0 {
        assert!(label.is_none());
    }
    kani::cover!(label.is_none());
    kani::cover!(label == Some("480p"));
    kani::cover!(label == Some("576p"));
    kani::cover!(label == Some("720p"));
    kani::cover!(label == Some("1080p"));
    kani::cover!(label == Some("1440p"));
    kani::cover!(label == Some("2160p"));
    kani::cover!(label == Some("4320p"));
}

/// Every label `quality_from_video_dimensions` emits reads back through
/// `resolution_lines` as its own line count, so the two resolution readers
/// agree. One explicit call per label: a `for` over the label array makes
/// CBMC merge the seven strings and times out; separate calls verify.
#[kani::proof]
#[kani::unwind(9)]
fn video_dimension_labels_read_back() {
    // `label_rank` (above) pins the emitted set to exactly these seven.
    assert_eq!(resolution_lines(Some("480p")), Some(480));
    assert_eq!(resolution_lines(Some("576p")), Some(576));
    assert_eq!(resolution_lines(Some("720p")), Some(720));
    assert_eq!(resolution_lines(Some("1080p")), Some(1080));
    assert_eq!(resolution_lines(Some("1440p")), Some(1440));
    assert_eq!(resolution_lines(Some("2160p")), Some(2160));
    let top = resolution_lines(Some("4320p"));
    assert_eq!(top, Some(4320));
    kani::cover!(top == Some(4320));
}

/// Monotone in each argument: a larger width (height fixed) or a larger
/// height (width fixed) never yields a lower label.
#[kani::proof]
#[kani::unwind(9)]
fn video_dimensions_monotone() {
    let a: i32 = kani::any();
    let b: i32 = kani::any();
    let other: i32 = kani::any();
    kani::assume(a <= b);
    let by_width = (
        label_rank(quality_from_video_dimensions(Some(a), Some(other))),
        label_rank(quality_from_video_dimensions(Some(b), Some(other))),
    );
    let by_height = (
        label_rank(quality_from_video_dimensions(Some(other), Some(a))),
        label_rank(quality_from_video_dimensions(Some(other), Some(b))),
    );
    assert!(by_width.0 <= by_width.1);
    assert!(by_height.0 <= by_height.1);
    kani::cover!(by_width.0 < by_width.1);
    kani::cover!(by_height.0 < by_height.1);
    kani::cover!(a < b && by_width.0 == by_width.1);
}

/// Height thresholds 4200/2100/1300/1000/700/560 and width thresholds
/// 7680/3200/2400/1800/1200/1000, exactly as the code states: each axis alone
/// (the other at 1, so the 480p floor applies) picks the documented band.
#[kani::proof]
#[kani::unwind(8)]
fn video_dimensions_thresholds() {
    let value: i32 = kani::any();
    let expected = |thresholds: [i32; 6]| -> Option<&'static str> {
        let labels = ["4320p", "2160p", "1440p", "1080p", "720p", "576p"];
        for (threshold, label) in thresholds.iter().zip(labels) {
            if value >= *threshold {
                return Some(label);
            }
        }
        (value > 0).then_some("480p")
    };
    let by_height = quality_from_video_dimensions(Some(1), Some(value));
    let by_width = quality_from_video_dimensions(Some(value), Some(1));
    assert_eq!(by_height, expected([4200, 2100, 1300, 1000, 700, 560]));
    assert_eq!(by_width, expected([7680, 3200, 2400, 1800, 1200, 1000]));
    kani::cover!(value == 4199 && by_height == Some("2160p"));
    kani::cover!(value == 560 && by_height == Some("576p"));
    kani::cover!(value == 0 && by_height.is_none());
}

// ---------------------------------------------------------------------------
// resolution_lines / resolution_rank
// ---------------------------------------------------------------------------

const LABEL_ALPHABET: [u8; 8] = *b"0127pi- ";

/// A bounded symbolic ASCII label over a small alphabet.
fn any_label<const N: usize>() -> String {
    let bytes: [u8; N] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= N);
    for byte in &bytes[..len] {
        kani::assume(LABEL_ALPHABET.contains(byte));
    }
    // SAFETY: every byte is ASCII (assumed above).
    unsafe { String::from_utf8_unchecked(bytes[..len].to_vec()) }
}

/// `resolution_lines` only reports plausible line counts; `resolution_rank`
/// is `u32::MAX` exactly for "no resolution" and every resolution outranks it.
#[kani::proof]
#[kani::unwind(6)]
fn resolution_lines_and_rank_bounds() {
    let label = any_label::<4>();
    let lines = resolution_lines(Some(&label));
    let rank = resolution_rank(&label);
    if let Some(lines) = lines {
        assert!((100..=10_000).contains(&lines));
        assert_eq!(rank, u32::MAX - lines);
        assert!(rank < u32::MAX);
    } else {
        assert_eq!(rank, u32::MAX);
    }
    // Case never matters.
    assert_eq!(lines, resolution_lines(Some(&label.to_ascii_uppercase())));
    kani::cover!(lines == Some(720) && label.ends_with('p'));
    kani::cover!(lines == Some(720) && label.ends_with('i'));
    kani::cover!(lines.is_none() && !label.is_empty());
}

/// Lower rank is strictly better resolution: rank order is the reverse of
/// line order, and equal lines tie.
#[kani::proof]
#[kani::unwind(6)]
fn resolution_rank_orders_by_lines() {
    let left = any_label::<4>();
    let right = any_label::<4>();
    let (left_lines, right_lines) = (
        resolution_lines(Some(&left)).unwrap_or(0),
        resolution_lines(Some(&right)).unwrap_or(0),
    );
    let (left_rank, right_rank) = (resolution_rank(&left), resolution_rank(&right));
    assert_eq!(left_lines.cmp(&right_lines), right_rank.cmp(&left_rank));
    kani::cover!(left_lines > right_lines && right_lines > 0);
    kani::cover!(left_lines == right_lines && left != right && left_lines > 0);
}

// ---------------------------------------------------------------------------
// quality_tier_index / quality_meets_or_exceeds_cutoff / quality_cutoff_met
// ---------------------------------------------------------------------------

/// Tier labels as a parsed profile stores them (normalized, see
/// `normalize_list` at profile parse).
const TIERS: [&str; 4] = ["2160P", "1080P", "720P", "SDTV"];
/// Query spellings: stored labels in other cases and padding, an
/// interlaced label, an unlisted label, blank.
const QUERIES: [&str; 6] = ["1080p", " 720P", "sdtv", "1080i", "x", ""];

fn any_of<'a>(values: &[&'a str]) -> &'a str {
    let index: usize = kani::any();
    kani::assume(index < values.len());
    values[index]
}

/// Up to three tiers picked from `TIERS` (duplicates allowed, any order).
fn any_tiers() -> Vec<String> {
    let len: usize = kani::any();
    kani::assume(len <= 3);
    (0..len).map(|_| any_of(&TIERS).to_string()).collect()
}

/// A profile with the given tiers and cutoff, otherwise as
/// `QualityProfileCriteria::default()`. Written out because `default()`
/// builds a `HashMap::new()`, whose `RandomState` reads the OS random source
/// (a foreign call Kani cannot model); a fixed-key hasher stands in for it.
fn criteria_with(tiers: Vec<String>, cutoff: Option<&str>) -> QualityProfileCriteria {
    // SAFETY: `RandomState` is two `u64` keys; any value is a valid key pair.
    let hasher: std::hash::RandomState = unsafe { std::mem::transmute([0u64; 2]) };
    QualityProfileCriteria {
        quality_tiers: tiers,
        archival_quality: Some("1080P".to_string()),
        allow_unknown_quality: false,
        source_allowlist: Vec::new(),
        source_blocklist: Vec::new(),
        video_codec_allowlist: Vec::new(),
        video_codec_blocklist: Vec::new(),
        audio_codec_allowlist: Vec::new(),
        audio_codec_blocklist: Vec::new(),
        atmos_preferred: false,
        dolby_vision_allowed: true,
        detected_hdr_allowed: true,
        prefer_remux: false,
        allow_bd_disk: true,
        allow_upgrades: true,
        ignore_proper_repack: false,
        prefer_dual_audio: false,
        required_audio_languages: Vec::new(),
        scoring_persona: Default::default(),
        scoring_overrides: Default::default(),
        cutoff_tier: cutoff.map(str::to_string),
        min_score_to_grab: None,
        cutoff_score: None,
        min_upgrade_score_delta: None,
        decision_pipeline: DecisionPipeline::default_order(),
        facet_persona_overrides: std::collections::HashMap::with_hasher(hasher),
    }
}

/// The cutoff check and the tier index are one notion of "better": meets iff
/// both are listed and the quality's index is at or before the cutoff's.
#[kani::proof]
#[kani::unwind(7)]
fn cutoff_agrees_with_tier_index() {
    let tiers = any_tiers();
    let quality = if kani::any() { any_of(&QUERIES) } else { any_of(&TIERS) };
    let cutoff = if kani::any() { any_of(&QUERIES) } else { any_of(&TIERS) };
    let criteria = criteria_with(tiers.clone(), Some(cutoff));
    let meets = quality_meets_or_exceeds_cutoff(quality, cutoff, &tiers);
    let expected = match (
        quality_tier_index(&criteria, Some(quality)),
        quality_tier_index(&criteria, Some(cutoff)),
    ) {
        (Some(quality_index), Some(cutoff_index)) => quality_index <= cutoff_index,
        _ => false,
    };
    assert_eq!(meets, expected);
    if tiers.is_empty() {
        assert!(!meets);
    }
    kani::cover!(meets);
    kani::cover!(!meets && !tiers.is_empty());
    kani::cover!(meets && quality != cutoff);
}

/// Cutoff monotonicity on bounded profiles: a quality listed at or before
/// one that meets the cutoff meets it too, a cutoff at or after one the
/// quality meets is met too, and every listed tier meets itself.
#[kani::proof]
#[kani::unwind(7)]
fn cutoff_monotone() {
    let tiers = any_tiers();
    kani::assume(!tiers.is_empty());
    let i: usize = kani::any();
    let j: usize = kani::any();
    kani::assume(i <= j && j < tiers.len());
    let probe = if kani::any() { any_of(&QUERIES) } else { any_of(&TIERS) };

    assert!(quality_meets_or_exceeds_cutoff(&tiers[i], &tiers[i], &tiers));
    if quality_meets_or_exceeds_cutoff(&tiers[j], probe, &tiers) {
        assert!(quality_meets_or_exceeds_cutoff(&tiers[i], probe, &tiers));
    }
    if quality_meets_or_exceeds_cutoff(probe, &tiers[i], &tiers) {
        assert!(quality_meets_or_exceeds_cutoff(probe, &tiers[j], &tiers));
    }
    // Better-listed always meets a worse-listed cutoff.
    assert!(quality_meets_or_exceeds_cutoff(&tiers[i], &tiers[j], &tiers));
    kani::cover!(i < j && tiers[i] != tiers[j]);
    kani::cover!(!quality_meets_or_exceeds_cutoff(&tiers[j], &tiers[i], &tiers));
}

/// `quality_cutoff_met`: with the tier gate it is exactly the cutoff check on
/// the trimmed quality; without it any non-blank quality meets.
#[kani::proof]
#[kani::unwind(7)]
fn quality_cutoff_met_matches_gate() {
    let tiers = any_tiers();
    let quality = if kani::any() {
        Some(if kani::any() { any_of(&QUERIES) } else { any_of(&TIERS) })
    } else {
        None
    };
    let cutoff = if kani::any() { Some(any_of(&TIERS)) } else { None };
    let gated: bool = kani::any();
    let mut criteria = criteria_with(tiers.clone(), cutoff);
    criteria.decision_pipeline = if gated {
        DecisionPipeline::default_order()
    } else {
        DecisionPipeline::default_order().without(DecisionGate::QualityTier)
    };
    let met = criteria.quality_cutoff_met(quality);
    let present = quality.map(str::trim).filter(|value| !value.is_empty());
    if gated {
        let expected = match (present, cutoff) {
            (Some(quality), Some(cutoff)) => quality_meets_or_exceeds_cutoff(quality, cutoff, &tiers),
            _ => false,
        };
        assert_eq!(met, expected);
    } else {
        assert_eq!(met, present.is_some());
    }
    kani::cover!(gated && met);
    kani::cover!(gated && !met && present.is_some() && cutoff.is_some());
    kani::cover!(!gated && met);
}

// ---------------------------------------------------------------------------
// title_owns_folder (deletion ownership guard)
// ---------------------------------------------------------------------------

const PATH_ALPHABET: [u8; 3] = *b"/a.";

/// A bounded absolute path over `/a.`, so `.`/`..` segments, doubled and
/// trailing separators and the root itself are all reachable.
fn any_abs_path<const N: usize>() -> String {
    let bytes: [u8; N] = kani::any();
    let len: usize = kani::any();
    kani::assume(len >= 1 && len <= N);
    kani::assume(bytes[0] == b'/');
    for byte in &bytes[..len] {
        kani::assume(PATH_ALPHABET.contains(byte));
    }
    // SAFETY: every byte is ASCII (assumed above).
    unsafe { String::from_utf8_unchecked(bytes[..len].to_vec()) }
}

/// Stand-in for `stored_paths::identity_os_str` (lowercase, then NFC), exact
/// on ASCII: NFC leaves ASCII unchanged and ASCII lowercasing is Unicode
/// lowercasing there. The assert keeps it sound: a non-ASCII input fails the
/// proof instead of being mis-modelled. Without it CBMC spends the whole
/// timeout in `unicode_normalization`'s canonical sort.
fn ascii_identity_os_str(value: &std::ffi::OsStr) -> std::ffi::OsString {
    let text = value.to_str().expect("ASCII path");
    assert!(text.is_ascii(), "stub is exact on ASCII only");
    text.to_ascii_lowercase().into()
}

/// Stand-in for `stored_paths::normalize_identity_unicode` (NFC unless the
/// escape form or ASCII), exact on ASCII, which it asserts. The product
/// guards NFC with `is_ascii()`, but CBMC does not decide that on heap
/// strings and explores the NFC branch anyway.
fn ascii_normalize_identity_unicode(value: &str) -> String {
    assert!(value.is_ascii(), "stub is exact on ASCII only");
    value.to_string()
}

fn title_with_folder(folder_path: Option<String>) -> scryer_domain::Title {
    scryer_domain::Title {
        id: String::new(),
        library_id: String::new(),
        name: String::new(),
        facet: scryer_domain::MediaFacet::Movie,
        monitored: false,
        tags: Vec::new(),
        external_ids: Vec::new(),
        root_folder_id: String::new(),
        created_by: None,
        created_at: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH,
        year: None,
        overview: None,
        poster_url: None,
        poster_source_url: None,
        background_url: None,
        background_source_url: None,
        sort_title: None,
        catalog_sort_key: String::new(),
        slug: None,
        imdb_id: None,
        runtime_minutes: None,
        popularity: None,
        canonical_tags: Vec::new(),
        content_status: None,
        language: None,
        first_aired: None,
        network: None,
        studio: None,
        country: None,
        aliases: Vec::new(),
        tagged_aliases: Vec::new(),
        metadata_language: None,
        metadata_fetched_at: None,
        min_availability: None,
        digital_release_date: None,
        movie_release_dates: None,
        folder_path,
    }
}

/// A title with no recorded folder owns nothing. Split per record: a merged
/// `None`/`Some("")` choice times out, each case alone verifies.
#[kani::proof]
#[kani::unwind(5)]
fn ownership_requires_a_recorded_folder_none() {
    let folder = any_abs_path::<3>();
    let owns = title_owns_folder(&title_with_folder(None), std::path::Path::new(&folder));
    assert!(!owns);
    kani::cover!(folder == "/");
    kani::cover!(folder.len() == 3);
}

#[kani::proof]
#[kani::unwind(5)]
fn ownership_requires_a_recorded_folder_empty() {
    let folder = any_abs_path::<3>();
    let owns = title_owns_folder(
        &title_with_folder(Some(String::new())),
        std::path::Path::new(&folder),
    );
    assert!(!owns);
    kani::cover!(folder == "/");
    kani::cover!(folder.len() == 3);
}

/// Never outside the recorded folder: an owned folder is inside the recorded
/// one and the recorded one inside it (as the scan/import containment check
/// reads paths), and a folder strictly inside or strictly holding the
/// recorded one is never owned. Library-root guard: a root that strictly
/// holds the recorded folder is never owned.
#[kani::proof]
#[kani::unwind(5)]
#[kani::stub(crate::stored_paths::identity_os_str, ascii_identity_os_str)]
#[kani::stub(crate::stored_paths::normalize_identity_unicode, ascii_normalize_identity_unicode)]
fn ownership_stays_inside_recorded_folder() {
    let recorded = any_abs_path::<3>();
    let folder = any_abs_path::<3>();
    let owns = title_owns_folder(
        &title_with_folder(Some(recorded.clone())),
        std::path::Path::new(&folder),
    );
    if owns {
        assert!(stored_path_is_inside_folder(&recorded, &folder));
        assert!(stored_path_is_inside_folder(&folder, &recorded));
    }
    if path_is_strictly_within(&recorded, &folder) || path_is_strictly_within(&folder, &recorded) {
        assert!(!owns);
    }
    kani::cover!(owns && recorded != folder);
    kani::cover!(owns && folder != "/");
    kani::cover!(!owns && path_is_strictly_within(&recorded, &folder));
    // The guard itself does not know library roots: a title whose recorded
    // folder IS the root owns it (refused elsewhere, by
    // `ensure_title_folder_is_not_a_library_root`).
    kani::cover!(owns && folder == "/");
}

/// Witness for the ownership half of the `/..` vs `//..` pair (4 bytes, out of
/// `ownership_stays_inside_recorded_folder`'s N=3 reach; N=4 times out): a
/// title recording `/..` owns `//..`, because `title_owns_folder` compares
/// `Path::components` identities and both spell RootDir, ParentDir. That
/// ownership is correct on POSIX; since P13 containment agrees (see
/// `ownership_double_slash_parent_contained`). Witness only (covers). Split
/// because the combined concrete check times out.
#[kani::proof]
#[kani::unwind(5)]
#[kani::stub(crate::stored_paths::identity_os_str, ascii_identity_os_str)]
#[kani::stub(crate::stored_paths::normalize_identity_unicode, ascii_normalize_identity_unicode)]
fn ownership_double_slash_parent_owned() {
    let owns = title_owns_folder(
        &title_with_folder(Some("/..".to_string())),
        std::path::Path::new("//.."),
    );
    kani::cover!(owns); // expected SATISFIED
    kani::cover!(!owns); // expected UNSATISFIED
}

/// Expected VERIFIED (may still hit the D43 timeout; the gate is the unit test
/// `ownership_and_containment_agree_on_every_short_absolute_path`): with `/..`
/// owning `//..` (harness above), an owned folder is never strictly inside the
/// recorded one. P13: `collapse_dot_segments` reads a leading `//` as a UNC
/// anchor only on Windows, so both spellings collapse to `/` on POSIX.
#[kani::proof]
#[kani::unwind(5)]
#[kani::stub(crate::stored_paths::identity_os_str, ascii_identity_os_str)]
#[kani::stub(crate::stored_paths::normalize_identity_unicode, ascii_normalize_identity_unicode)]
fn ownership_double_slash_parent_contained() {
    assert!(!path_is_strictly_within("//..", "/.."));
}
