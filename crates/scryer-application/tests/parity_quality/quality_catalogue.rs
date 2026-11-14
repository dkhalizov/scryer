//! Stream `quality_catalogue`: the arr default quality order (Quality.cs
//! `DefaultQualityDefinitions` weights) against Scryer's tier ordering.
//!
//! Suites: `son_quality_default_order` (SON-QUALITY-0120..0141, 22 cases) and
//! `rad_quality_default_order` (RAD-QUALITY-0135..0164, 30 cases), cases with
//! `input.op == "default_definition"` in `corpus/reference/<app>/quality/ordering.json`.
//!
//! Scryer function under test: `parity_seams::resolution_rank(&str)` (the
//! cutoff election's comparator, acquisition/decision_helpers.rs:196; lower
//! is better, a label with no `<n>p`/`<n>i` token ranks last).
//!
//! Method: for case Q and every other default_definition case Q' in the file,
//! expected = weight(Q).cmp(weight(Q')), actual = rank(Q').cmp(rank(Q))
//! (operands swapped because a lower rank is better). Pass iff all pairs agree.
//!
//! # Translation table
//!
//! - Arr weight -> the reference's default profile order:
//!   `GetDefaultProfile` groups `DefaultQualityDefinitions` by `Weight`
//!   (QualityProfileService.cs:190 Sonarr, :215 Radarr), and the profile index
//!   is the comparator. Equal weight = one group = tie. Used only on the
//!   expected side.
//! - Arr quality name with a `<n>p` token (`Bluray-1080p`, `Remux-2160p`) ->
//!   passed unchanged to `resolution_rank`, which reads compound labels.
//! - Arr quality name with no token (`SDTV`, `DVD`, `DVD-R`, `REGIONAL`,
//!   `Raw-HD`, `BR-DISK`) -> `"<lines>p"` from the root `QUALITIES` lines
//!   (480 or 1080). Scryer never stores such a name: a file's label is the
//!   parser's resolution token (scryer-release-parser parse.rs:4434/4466) or
//!   the dimension tier (`derive_primary_quality_label`, types.rs:4212).
//!   Names whose lines are `None` (`Unknown`, `CAM`, `TELESYNC`, `TELECINE`,
//!   `WORKPRINT`, `DVDSCR`) stay raw and rank last, as an unresolved label
//!   does in Scryer.
//! - Not modelled: the dimension path spells 2160p as `"4K"`, which
//!   `resolution_lines` does not read, so a dimension-labelled 2160p file
//!   ranks last in the cutoff election. Here 2160p names keep their token.
//!
//! # Verdicts
//!
//! Green: SON-QUALITY-0120 (Unknown), 0126 and RAD-QUALITY-0148 (Bluray-576p).
//! Red: every other case. Sources tie within a resolution (HDTV/WEB/Bluray/
//! Remux, P11/D48), and the no-resolution names (Unknown, CAM, TELESYNC,
//! TELECINE, WORKPRINT, DVDSCR) tie with each other and rank below 480p.
//!
//! # Ignored keys
//!
//! `expect.min_size`, `max_size`, `preferred_size` (Scryer has no per-quality
//! size table), `expect.group_name` (no quality groups), `note`.
//!
//! # Unmapped (tests: []), with reasons
//!
//! - `quality_declaration` (SON 0097-0118, RAD 0104-0133): numeric id, source
//!   enum and resolution int of each named quality. Representation: Scryer has
//!   no named qualities and stores a resolution label; the only reader of a
//!   name returns the digits already in the input, so a green would echo it.
//! - `all_qualities` (SON 0119, RAD 0134): declaration order of the closed
//!   list, not a rank. Scryer has no quality list.
//! - `from_id` / `to_id` (SON 0050-0077, RAD 0059-0084): numeric id casts;
//!   Scryer has no id space.
//! - `definitions_init` (SON 0038-0041, RAD 0041-0044): startup reconcile of a
//!   stored quality-definition table, asserted by mock call counts. Scryer has
//!   no such table or store.
//! - `find_by_source_and_resolution` (SON 0042-0049, RAD 0045-0058): snaps a
//!   (source, resolution) pair onto the closed set. No Scryer function maps a
//!   source plus resolution to anything; a test would echo the harness source
//!   translation. Concept absent.

use scryer_application::parity_seams::resolution_rank;
use scryer_parity::{Suite, corpus};
use serde_json::Value;

/// Scryer's label for an arr quality name, ranked (see translation table).
fn rank(name: &str) -> u32 {
    let raw = resolution_rank(name);
    if raw != u32::MAX {
        return raw;
    }
    match super::QUALITIES
        .iter()
        .find(|q| q.0 == name)
        .and_then(|q| q.3)
    {
        Some(lines) => resolution_rank(&format!("{lines}p")),
        None => raw,
    }
}

fn default_order(suite_name: &str, app: &str) {
    let Some(mut suite) = Suite::open(suite_name) else {
        return;
    };
    let cases: Vec<Value> = corpus(&format!("corpus/reference/{app}/quality/ordering.json"));
    let defs: Vec<(&str, &str, i64)> = cases
        .iter()
        .filter(|c| c["input"]["op"] == "default_definition")
        .map(|c| {
            (
                c["id"].as_str().unwrap(),
                c["input"]["given"]["quality"].as_str().unwrap(),
                c["expect"]["weight"].as_i64().unwrap(),
            )
        })
        .collect();
    for &(id, name, weight) in &defs {
        if !suite.wants(id) {
            continue;
        }
        let wrong: Vec<String> = defs
            .iter()
            .filter(|d| d.0 != id)
            .filter_map(|&(_, other, w2)| {
                let expected = weight.cmp(&w2);
                let actual = rank(other).cmp(&rank(name));
                (expected != actual)
                    .then(|| format!("vs {other}: reference {expected:?}, Scryer {actual:?}"))
            })
            .collect();
        suite.record(
            id,
            wrong.is_empty(),
            &format!(
                "{name}: {} of {} pairs disagree: {}",
                wrong.len(),
                defs.len() - 1,
                wrong.join("; ")
            ),
        );
    }
    suite.finish();
}

#[test]
fn son_quality_default_order() {
    default_order(
        "scryer-application::parity_quality::son_quality_default_order",
        "sonarr",
    );
}

#[test]
fn rad_quality_default_order() {
    default_order(
        "scryer-application::parity_quality::rad_quality_default_order",
        "radarr",
    );
}
