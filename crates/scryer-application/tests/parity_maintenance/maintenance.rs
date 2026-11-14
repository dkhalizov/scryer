//! Parity net, area `maintenance`, stream "maintenance" (stages 6-7).
//!
//! Suites (one #[test] each; helpers of the root file are used via `super::`):
//! - `mnt_comparator`: comparator.json `doRuleAction`, 0297, 0299..0312,
//!   0334..0336. Scryer: `MaintenanceRulesEngine::build` + `evaluate`
//!   (through `super::evaluate`) on an input from the production
//!   `build_title_input`.
//! - `mnt_playback_hold`: collection_worker.json 0160, 0174.
//!   Scryer: `maintenance_rules::fold_playback_hold` (safety.rs:104) and
//!   `MaintenancePlaybackHold::holds_destructive_work`.
//!
//! Translation, Maintainerr -> Scryer. `mnt_comparator` reuses the root
//! header's operator table unchanged (EQUALS is plain Rego `==`); additionally:
//!
//! | Maintainerr | Scryer |
//! |---|---|
//! | `val1` (getter value) | a carrier fact of its JSON type (`super::place`) |
//! | `val2` (Maintainerr's `lastVal`) | a second getter value in the next carrier of its type (never a Rego literal: a literal cannot be undefined) |
//! | `val2` = `{"$undefined": true}` | `Observation::unknown("parity_undefined")` with val1's type (doRuleAction has no declared rule type); the engine holds the rule: outcome `Unknown` |
//! | `{"$date": d}` | RFC3339 string through `time.parse_rfc3339_ns` |
//! | a text list with `$undefined` entries | the entries are dropped (Rego has no undefined) |
//! | `expect.result` true / false | outcome `Match` / not `Match` (`NoMatch` or `Unknown`) |
//! | given.mediaServer "unreachable" | one Plex connection, `PlaybackProbeStatus::Unreachable` |
//! | given.activeSessions (non-empty) | one Plex connection, `ActiveSessions(len)`; Scryer's probe counts sessions and has no item identity |
//! | neither key | `Idle` |
//! | per-member verdict | one hold for the whole run (action_execution safety check holds every due candidate on anything but `Clear`; safety.rs:15-26) |
//! | expect.collectionsRead | `!hold.holds_destructive_work()` (nothing passes the playback gate), computed from the hold, not from the member list |
//! | expect.handled | `[]` if the hold stops the run (nothing is handled); if the gate proceeds there is no Scryer value (the gate takes no subject), so the case is recorded red, never an echo of `given.media` |
//! | expect.deferredLog | `matches!(hold, Hold { .. })` |
//! | expect.events.CollectionHandler_Failed (count) | 1 if `matches!(hold, Unknown { .. })` else 0: the outage is reported as an outage (candidates held with PLAYBACK_UNKNOWN, action_execution.rs:2453-2461), not folded into "someone is playing" |
//!
//! Compared expect keys: `result` (comparator); `collectionsRead`, `handled`,
//! `deferredLog`, `events.CollectionHandler_Failed` (playback). Ignored expect
//! key: `events.CollectionHandler_Finished` (Maintainerr's end-of-run event;
//! `fold_playback_hold` produces no run report). Ignored input keys: `op`
//! (selection only), `collections` (playback holds every action kind).
//!
//! Strength of the comparator greens: they run the production engine over
//! production facts, but most cannot tell this table from Maintainerr's.
//! 0300 and 0335 (undefined `val2`) compare only Match vs not Match, so they
//! do not tell the fail-closed hold (`Unknown`) from `NoMatch`: an unknown
//! fact is also missing from `input.facts`, so the body would be undefined
//! even without the host's unknown tracking. 0305 (false EQUALS 1) is a Rego
//! cross-type comparison, false for any bool/number pair.
//!
//! Expected reds: 0308 (no Scryer fact holds a list of numbers: CONTAINS
//! exists, the value type does not, D29), 0174 (the playback hold is global
//! by design, safety.rs:15-26, so the idle member is held too; waiver
//! candidate). 0174 cannot turn green from a Scryer change alone:
//! `fold_playback_hold` takes no subject and `handled` is `[]` when held and
//! unmapped (red) when the gate proceeds, so per-subject session mapping would
//! need a harness change with it. Because `handled` already fails, its
//! `deferredLog` part is not ratchet-protected.
//!
//! Unmapped (tests: []), with reasons:
//! - 0294, 0295, 0296 (case-insensitive EQUALS, a one-entry list EQUALS its
//!   value, rule.comparator.service.ts:621-673): Scryer has no comparator; a
//!   rule is author-written Rego and D36 puts Maintainerr's matching in the
//!   ported rule's source. A status here would measure this harness's EQUALS
//!   translation (`==` vs `lower(L) == lower(R)`), and no Scryer change could
//!   flip it. Open port decision, see 5001..5028 in the root header.
//! - 0298 (true EQUALS 1): same reason; also, 1/0 is how Maintainerr encodes
//!   every BOOL custom value (`+rule.customVal.value`,
//!   rule.comparator.service.ts:422-424, spec :55), so this is not a D35
//!   coercion quirk. A Scryer rule compares with the literal true/false.
//! - 0001, 0032, 0280: TMDB/TVDB id resolution before the *arr / Seerr lookup
//!   (D33); Scryer acts on its own ids and request links.
//! - 0004, 0045: transient lookup failure fails closed; Scryer's counterpart
//!   (title read error holds with UNKNOWN_AT_EXECUTION) is inside async
//!   `AppUseCase::maintenance_execution_safety_checks` (action_execution.rs:1747)
//!   over repositories, no public doubles.
//! - 0005, 0042: delete of an untracked item through the media server (D32:
//!   Scryer never does, fails closed; waiver candidates).
//! - 0006, 0058..0064: mock-interaction assertions on Radarr/Sonarr API calls;
//!   Scryer DELETE is async `execute_scoped_maintenance_deletion`
//!   (scoped_deletion.rs:758) over repositories.
//! - 0094..0097, 0100..0102, 0116..0120, 0126..0129: Sonarr helper (API
//!   client); counterparts are async AppUseCase (scoped_deletion.rs:548-620,
//!   :647-688); 0127..0129 (path tail vs Sonarr relativePath) have no concept.
//! - 0131, 0132, 0136..0138, 0140..0144, 0150, 0155, 0157, 0158: collection
//!   handler (media-server collections, rescans, request cleanup, handled
//!   bytes); async `execute_one_maintenance_candidate` (action_execution.rs:1305),
//!   no pure sub-decision.
//! - 0162, 0165, 0173, 0175..0177: collection worker loop; due_at and
//!   hold-on-unknown are inline in async `reconcile_maintenance_title`
//!   (evaluation.rs:1827-1900); 0173's manual membership and the events/Seerr
//!   sync of 0175..0177 have no counterpart.
//! - 0281: scheduler bootstrap. Job scheduling is not ported (D33); Scryer has
//!   no per-rule cron, maintenance evaluation is one job, and no pure function
//!   maps rule groups to jobs.
//! - 0189, 0191, 0245, 0256, 0273, 0274: see the root header.
//! - 0358: see the root header; its only expected value is the comparator's
//!   resolved-operand trace (D36: traces are not behaviour).

