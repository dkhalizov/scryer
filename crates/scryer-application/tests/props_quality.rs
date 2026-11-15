//! Parity net, phase 5: property tests for quality ordering and the cutoff
//! (notes/11-parity-net-design.md § Checks beyond examples).
//!
//! Ordering under test: `resolution_rank` (lower is better), read through
//! `resolution_lines`; tier lookup: `normalize_quality_tier` +
//! `quality_tier_index`; cutoff: `quality_meets_or_exceeds_cutoff` and
//! `QualityProfileCriteria::quality_cutoff_met`; probe labels:
//! `quality_from_video_dimensions`. Fixed seed, no persistence: deterministic.

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, RngSeed};
use scryer_application::parity_seams::{
    evaluate_profile_requirements, normalize_quality_tier, quality_from_video_dimensions,
    quality_meets_or_exceeds_cutoff, quality_tier_index, quality_tier_label, release_tier_label,
    resolution_lines, resolution_rank,
};
use scryer_application::{
    DecisionGate, QualityProfileCriteria, builtin_default_quality_profile, parse_release_metadata,
};

fn config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        rng_seed: RngSeed::Fixed(0x5c71_e7a1),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

const SOURCES: &[&str] = &[
    "Bluray", "WEBDL", "WEBRip", "HDTV", "Remux", "DVD", "Raw-HD",
];
const NAMED: &[&str] = &[
    "SDTV",
    "DVD",
    "DVD-R",
    "Raw-HD",
    "BR-DISK",
    "Unknown",
    "4K",
    "UHD",
    "Bluray-1080p Remux",
    "Remux-2160p",
    "WEBDL-480p",
    "1080i",
    "2160p",
    "720p",
    "576p",
    "480p",
    "4320p",
];

/// Quality labels as profiles, parsers and the arr APIs spell them, plus junk.
fn label() -> impl Strategy<Value = String> {
    prop_oneof![
        (
            100u32..=10_000,
            prop::sample::select(vec!["p", "P", "i", "I", ""])
        )
            .prop_map(|(lines, suffix)| format!("{lines}{suffix}")),
        (
            prop::sample::select(SOURCES),
            100u32..=4320,
            prop::sample::select(vec!["p", "i"])
        )
            .prop_map(|(source, lines, suffix)| format!("{source}-{lines}{suffix}")),
        prop::sample::select(NAMED).prop_map(String::from),
        "[ -~]{0,24}",
        "\\PC{0,16}",
    ]
}

fn rank(label: &str) -> u32 {
    resolution_rank(label)
}

/// Distinct resolutions sorted best first, as Scryer stores tiers ("1080P").
fn canonical_tiers() -> impl Strategy<Value = Vec<u32>> {
    prop::collection::btree_set(100u32..=10_000, 1..8)
        .prop_map(|set| set.into_iter().rev().collect::<Vec<_>>())
}

fn tier_names(lines: &[u32]) -> Vec<String> {
    lines.iter().map(|lines| format!("{lines}P")).collect()
}

/// A spelling of tier `lines` that the profile lookup must treat as that tier.
fn spelling(lines: u32, variant: u8) -> String {
    match variant % 4 {
        0 => format!("{lines}p"),
        1 => format!("{lines}P"),
        2 => format!("  {lines}p "),
        _ => format!("\t{lines}P"),
    }
}

/// Tier lists of normalized labels (any label, not only resolutions),
/// deduplicated, no empty name.
fn normalized_tiers() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(label(), 1..10).prop_filter_map("no usable tier", |labels| {
        let mut tiers: Vec<String> = Vec::new();
        for label in labels {
            let tier = normalize_quality_tier(Some(&label)).expect("Some in, Some out");
            if !tier.is_empty() && !tiers.contains(&tier) {
                tiers.push(tier);
            }
        }
        (!tiers.is_empty()).then_some(tiers)
    })
}

