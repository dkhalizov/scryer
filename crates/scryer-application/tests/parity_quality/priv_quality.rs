//! Stream `priv_quality`: captured Radarr quality definitions (MB per minute
//! bounds, preferred size) against Scryer's size gate and size fit.
//!
//! Suite: `rad_quality_size_limits`, IDs RAD-QUALITY-5025..5034. Selection
//! (before `wants()`): area `quality`, id prefix `RAD-QUALITY-5`, ref path
//! ending `radarr/config/qualitydefinition.json`, fragment `id=N`. The weight
//! entry (fragment `weight`) and out-of-scope entries are skipped.
//!
//! Scryer functions under test, in the order `run_term_pipeline`
//! (quality/canonical.rs:792-801) calls them: `parity_seams::evaluate_profile_requirements`
//! then `parity_seams::apply_size_requirement` (-> `apply_implausible_size_limits`,
//! `minimum_plausible_bitrate_mbps`; sets `decision.size_fit_penalty`).
//! Profile: `builtin_default_quality_profile` (1080p, Balanced) for every
//! probe, the 2160p ones too. Only the two size block codes count (its tier
//! and source codes are ignored) and the implausibility bounds do not read
//! the profile, but `size_fit_penalty` depends on its persona and
//! `prefer_remux`. Scryer's bounds are fixed per resolution tier and codec,
//! not per-quality settings (D29: mapped, red).
//!
//! # Translation table (reference term -> Scryer term)
//!
//! - `quality.name` (never `title`) -> `super::QUALITIES` row -> neutral probe
//!   title `Parity.Size.Probe.2020.<lines>p.<token>.x264-PARITY`, token HDTV,
//!   WEB-DL, WEBRip, BluRay, or BluRay.REMUX. The parse is asserted (panic)
//!   to give the table's source, remux flag and lines, and a video codec
//!   (without a codec Scryer skips every size check). x264 is a harness
//!   choice: H264 has Scryer's highest floor factor.
//! - Movie runtime -> `CoverageSizeBasis::single(Some(110))`; 110 is Radarr's
//!   own fallback median runtime (AcceptableSizeSpecification.cs:39). The arr
//!   bounds are per minute, so any runtime gives the same arr verdict.
//! - MB -> bytes: `Megabytes()` rounds MB*1024*1024 to a long first
//!   (NumberExtensions.cs:45), then multiplies by the runtime. C#
//!   Convert.ToInt64 rounds half to even, Rust `f64::round` half away from
//!   zero; no difference for integral captured values.
//! - Category `movie`.
//! - Arr verdict per probe (AcceptableSizeSpecification.cs:22-80): accept
//!   when size >= minBytes and (no maximum or size <= maxBytes); null min = 0,
//!   null or 0 max = unlimited.
//! - Probes, computed at run time from the captured `minSize`, `maxSize`,
//!   `preferredSize`: max(minBytes, 1); minBytes-1 when minBytes > 1;
//!   maxBytes and maxBytes+1 when a maximum exists, else 1 TiB; the preferred
//!   size in bytes when set; and interior probes of 2, 5, 10, 20, 40, 80 GiB
//!   (harness constants for a 110-minute movie) kept where the arr accepts
//!   them. Size 0 is not a probe (both apps treat it as unknown).
//! - Scryer rejects when `block_codes` has `size_implausible_for_quality` or
//!   `size_implausibly_small_for_quality`. One failure per probe whose
//!   verdict differs.
//! - Size ranking (DownloadDecisionComparer.cs:166-186), over probes both
//!   apps accept: the arr key is `size` rounded down to 200 MiB (largest
//!   first) when `preferredSize` is null, else `-|size - preferredBytes|`
//!   truncated to 200 MiB (closest first). Scryer ranks by `size_fit_penalty`
//!   (lower is better). For every pair the arr ranks strictly apart, Scryer's
//!   penalty must not be higher for the arr's better probe (ties allowed).
//!   With `preferredSize` set, a ranking failure also names the missing
//!   Scryer setting (per-quality preferred size) as context.
//! - Failures joined into `suite.record(id, none, joined)`.
//!
//! # Ignored keys
//!
//! `title` (editable display name), `weight` (quality_catalogue), `id`
//! (lookup only).
//!
//! # Expected reds
//!
//! Min side: Scryer rejects the 1-byte probe as implausibly small. Max side:
//! Scryer's ceiling differs from the arr maximum (accepts above a low arr
//! maximum, rejects the unlimited probe). Ranking: Scryer's size fit raises
//! the penalty above its band, so it does not keep largest-first or
//! closest-to-preferred.
//!
//! # Unmapped (tests: []), with reasons
//!
//! - Custom-format definitions (SON 5032 5035 5036 5040 5041 5043 5044 5046
//!   5049 5051 5052; RAD 5037 5039-5045 5047 5049 5051 5052 5054-5058): no
//!   positive verdict in any captured source (`data/parse.json` customFormats,
//!   `data/history.json` customFormats, `data/moviefile.json` /
//!   `data/episodefile.json` customFormats); no format name occurs as a
//!   customFormats entry (exact match) under `corpus/captured/<app>/data`.
//!   Negative-only checks would pass even if the rule never fired, and
//!   positive titles would have to be invented. An adopt run of the root
//!   `son_/rad_quality_config` suites still records red on a Scryer false
//!   positive among the parse.json titles.
//! - Profile definitions (SON 5007, RAD 5002, RAD 5005): production has no
//!   arr-profile to Scryer-profile converter (external_import.rs:574 keeps id,
//!   name, language; `resolve_arr_quality_profile_id` only matches an existing
//!   profile and lives in another crate). A harness-built profile would test
//!   the harness. Open decision, re-checked.