use scryer_application::maintenance_rules::{MaintenancePlaybackHold, fold_playback_hold};
use scryer_application::{ConnectionPlaybackActivity, PlaybackActivitySnapshot, PlaybackProbeStatus};
use scryer_domain::MediaServerProvider;
use scryer_parity::{Suite, corpus};
use scryer_rules::maintenance::MaintenanceOutcome;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

use super::{Kind, Operand};

const DIR: &str = "corpus/reference/maintainerr/maintenance";

// 0294, 0295, 0296, 0298 are left out on purpose (header, "Unmapped").
const COMPARATOR_CASES: &[&str] = &[
    "MNT-MAINTENANCE-0297", "MNT-MAINTENANCE-0299",
    "MNT-MAINTENANCE-0300", "MNT-MAINTENANCE-0301", "MNT-MAINTENANCE-0302",
    "MNT-MAINTENANCE-0303", "MNT-MAINTENANCE-0304", "MNT-MAINTENANCE-0305",
    "MNT-MAINTENANCE-0306", "MNT-MAINTENANCE-0307", "MNT-MAINTENANCE-0308",
    "MNT-MAINTENANCE-0309", "MNT-MAINTENANCE-0310", "MNT-MAINTENANCE-0311",
    "MNT-MAINTENANCE-0312", "MNT-MAINTENANCE-0334", "MNT-MAINTENANCE-0335",
    "MNT-MAINTENANCE-0336",
];
const PLAYBACK_CASES: &[&str] = &["MNT-MAINTENANCE-0160", "MNT-MAINTENANCE-0174"];