fn criteria_with(tiers: Vec<String>, cutoff: &str) -> QualityProfileCriteria {
    let mut criteria = builtin_default_quality_profile().criteria;
    criteria.quality_tiers = tiers;
    criteria.cutoff_tier = Some(cutoff.to_string());
    criteria
}

/// Two labels whose line counts differ by at most 2, in any spelling: the
/// pairs a coarse rank (bucketing, halving) would merge.
fn adjacent_pair() -> impl Strategy<Value = (String, String)> {
    (100u32..=10_000, 0u32..=2, any::<u8>(), any::<u8>(), prop::sample::select(SOURCES)).prop_map(
        |(lines, delta, va, vb, source)| {
            let b = if vb % 5 == 4 {
                format!("{source}-{}p", lines + delta)
            } else {
                spelling(lines + delta, vb)
            };
            (spelling(lines, va), b)
        },
    )
}

const W_EDGES: &[i32] = &[0, 1, 1000, 1200, 1800, 2400, 3200, 7680];
const H_EDGES: &[i32] = &[0, 1, 560, 700, 1000, 1300, 2100, 4200];

/// A probe dimension: often within 3 of one of the label table's thresholds,
/// otherwise anywhere in a plausible range.
fn dimension(edges: &'static [i32], max: i32) -> impl Strategy<Value = i32> {
    prop_oneof![
        (prop::sample::select(edges), -3i32..=3).prop_map(|(edge, offset)| edge + offset),
        -100i32..max,
    ]
}

/// A cutoff/quality argument: one of the tiers (as stored, or padded) most of
/// the time, so the both-listed cases are common, otherwise any label.
fn tier_or_label(tiers: Vec<String>) -> impl Strategy<Value = String> {
    prop_oneof![
        2 => (prop::sample::select(tiers), any::<bool>())
            .prop_map(|(tier, pad)| if pad { format!(" {tier}\t") } else { tier }),
        1 => label(),
    ]
}

// ── 1. Ordering ─────────────────────────────────────────────────────────────
//
// Totality and transitivity of `resolution_rank` are not checked: it returns
// a u32, so they hold for any implementation. What can break is the mapping
// from lines to rank, checked below.