use scryer_application::parity_seams::{CoverageSizeBasis, apply_size_requirement, evaluate_profile_requirements};
use scryer_application::{builtin_default_quality_profile, parse_release_metadata};
use scryer_parity::{Catalogue, Suite, corpus};
use serde_json::Value;

const RUNTIME_MIN: i64 = 110;
const FAR_BYTES: i64 = 1 << 40;
const GIB: i64 = 1 << 30;
const INTERIOR_GIB: [i64; 6] = [2, 5, 10, 20, 40, 80];
const MB: f64 = 1_048_576.0;
const BIN: i64 = 200 * 1_048_576;
const BLOCK: [&str; 2] = ["size_implausible_for_quality", "size_implausibly_small_for_quality"];

/// (accepted, size_fit_penalty) from Scryer for one probe size.
fn scryer(title: &str, bytes: i64) -> (bool, i32) {
    let profile = builtin_default_quality_profile();
    let release = parse_release_metadata(title);
    let mut d = evaluate_profile_requirements(&profile, &release, false, Some("movie"));
    apply_size_requirement(&mut d, &profile, &release, Some(bytes), Some("movie"), CoverageSizeBasis::single(Some(RUNTIME_MIN as i32)));
    (!d.block_codes.iter().any(|c| BLOCK.contains(&c.as_str())), d.size_fit_penalty)
}

fn probe_title(name: &str) -> String {
    let (_, source, remux, lines) = super::QUALITIES
        .iter()
        .find(|q| q.0 == name)
        .map(|q| (q.0, q.1, q.2, q.3))
        .unwrap_or_else(|| panic!("quality {name} missing from QUALITIES"));
    let (source, lines) = (source.expect("source"), lines.expect("lines"));
    let token = match (source, remux) {
        ("BluRay", true) => "BluRay.REMUX",
        (s, false) => s,
        (s, true) => panic!("no remux token for {s}"),
    };
    let title = format!("Parity.Size.Probe.2020.{lines}p.{token}.x264-PARITY");
    let parsed = parse_release_metadata(&title);
    assert_eq!(parsed.source.as_ref().map(|s| s.as_str()), Some(source), "probe source");
    assert_eq!(parsed.is_remux, remux, "probe remux");
    assert_eq!(parsed.quality.as_deref().and_then(super::label_lines), Some(lines), "probe lines");
    assert!(parsed.video_codec.is_some(), "probe needs a codec");
    title
}

#[test]
fn rad_quality_size_limits() {
    let Some(mut suite) = Suite::open("scryer-application::parity_quality::rad_quality_size_limits") else { return };
    let defs: Vec<Value> = corpus("corpus/captured/radarr/config/qualitydefinition.json");
    for (area, entry) in &Catalogue::load().entries {
        if area != "quality" || !entry.id.starts_with("RAD-QUALITY-5") {
            continue;
        }
        let Some((path, fragment)) = entry.reference.split_once('#') else { continue };
        if !path.ends_with("radarr/config/qualitydefinition.json") {
            continue;
        }
        let Some(n) = super::fragment_number(fragment, "id") else { continue };
        if !suite.wants(&entry.id) {
            continue;
        }
        let def = super::by_id(&defs, "id", n, "quality definition");
        let title = probe_title(def["quality"]["name"].as_str().unwrap());
        let per_min = |key: &str| def[key].as_f64();
        let bytes = |mb: f64| (mb * MB).round() as i64 * RUNTIME_MIN;
        let min = bytes(per_min("minSize").unwrap_or(0.0));
        let max = per_min("maxSize").filter(|m| *m > 0.0).map(bytes);
        let preferred = per_min("preferredSize").map(bytes);
        let arr_accepts = |s: i64| s >= min && max.is_none_or(|m| s <= m);
        // DownloadDecisionComparer.CompareSize: higher key ranks first.
        let arr_key = |s: i64| match preferred {
            Some(p) => -((s - p) / BIN).abs(),
            None => s / BIN,
        };

        let mut sizes = vec![min.max(1), min - 1, max.map_or(FAR_BYTES, |m| m), max.map_or(0, |m| m + 1)];
        sizes.extend(preferred);
        sizes.extend(INTERIOR_GIB.iter().map(|g| g * GIB).filter(|s| arr_accepts(*s)));
        sizes.retain(|s| *s > 0);
        sizes.sort();
        sizes.dedup();

        let mut failures = Vec::new();
        let mut both = Vec::new();
        for size in sizes {
            let arr = arr_accepts(size);
            let (ok, penalty) = scryer(&title, size);
            if ok != arr {
                failures.push(format!("{size} B ({:.1} MiB/min): arr {}, scryer {}", rate(size), verb(arr), verb(ok)));
            } else if ok {
                both.push((size, penalty));
            }
        }
        let mut inversions = Vec::new();
        for &(a, pa) in &both {
            for &(b, pb) in &both {
                if arr_key(a) > arr_key(b) && pa > pb {
                    inversions.push(format!("{:.1} over {:.1} MiB/min, scryer penalty {pa} > {pb}", rate(a), rate(b)));
                }
            }
        }
        if !inversions.is_empty() {
            let rule = match preferred {
                Some(_) => "closest-to-preferred not kept (no Scryer setting for a per-quality preferred size, preferredSize MB/min; Scryer's size fit uses its own expected bitrate)",
                None => "largest-first not kept",
            };
            failures.push(format!("{rule}: arr ranks {}", inversions.join(", ")));
        }
        suite.record(&entry.id, failures.is_empty(), failures.join("; "));
    }
    suite.finish();
}

fn rate(size: i64) -> f64 {
    size as f64 / MB / RUNTIME_MIN as f64
}

fn verb(accept: bool) -> &'static str {
    if accept { "accept" } else { "reject" }
}