fn comparator_outcome(given: &Value) -> Result<Result<MaintenanceOutcome, String>, String> {
    let mut input = super::plain_input();
    let mut used = HashMap::new();
    let first = super::operand(&given["val1"]);
    let Operand::Known(v) = &first else {
        return Err("val1 is not a known value".into());
    };
    let first_kind: Kind = super::value_kind(v)?;
    let second = super::operand(&given["val2"]);
    let second_kind = match &second {
        Operand::Known(v) => super::value_kind(v)?,
        _ => first_kind,
    };
    let l = super::place(&mut input.facts, &mut used, first_kind, &first)?;
    let r = super::place(&mut input.facts, &mut used, second_kind, &second)?;
    let body = super::condition(given["action"].as_str().unwrap(), &l, &r)?;
    super::evaluate(&[body], &input)
}

#[test]
fn mnt_comparator() {
    let Some(mut suite) = Suite::open("scryer-application::parity_maintenance::mnt_comparator")
    else {
        return;
    };
    let cases: Vec<Value> = corpus(&format!("{DIR}/comparator.json"));
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if case["input"]["op"] != "doRuleAction"
            || !COMPARATOR_CASES.contains(&id)
            || !suite.wants(id)
        {
            continue;
        }
        match comparator_outcome(&case["input"]["given"]) {
            Err(e) | Ok(Err(e)) => suite.record(id, false, e),
            Ok(Ok(outcome)) => suite.check(
                id,
                case["expect"]["result"].clone(),
                json!(outcome == MaintenanceOutcome::Match),
            ),
        }
    }
    suite.finish();
}

#[test]
fn mnt_playback_hold() {
    let Some(mut suite) = Suite::open("scryer-application::parity_maintenance::mnt_playback_hold")
    else {
        return;
    };
    let cases: Vec<Value> = corpus(&format!("{DIR}/collection_worker.json"));
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if case["input"]["op"] != "collection_worker_execute"
            || !PLAYBACK_CASES.contains(&id)
            || !suite.wants(id)
        {
            continue;
        }
        let given = &case["input"]["given"];
        let status = if given["mediaServer"] == "unreachable" {
            PlaybackProbeStatus::Unreachable("parity_unreachable".into())
        } else {
            match given["activeSessions"].as_array() {
                Some(s) if !s.is_empty() => PlaybackProbeStatus::ActiveSessions(s.len() as u32),
                _ => PlaybackProbeStatus::Idle,
            }
        };
        let snapshot = PlaybackActivitySnapshot {
            connections: vec![ConnectionPlaybackActivity {
                connection_id: super::CONNECTION.into(),
                provider: MediaServerProvider::Plex,
                status,
            }],
            observed_at: super::evaluation_time(),
        };
        let hold = fold_playback_hold(&snapshot);
        let proceeds = !hold.holds_destructive_work();
        let mut expected = Map::new();
        let mut actual = Map::new();
        let mut missing = Vec::new();
        for (key, value) in case["expect"].as_object().unwrap() {
            let a = match key.as_str() {
                "collectionsRead" => json!(proceeds),
                // No echo of given.media: the gate names no members, so a
                // proceeding gate has no `handled` value to compare.
                "handled" if !proceeds => json!([]),
                "deferredLog" => json!(matches!(hold, MaintenancePlaybackHold::Hold { .. })),
                "events" => {
                    // Only the Failed count has a counterpart (header).
                    let key = "events.CollectionHandler_Failed".to_string();
                    expected.insert(key.clone(), value["CollectionHandler_Failed"].clone());
                    let failed = matches!(hold, MaintenancePlaybackHold::Unknown { .. });
                    actual.insert(key, json!(u32::from(failed)));
                    continue;
                }
                _ => {
                    missing.push(key.clone());
                    continue;
                }
            };
            expected.insert(key.clone(), value.clone());
            actual.insert(key.clone(), a);
        }
        if missing.is_empty() {
            suite.check(id, Value::Object(expected), Value::Object(actual));
        } else {
            suite.record(id, false, format!("no Scryer field for {missing:?}"));
        }
    }
    suite.finish();
}