proptest! {
    #![proptest_config(config(2048))]

    #[test]
    fn rank_order_is_antisymmetric(
        (a, b) in prop_oneof![(label(), label()), adjacent_pair()],
    ) {
        // A preorder on labels: mutual <= means they name the same resolution.
        if rank(&a) <= rank(&b) && rank(&b) <= rank(&a) {
            prop_assert_eq!(resolution_lines(Some(&a)), resolution_lines(Some(&b)));
        }
    }

    #[test]
    fn rank_prefers_more_lines_and_ignores_spelling(
        more in 101u32..=10_000,
        gap in prop_oneof![1u32..=3, 1u32..=9_900],
        source in prop::sample::select(SOURCES),
        junk in "[A-Za-z]{0,12}",
    ) {
        let fewer = more.saturating_sub(gap).max(100);
        prop_assume!(fewer < more);
        let canonical = rank(&format!("{more}p"));
        let worse = rank(&format!("{fewer}p"));
        prop_assert!(canonical < worse);
        for spelled in [
            format!("{more}P"),
            format!("{more}i"),
            format!("{more}"),
            format!("{source}-{more}p"),
            format!("{source}-{more}p Remux"),
        ] {
            prop_assert_eq!(rank(&spelled), canonical, "spelling {:?}", spelled);
        }
        // A label that names no resolution ranks after every one that does.
        if resolution_lines(Some(&junk)).is_none() {
            prop_assert!(rank(&junk) > canonical);
        }
    }

    #[test]
    fn normalize_quality_tier_is_idempotent(raw in label()) {
        let once = normalize_quality_tier(Some(&raw));
        let twice = normalize_quality_tier(once.as_deref());
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn tier_index_agrees_with_rank_on_resolution_tiers(
        lines in canonical_tiers(),
        i in any::<prop::sample::Index>(),
        j in any::<prop::sample::Index>(),
        vi in any::<u8>(),
        vj in any::<u8>(),
    ) {
        let mut criteria = builtin_default_quality_profile().criteria;
        criteria.quality_tiers = tier_names(&lines);
        let (a, b) = (spelling(*i.get(&lines), vi), spelling(*j.get(&lines), vj));
        let (ia, ib) = (
            quality_tier_index(&criteria, Some(&a)),
            quality_tier_index(&criteria, Some(&b)),
        );
        prop_assert_eq!(ia, Some(i.index(lines.len())), "label {:?}", a);
        prop_assert_eq!(ib, Some(j.index(lines.len())), "label {:?}", b);
        prop_assert_eq!(ia.cmp(&ib), rank(&a).cmp(&rank(&b)));
    }

    #[test]
    fn video_dimensions_monotone_in_width(
        wa in dimension(W_EDGES, 10_000),
        wb in dimension(W_EDGES, 10_000),
        // half the time below every height threshold, so width alone decides
        h in prop_oneof![-10i32..560, dimension(H_EDGES, 6_000)],
    ) {
        let (w1, w2) = (wa.min(wb), wa.max(wb));
        let low = quality_from_video_dimensions(Some(w1), Some(h));
        let high = quality_from_video_dimensions(Some(w2), Some(h));
        prop_assert!(
            rank(high.unwrap_or("")) <= rank(low.unwrap_or("")),
            "w {} -> {:?}, w {} -> {:?} (h {})", w1, low, w2, high, h
        );
    }

    #[test]
    fn video_dimensions_monotone_in_height(
        // half the time below every width threshold, so height alone decides
        w in prop_oneof![-10i32..1000, dimension(W_EDGES, 10_000)],
        ha in dimension(H_EDGES, 6_000),
        hb in dimension(H_EDGES, 6_000),
    ) {
        let (h1, h2) = (ha.min(hb), ha.max(hb));
        let low = quality_from_video_dimensions(Some(w), Some(h1));
        let high = quality_from_video_dimensions(Some(w), Some(h2));
        prop_assert!(
            rank(high.unwrap_or("")) <= rank(low.unwrap_or("")),
            "h {} -> {:?}, h {} -> {:?} (w {})", h1, low, h2, high, w
        );
    }

    #[test]
    fn video_dimension_labels_rank_as_their_lines(
        w in prop::option::of(prop_oneof![4 => dimension(W_EDGES, 8_000), 1 => any::<i32>()]),
        h in prop::option::of(prop_oneof![4 => dimension(H_EDGES, 4_500), 1 => any::<i32>()]),
    ) {
        if let Some(label) = quality_from_video_dimensions(w, h) {
            let lines = resolution_lines(Some(label));
            prop_assert!(lines.is_some(), "label {} names no resolution", label);
            prop_assert_eq!(rank(label), u32::MAX - lines.unwrap_or_default());
            // and it is a key the tier list can hold
            prop_assert_eq!(
                normalize_quality_tier(Some(label)),
                Some(format!("{}P", lines.unwrap_or_default()))
            );
        }
    }
}

// ── 2. Cutoff ───────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(config(2048))]

    #[test]
    fn cutoff_better_quality_still_meets(
        tiers in normalized_tiers(),
        q in any::<prop::sample::Index>(),
        better in any::<prop::sample::Index>(),
        cutoff in any::<prop::sample::Index>(),
    ) {
        let (qi, ci) = (q.index(tiers.len()), cutoff.index(tiers.len()));
        let bi = better.index(qi + 1); // 0..=qi: ranked at least as high as q
        if quality_meets_or_exceeds_cutoff(&tiers[qi], &tiers[ci], &tiers) {
            prop_assert!(quality_meets_or_exceeds_cutoff(&tiers[bi], &tiers[ci], &tiers));
        }
    }

    #[test]
    fn cutoff_lowering_never_unmeets(
        tiers in normalized_tiers(),
        q in label(),
        cutoff in any::<prop::sample::Index>(),
        lower in any::<prop::sample::Index>(),
    ) {
        let ci = cutoff.index(tiers.len());
        let li = ci + lower.index(tiers.len() - ci); // ci..len: same or lower cutoff
        if quality_meets_or_exceeds_cutoff(&q, &tiers[ci], &tiers) {
            prop_assert!(quality_meets_or_exceeds_cutoff(&q, &tiers[li], &tiers));
        }
    }

    #[test]
    fn cutoff_matches_tier_index(
        (tiers, q, cutoff) in normalized_tiers().prop_flat_map(|tiers| (
            Just(tiers.clone()),
            tier_or_label(tiers.clone()),
            tier_or_label(tiers),
        )),
    ) {
        let criteria = criteria_with(tiers.clone(), &cutoff);
        let expected = matches!(
            (quality_tier_index(&criteria, Some(&q)), quality_tier_index(&criteria, Some(&cutoff))),
            (Some(qi), Some(ci)) if qi <= ci
        );
        prop_assert_eq!(quality_meets_or_exceeds_cutoff(&q, &cutoff, &tiers), expected);
    }

    #[test]
    fn profile_cutoff_met_follows_tier_cutoff(
        tiers in normalized_tiers(),
        q in any::<prop::sample::Index>(),
        better in any::<prop::sample::Index>(),
        cutoff in any::<prop::sample::Index>(),
        lower in any::<prop::sample::Index>(),
        free in prop_oneof![4 => label(), 1 => prop::sample::select(vec!["", " ", "  ", "\t", " \n "]).prop_map(String::from)],
    ) {
        let (qi, ci) = (q.index(tiers.len()), cutoff.index(tiers.len()));
        let bi = better.index(qi + 1);
        let li = ci + lower.index(tiers.len() - ci);
        let criteria = criteria_with(tiers.clone(), &tiers[ci]);
        prop_assert!(criteria.decision_pipeline.contains(DecisionGate::QualityTier));

        // With the tier gate the profile's verdict is the tier cutoff's.
        for quality in [tiers[qi].as_str(), free.as_str()] {
            let applies = !quality.trim().is_empty();
            prop_assert_eq!(
                criteria.quality_cutoff_met(Some(quality)),
                applies && quality_meets_or_exceeds_cutoff(quality.trim(), &tiers[ci], &tiers),
                "quality {:?}", quality
            );
        }
        if criteria.quality_cutoff_met(Some(&tiers[qi])) {
            prop_assert!(criteria.quality_cutoff_met(Some(&tiers[bi])));
            let lowered = criteria_with(tiers.clone(), &tiers[li]);
            prop_assert!(lowered.quality_cutoff_met(Some(&tiers[qi])));
        }

        // Without it any present quality meets the (absent) quality half.
        let mut ungated = criteria.clone();
        ungated.decision_pipeline = ungated.decision_pipeline.without(DecisionGate::QualityTier);
        prop_assert_eq!(ungated.quality_cutoff_met(Some(&free)), !free.trim().is_empty());
        prop_assert!(!ungated.quality_cutoff_met(None));
    }
}

// ── 3. Source tiers (P11) ──────────────────────────────────────────────────

/// A Remux tier above each resolution's group: Remux-2160p is the cutoff.
#[test]
fn source_tiers_split_a_resolution() {
    let tiers: Vec<String> = ["REMUX-2160P", "2160P", "REMUX-1080P", "1080P"]
        .map(String::from)
        .to_vec();
    let criteria = criteria_with(tiers.clone(), "REMUX-2160P");
    let index = |q: Option<&str>| quality_tier_index(&criteria, q);

    let web = quality_tier_label(Some("2160p"), Some("WEB-DL"));
    let remux = quality_tier_label(Some("2160p"), Some("Remux"));
    assert_eq!(web.as_deref(), Some("WEBDL-2160P"));
    assert_eq!(remux.as_deref(), Some("REMUX-2160P"));
    assert_eq!(index(web.as_deref()), Some(1));
    assert_eq!(index(remux.as_deref()), Some(0));
    assert!(!criteria.quality_cutoff_met(web.as_deref()));
    assert!(criteria.quality_cutoff_met(remux.as_deref()));
    assert_eq!(
        index(quality_tier_label(Some("1080p"), Some("Remux")).as_deref()),
        Some(2)
    );
    assert_eq!(
        index(quality_tier_label(Some("1080p"), Some("BluRay")).as_deref()),
        Some(3)
    );

    let mut profile = builtin_default_quality_profile();
    profile.criteria = criteria.clone();
    for (name, expected) in [
        ("Movie.2023.2160p.WEB-DL.DDP5.1.H.265-GRP", 1),
        (
            "Movie.2023.2160p.UHD.BluRay.REMUX.HDR.HEVC.TrueHD.7.1-GRP",
            0,
        ),
        ("Movie.2023.1080p.BluRay.REMUX.AVC.DTS-HD.MA.5.1-GRP", 2),
        ("Movie.2023.1080p.BluRay.x264-GRP", 3),
    ] {
        let parsed = parse_release_metadata(name);
        let key = release_tier_label(&parsed);
        assert_eq!(index(key.as_deref()), Some(expected), "{name} -> {key:?}");
        let decision = evaluate_profile_requirements(&profile, &parsed, false, None);
        assert_eq!(decision.tier_index, Some(expected), "{name}");
        assert_eq!(
            criteria.quality_cutoff_met(key.as_deref()),
            expected == 0,
            "{name}"
        );
    }
}

proptest! {
    #![proptest_config(config(2048))]

    /// A profile of bare resolutions ranks a sourced label as its resolution.
    #[test]
    fn bare_tiers_ignore_source(
        lines in canonical_tiers(),
        i in any::<prop::sample::Index>(),
        c in any::<prop::sample::Index>(),
        v in any::<u8>(),
        source in prop::option::of(prop_oneof![
            // fork (P33e, D78): Raw-HD has no bare fallback; see below.
            prop::sample::select(SOURCES).prop_filter("Raw-HD", |s| *s != "Raw-HD").prop_map(String::from),
            prop::sample::select(vec!["WEB-DL", "BluRay", "BRDISK", "Remux", "DVD"]).prop_map(String::from),
            "[ -~]{0,12}",
        ]),
    ) {
        let tiers = tier_names(&lines);
        let criteria = criteria_with(tiers.clone(), &tiers[c.index(tiers.len())]);
        let bare = spelling(*i.get(&lines), v);
        let key = quality_tier_label(Some(&bare), source.as_deref());
        prop_assert_eq!(
            quality_tier_index(&criteria, key.as_deref()),
            Some(i.index(lines.len())),
            "key {:?}", key
        );
        prop_assert_eq!(
            criteria.quality_cutoff_met(key.as_deref()),
            criteria.quality_cutoff_met(Some(&bare))
        );
    }

    /// fork (P33e, D78): a Raw-HD key never resolves to a bare tier; only an
    /// explicit `RAWHD-<res>` tier ranks it.
    #[test]
    fn raw_hd_never_falls_back_to_a_bare_tier(
        lines in canonical_tiers(),
        i in any::<prop::sample::Index>(),
        v in any::<u8>(),
        source in prop::sample::select(vec!["Raw-HD", "RawHD", "rawhd", " RAW-HD "]),
    ) {
        let tiers = tier_names(&lines);
        let criteria = criteria_with(tiers.clone(), &tiers[0]);
        let key = quality_tier_label(Some(&spelling(*i.get(&lines), v)), Some(source));
        prop_assert_eq!(quality_tier_index(&criteria, key.as_deref()), None, "key {:?}", key);
        let raw_hd = key.clone().expect("a resolution gives a key");
        let mut explicit = tiers;
        explicit.insert(0, raw_hd.clone());
        let criteria = criteria_with(explicit, &raw_hd);
        prop_assert_eq!(quality_tier_index(&criteria, key.as_deref()), Some(0));
    }
}
