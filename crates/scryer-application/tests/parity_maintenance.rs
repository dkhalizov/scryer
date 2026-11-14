//! Parity net, area `maintenance` (notes/11-parity-net-design.md, parity/README.md).
//!
//! Scryer under test:
//! - the maintenance rule engine (`scryer_rules::maintenance::MaintenanceRulesEngine`):
//!   Rego matchers over the fact document, with the host's three-valued facts
//!   (a known value; `absent` = the key is missing from `input.facts`; `unknown`
//!   = the rule is held before it runs, outcome `Unknown`);
//! - the fact builder (`maintenance_rules::facts::build_title_input`) for the
//!   watch and request facts.
//!
//! Translation table, Maintainerr -> Scryer:
//!
//! | Maintainerr | Scryer |
//! |---|---|
//! | a getter value | a carrier fact of the same type: NUMBER -> `file_count`, `episode_count`, `episode_file_count`, `monitored_episode_count`; DATE -> `added_at`, `first_imported_at`, `last_upgraded_at`, `lists_left_at`; TEXT -> `quality_profile_id`, `added_by_username`, `request_origin`; text list -> `tags`, `lists_names`, `lists_on_leave`; BOOL -> `monitored`, `has_file`, `lists_left_all` (the n-th value of a type takes the n-th carrier; a carrier's meaning is irrelevant to operator cases). The type is the value's own JSON type, or the rule's declared type when the value is null/undefined/throws |
//! | value `null` | `Observation::absent()` |
//! | value `undefined` | `Observation::unknown(..)` |
//! | getter throws | `Observation::unknown(..)` (Scryer never throws from a fact read; the read fails into unknown) |
//! | `{"$date": d}` | RFC3339 string; compared through `time.parse_rfc3339_ns` |
//! | custom value NUMBER / DATE / TEXT | Rego number / `time.parse_rfc3339_ns("..")` / string literal |
//! | EQUALS, NOT_EQUALS, BIGGER, SMALLER, BEFORE, AFTER | `==`, `!=`, `>`, `<`, `<`, `>` (plain Rego: none of Maintainerr's string lowercasing, set-based list EQUALS or day-granular date EQUALS, `rule.comparator.service.ts:621-669`; `mnt_maintenance` compares only numbers and dates, `mnt_comparator` in `parity_maintenance/maintenance.rs` also texts, lists and bools, and leaves out the cases where only lowercasing or set EQUALS decides, D36) |
//! | CONTAINS on a text | `indexof(L, R) != -1` |
//! | CONTAINS on a list | `R in L`, or `some v in R; v in L` when R is a list (`undefined` entries of a literal list dropped: Rego has no undefined value) |
//! | a later rule of the same section with operator "0" (AND) | another statement of the same `match if { .. }` body |
//! | a later rule of the same section with operator null or "1" (OR, `rule.comparator.service.ts:228, :332`; only the first rule of the group and of each section is forced to null, :118-120, :149) | no translation: Maintainerr folds left over the section, a Rego body is one conjunction (the case fails) |
//! | the first rule of a later section, operator null or "1" (OR, :145-147) | another `match if` body |
//! | the first rule of a later section, operator "0" (AND) | no translation (the case fails) |
//! | item matched / not / transient failure | outcome `Match` / `NoMatch` / `Unknown` |
//! | evaluation throws | the evaluator returns an error or a rule error |
//! | plex `lastViewedAt` / `seenBy` | `last_watched_at` / `watched_by_user_ids` |
//! | plex `isWatched` (the server account's watched state) | `watched_by_anyone` (Scryer has no per-server-account flag; it is the nearest fact) |
//! | plex watch state `{isWatched: false, viewCount: 0}` | no play rows, history readable |
//! | seerr `isRequested` (1/0) / `addUser` | `requested` (true/false) / `requested_by_usernames` |
//! | a Plex history row | one played `UserMediaSignal` on an observed connection; its account resolves to the Scryer user named like the Plex user (unknown account -> no Scryer user); a row with no account is its own linked user |
//! | Plex history unreadable | watch gate `WatchSignalFreshness::Unavailable` |
//! | a Seerr request by user N | requester user id "N" (`None` when there are no requests) |
//!
//! Expect keys compared: `result`, `matched`, `matchedCount`,
//! `transientFailure`, `throws` (compared as "an error happened", the message
//! is Maintainerr's own), `value`. Keys Scryer has no field for fail the case:
//! `ruleResult`, `sectionResult` (Maintainerr's per-rule comparison trace).
//!
//! Suite `mnt_maintenance`, mapped IDs:
//! - rule_group.json: 0352..0355, 0357, 0363..0366, 0369..0372.
//! - plex_getter.json: 0179, 0192, 0242, 0243, 0244. seerr_getter.json: 0255,
//!   0262, 0263.
//!
//! Suites of `parity_maintenance/maintenance.rs` (its header has their IDs,
//! tables and unmapped reasons): `mnt_comparator` (comparator.json
//! `doRuleAction` through this file's operator table and the production
//! engine), `mnt_playback_hold` (collection_worker.json 0160, 0174).
//!
//! Unmapped (tests: []), with reasons:
//! - comparator.json 0294, 0295, 0296, 0298: see `parity_maintenance/maintenance.rs`
//!   (only Maintainerr's lowercasing, set EQUALS or BOOL encoding decides
//!   them; that belongs to the open Rego port decision, 5001..5028 below).
//! - 0358: a NUMBER custom value on a DATE property means "now minus n
//!   seconds"; Scryer rules have no such value type (the author writes the
//!   arithmetic against `input.evaluation_time`).
//! - 0189, 0191 (item-level lastViewedAt merged with history), 0245 (user
//!   list unreadable): resolved in the media-server signal sync and the run's
//!   roster read, not in the fact builder.
//! - 0256 (each requester once): deduplication happens in the batched
//!   requester read before the builder; 0273, 0274 (Seerr media-added date):
//!   Scryer's `added_at` is the title row's creation, no counterpart;
//!   0280 (no TMDB id): Scryer's request facts come from request links.
//! - 0001..0177 (Radarr/Sonarr actions, collection handler and worker),
//!   0281 (scheduler): async action execution and jobs over repositories and
//!   *arr clients; the only doubles are crate-private (`lib_tests`). Per-group
//!   reasons for the unmapped ones are in `parity_maintenance/maintenance.rs`
//!   (0160 and 0174 are mapped there).
//! - MNT-MAINTENANCE-5001..5028 (`mnt_maintenance_config`, no suite fn): our
//!   Maintainerr rule groups, collections and schedules; a check needs a
//!   decision on how our rules are ported to Scryer Rego (open decision), the
//!   config gives no expected answer for a Scryer function.
//! - MNT-MAINTENANCE-6001..6011 (`mnt_maintenance_captured`, no suite fn):
//!   log-pattern observations with no facts to evaluate.
//! - PRW-MAINTENANCE-5001, 5002: Prowlarr history cleanup, no Scryer function.

#[path = "parity_maintenance/maintenance.rs"]
mod maintenance;

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use scryer_application::maintenance_rules::facts::{
    MaintenanceLibraryRef, MaintenanceTitleClaims, MaintenanceTitlePeople, MaintenanceTitleWatch,
    MaintenanceWatchContext, WatchSignalFreshness, build_title_input,
};
use scryer_domain::{MediaServerProvider, MediaServerSignalKind, Title, UserMediaSignal};
use scryer_parity::{Suite, corpus};
use scryer_rules::maintenance::{
    MaintenanceFactsDoc, MaintenanceInput, MaintenanceOutcome, MaintenancePolicy,
    MaintenanceRulesEngine, Observation, rewrite_package_declaration,
};
use serde_json::{Map, Value, json};

const SUITE: &str = "scryer-application::parity_maintenance::mnt_maintenance";
const CONNECTION: &str = "parity-plex";

const RULE_GROUP_CASES: &[&str] = &[
    "MNT-MAINTENANCE-0352",
    "MNT-MAINTENANCE-0353",
    "MNT-MAINTENANCE-0354",
    "MNT-MAINTENANCE-0355",
    "MNT-MAINTENANCE-0357",
    "MNT-MAINTENANCE-0363",
    "MNT-MAINTENANCE-0364",
    "MNT-MAINTENANCE-0365",
    "MNT-MAINTENANCE-0366",
    "MNT-MAINTENANCE-0369",
    "MNT-MAINTENANCE-0370",
    "MNT-MAINTENANCE-0371",
    "MNT-MAINTENANCE-0372",
];
const GETTER_CASES: &[&str] = &[
    "MNT-MAINTENANCE-0179",
    "MNT-MAINTENANCE-0192",
    "MNT-MAINTENANCE-0242",
    "MNT-MAINTENANCE-0243",
    "MNT-MAINTENANCE-0244",
    "MNT-MAINTENANCE-0255",
    "MNT-MAINTENANCE-0262",
    "MNT-MAINTENANCE-0263",
];

// ── Fixed inputs ────────────────────────────────────────────────────────────

fn evaluation_time() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(1_767_225_600, 0).unwrap() // 2026-01-01T00:00:00Z
}

fn title() -> Title {
    serde_json::from_value(json!({
        "id": "parity-title",
        "library_id": "parity-movies",
        "name": "Parity Movie",
        "facet": "movie",
        "monitored": true,
        "tags": [],
        "external_ids": [],
        "root_folder_id": "parity-root",
        "created_at": "2025-01-01T00:00:00Z",
        "catalog_sort_key": "parity movie",
        "canonical_tags": [],
        "aliases": [],
        "tagged_aliases": []
    }))
    .unwrap()
}

fn input_for(
    requesters: Option<&[String]>,
    usernames: &HashMap<String, String>,
    context: &MaintenanceWatchContext,
    signals: Option<&[UserMediaSignal]>,
) -> MaintenanceInput {
    let title = title();
    let library = MaintenanceLibraryRef {
        id: title.library_id.clone(),
        name: "Movies".into(),
    };
    build_title_input(
        evaluation_time(),
        &title,
        &library,
        &[],
        MaintenanceTitlePeople {
            requester_user_ids: requesters,
            usernames,
        },
        MaintenanceTitleWatch {
            context,
            signals,
            audience: None,
        },
        MaintenanceTitleClaims::none(),
        &[],
    )
}

fn plain_input() -> MaintenanceInput {
    input_for(
        None,
        &HashMap::new(),
        &MaintenanceWatchContext::default(),
        None,
    )
}

fn rfc3339(s: &str) -> Option<String> {
    if let Ok(t) = DateTime::parse_from_rfc3339(s) {
        return Some(t.with_timezone(&Utc).to_rfc3339());
    }
    let day = NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
    Some(
        Utc.from_utc_datetime(&day.and_hms_opt(0, 0, 0)?)
            .to_rfc3339(),
    )
}

fn date_of(v: &Value) -> Option<String> {
    v.get("$date").and_then(Value::as_str).and_then(rfc3339)
}

// ── Operands and carriers ───────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    Int,
    Date,
    Str,
    StrList,
    Bool,
}

enum Operand {
    Known(Value),
    Null,
    Undefined,
    Throws,
}

fn is_undefined(v: &Value) -> bool {
    v.get("$undefined").is_some()
}

fn operand(v: &Value) -> Operand {
    if v.is_null() {
        Operand::Null
    } else if is_undefined(v) {
        Operand::Undefined
    } else if v.get("$throws").is_some() {
        Operand::Throws
    } else {
        Operand::Known(v.clone())
    }
}

fn value_kind(v: &Value) -> Result<Kind, String> {
    match v {
        Value::Number(n) if n.is_i64() => Ok(Kind::Int),
        Value::String(_) => Ok(Kind::Str),
        Value::Bool(_) => Ok(Kind::Bool),
        Value::Array(items) if items.iter().all(|i| i.is_string() || is_undefined(i)) => {
            Ok(Kind::StrList)
        }
        Value::Object(o) if o.contains_key("$date") => Ok(Kind::Date),
        other => Err(format!("no Scryer fact carries the value {other}")),
    }
}

fn declared_kind(declared: &str) -> Result<Kind, String> {
    match declared {
        "NUMBER" => Ok(Kind::Int),
        "DATE" => Ok(Kind::Date),
        "TEXT" => Ok(Kind::Str),
        "TEXT_LIST" => Ok(Kind::StrList),
        "BOOL" | "BOOLEAN" => Ok(Kind::Bool),
        other => Err(format!("no carrier for declared type {other}")),
    }
}

fn operand_kind(op: &Operand, declared: &str) -> Result<Kind, String> {
    match op {
        Operand::Known(v) => value_kind(v),
        _ => declared_kind(declared),
    }
}

fn carriers(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Int => &[
            "file_count",
            "episode_count",
            "episode_file_count",
            "monitored_episode_count",
        ],
        Kind::Date => &[
            "added_at",
            "first_imported_at",
            "last_upgraded_at",
            "lists_left_at",
        ],
        Kind::Str => &["quality_profile_id", "added_by_username", "request_origin"],
        Kind::StrList => &["tags", "lists_names", "lists_on_leave"],
        Kind::Bool => &["monitored", "has_file", "lists_left_all"],
    }
}

fn obs<T: serde::Serialize>(
    op: &Operand,
    convert: impl FnOnce(&Value) -> Option<T>,
) -> Result<Observation<T>, String> {
    Ok(match op {
        Operand::Known(v) => Observation::known(
            convert(v).ok_or_else(|| format!("{v} does not fit its carrier fact"))?,
        ),
        Operand::Null => Observation::absent(),
        Operand::Undefined => Observation::unknown("parity_undefined"),
        Operand::Throws => Observation::unknown("parity_getter_threw"),
    })
}

fn string(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

fn string_list(v: &Value) -> Option<Vec<String>> {
    v.as_array()
        .map(|items| items.iter().filter_map(string).collect())
}

fn set_fact(f: &mut MaintenanceFactsDoc, name: &str, op: &Operand) -> Result<(), String> {
    match name {
        "file_count" => f.file_count = obs(op, Value::as_i64)?,
        "episode_count" => f.episode_count = obs(op, Value::as_i64)?,
        "episode_file_count" => f.episode_file_count = obs(op, Value::as_i64)?,
        "monitored_episode_count" => f.monitored_episode_count = obs(op, Value::as_i64)?,
        "added_at" => f.added_at = obs(op, date_of)?,
        "first_imported_at" => f.first_imported_at = obs(op, date_of)?,
        "last_upgraded_at" => f.last_upgraded_at = obs(op, date_of)?,
        "lists_left_at" => f.lists_left_at = obs(op, date_of)?,
        "quality_profile_id" => f.quality_profile_id = obs(op, string)?,
        "added_by_username" => f.added_by_username = obs(op, string)?,
        "request_origin" => f.request_origin = obs(op, string)?,
        "tags" => f.tags = obs(op, string_list)?,
        "lists_names" => f.lists_names = obs(op, string_list)?,
        "lists_on_leave" => f.lists_on_leave = obs(op, string_list)?,
        "monitored" => f.monitored = obs(op, Value::as_bool)?,
        "has_file" => f.has_file = obs(op, Value::as_bool)?,
        "lists_left_all" => f.lists_left_all = obs(op, Value::as_bool)?,
        other => return Err(format!("unknown carrier {other}")),
    }
    Ok(())
}

/// One side of a comparison as Rego, with its type.
struct Side {
    expr: String,
    kind: Kind,
}

/// Puts a getter value into the next free carrier of its type.
fn place(
    facts: &mut MaintenanceFactsDoc,
    used: &mut HashMap<Kind, usize>,
    kind: Kind,
    op: &Operand,
) -> Result<Side, String> {
    let n = used.entry(kind).or_insert(0);
    let name = *carriers(kind).get(*n).ok_or("out of carrier facts")?;
    *n += 1;
    set_fact(facts, name, op)?;
    let fact = format!("input.facts.{name}");
    let expr = if kind == Kind::Date {
        format!("time.parse_rfc3339_ns({fact})")
    } else {
        fact
    };
    Ok(Side { expr, kind })
}

/// A known value written into the rule source.
fn literal(v: &Value) -> Result<Side, String> {
    let kind = value_kind(v)?;
    let expr = match kind {
        Kind::Date => format!(
            "time.parse_rfc3339_ns({})",
            Value::String(date_of(v).ok_or("bad $date")?)
        ),
        Kind::StrList => {
            let items: Vec<&Value> = v
                .as_array()
                .unwrap()
                .iter()
                .filter(|i| !is_undefined(i))
                .collect();
            serde_json::to_string(&items).unwrap()
        }
        _ => v.to_string(),
    };
    Ok(Side { expr, kind })
}

fn condition(action: &str, l: &Side, r: &Side) -> Result<Vec<String>, String> {
    let op = match action {
        "EQUALS" => "==",
        "NOT_EQUALS" => "!=",
        "BIGGER" | "AFTER" => ">",
        "SMALLER" | "BEFORE" => "<",
        "CONTAINS" => {
            return match (l.kind, r.kind) {
                (Kind::Str, _) => Ok(vec![format!("indexof({}, {}) != -1", l.expr, r.expr)]),
                (Kind::StrList, Kind::StrList) => Ok(vec![
                    format!("some v in {}", r.expr),
                    format!("v in {}", l.expr),
                ]),
                (Kind::StrList, _) => Ok(vec![format!("{} in {}", r.expr, l.expr)]),
                _ => Err(format!("no translation for CONTAINS on {:?}", l.kind)),
            };
        }
        other => return Err(format!("no translation for action {other}")),
    };
    Ok(vec![format!("{} {op} {}", l.expr, r.expr)])
}

// ── Engine ──────────────────────────────────────────────────────────────────

/// Outer `Err`: the translation did not load (the case fails as unmapped
/// translation). Inner `Err`: Scryer's evaluation itself failed.
fn evaluate(
    bodies: &[Vec<String>],
    input: &MaintenanceInput,
) -> Result<Result<MaintenanceOutcome, String>, String> {
    let mut source = String::new();
    for body in bodies {
        source.push_str("match if {\n");
        for statement in body {
            source.push_str("  ");
            source.push_str(statement);
            source.push('\n');
        }
        source.push_str("}\n");
    }
    let policy = MaintenancePolicy {
        id: "parity_case".into(),
        name: "parity case".into(),
        rego_source: rewrite_package_declaration(&source, "parity_case"),
    };
    let engine = MaintenanceRulesEngine::build(&[policy])
        .map_err(|e| format!("translated rule does not load: {e}\n{source}"))?;
    let mut evaluator = engine.evaluator();
    Ok(match evaluator.evaluate(input) {
        Err(e) => Err(e.to_string()),
        Ok(result) => match (result.errors.first(), result.records.first()) {
            (Some(error), _) => Err(error.message.clone()),
            (None, Some(record)) => Ok(record.decision.outcome),
            (None, None) => Err("no decision".into()),
        },
    })
}

// ── Cases ───────────────────────────────────────────────────────────────────

fn is_and(operator: &Value) -> bool {
    *operator == "0" || *operator == 0
}

fn is_or(operator: &Value) -> bool {
    operator.is_null() || *operator == "1" || *operator == 1
}

/// rule_group.json: one item's outcome under the translated rule group.
fn rule_group_item(
    rules: &[Value],
    values: &[Value],
) -> Result<Result<MaintenanceOutcome, String>, String> {
    let mut input = plain_input();
    let mut used = HashMap::new();
    let mut slots = values.iter();
    let mut next = |declared: &Value| -> Result<(Operand, Kind), String> {
        let value = slots.next().ok_or("fewer getter values than operands")?;
        let op = operand(value);
        let kind = operand_kind(&op, declared.as_str().unwrap_or(""))?;
        Ok((op, kind))
    };
    let mut bodies: Vec<Vec<String>> = Vec::new();
    let mut section = None;
    for rule in rules {
        let (first, first_kind) = next(&rule["firstVal"]["type"])?;
        let left = place(&mut input.facts, &mut used, first_kind, &first)?;
        let right = if rule["lastVal"].is_object() {
            let (last, last_kind) = next(&rule["lastVal"]["type"])?;
            place(&mut input.facts, &mut used, last_kind, &last)?
        } else {
            let custom = &rule["customVal"];
            let raw = custom["value"].as_str().unwrap_or_default();
            match (custom["ruleTypeId"].as_str(), first_kind) {
                (Some("NUMBER"), Kind::Int) => {
                    literal(&json!(raw.parse::<i64>().map_err(|e| e.to_string())?))?
                }
                (Some("DATE"), _) => literal(&json!({ "$date": raw }))?,
                (Some("TEXT"), _) => literal(&json!(raw))?,
                (t, k) => {
                    return Err(format!(
                        "no translation for custom value {t:?} on a {k:?} property"
                    ));
                }
            }
        };
        let statements = condition(rule["action"].as_str().unwrap(), &left, &right)?;
        let this_section = rule["section"].as_u64();
        if section == Some(this_section) {
            if !is_and(&rule["operator"]) {
                return Err(format!(
                    "no translation for in-section operator {}",
                    rule["operator"]
                ));
            }
            bodies.last_mut().unwrap().extend(statements);
        } else {
            if section.is_some() && !is_or(&rule["operator"]) {
                return Err(format!(
                    "no translation for section operator {}",
                    rule["operator"]
                ));
            }
            section = Some(this_section);
            bodies.push(statements);
        }
    }
    evaluate(&bodies, &input)
}

fn rule_group(suite: &mut Suite, id: &str, case: &Value) {
    let given = &case["input"]["given"];
    let rules = given["rules"].as_array().unwrap();
    let mut outcomes: Vec<(String, Result<MaintenanceOutcome, String>)> = Vec::new();
    for media in given["media"].as_array().unwrap() {
        let media = media.as_str().unwrap();
        let values = match given.get("getterValuesByMedia") {
            Some(by_media) => &by_media[media],
            None => &given["getterValues"],
        };
        match rule_group_item(rules, values.as_array().unwrap()) {
            Ok(outcome) => outcomes.push((media.to_string(), outcome)),
            Err(e) => return suite.record(id, false, e),
        }
    }
    let matched: Vec<&str> = outcomes
        .iter()
        .filter(|(_, o)| o == &Ok(MaintenanceOutcome::Match))
        .map(|(m, _)| m.as_str())
        .collect();
    let mut expected = Map::new();
    let mut actual = Map::new();
    let mut missing = Vec::new();
    for (key, value) in case["expect"].as_object().unwrap() {
        let (e, a) = match key.as_str() {
            "matched" => (value.clone(), json!(matched)),
            "matchedCount" => (value.clone(), json!(matched.len())),
            "result" => (value.clone(), json!(!matched.is_empty())),
            "transientFailure" => {
                let by_media: Map<String, Value> = outcomes
                    .iter()
                    .map(|(m, o)| {
                        (
                            m.clone(),
                            Value::Bool(o == &Ok(MaintenanceOutcome::Unknown)),
                        )
                    })
                    .collect();
                (value.clone(), Value::Object(by_media))
            }
            "throws" => (
                Value::Bool(true),
                Value::Bool(outcomes.iter().any(|(_, o)| o.is_err())),
            ),
            _ => {
                missing.push(key.clone());
                continue;
            }
        };
        expected.insert(key.clone(), e);
        actual.insert(key.clone(), a);
    }
    let pass = missing.is_empty() && expected == actual;
    let mut detail = String::new();
    if expected != actual {
        detail.push_str(&format!(
            "expected {expected:?}, got {actual:?}; outcomes {outcomes:?}"
        ));
    }
    if !missing.is_empty() {
        detail.push_str(&format!(" no Scryer field for {missing:?}"));
    }
    suite.record(id, pass, detail);
}

fn signal(index: usize, user: Option<String>, played_at: Option<DateTime<Utc>>) -> UserMediaSignal {
    UserMediaSignal {
        id: format!("parity-signal-{index}"),
        connection_id: CONNECTION.into(),
        provider: MediaServerProvider::Plex,
        external_user_id: format!("parity-account-{index}"),
        scryer_user_id: user,
        provider_item_id: "parity-item".into(),
        kind: MediaServerSignalKind::Movie,
        scryer_title_id: Some("parity-title".into()),
        scryer_episode_id: None,
        played: true,
        play_count: 1,
        last_played_at: played_at,
        is_favorite: false,
        observed_at: evaluation_time(),
        sync_generation: 1,
        created_at: evaluation_time(),
        updated_at: evaluation_time(),
    }
}

/// The fact an observation serializes to, in Maintainerr's value encoding.
fn observed(input: &MaintenanceInput, fact: &str) -> Value {
    let document = serde_json::to_value(input).unwrap();
    let envelope = &document["observations"][fact];
    match envelope["status"].as_str() {
        Some("known") => envelope["value"].clone(),
        Some("absent") => Value::Null,
        _ => json!({ "$undefined": true }),
    }
}

/// plex_getter.json / seerr_getter.json: one property's value.
fn getter(given: &Value) -> Result<Value, String> {
    let property = given["property"].as_str().unwrap();
    let fact = match property {
        "lastViewedAt" => "last_watched_at",
        "seenBy" => "watched_by_user_ids",
        "isWatched" => "watched_by_anyone",
        "isRequested" => "requested",
        "addUser" => "requested_by_usernames",
        other => return Err(format!("no Scryer fact for {other}")),
    };
    let input = if let Some(requests) = given["requests"].as_array() {
        let ids: Vec<String> = requests
            .iter()
            .map(|r| r["requestedBy"]["id"].to_string())
            .collect();
        let usernames: HashMap<String, String> = requests
            .iter()
            .filter_map(|r| {
                let by = &r["requestedBy"];
                ["username", "plexUsername", "displayName"]
                    .iter()
                    .find_map(|k| by[*k].as_str())
                    .map(|name| (by["id"].to_string(), name.to_string()))
            })
            .collect();
        let requesters = (!ids.is_empty()).then_some(ids);
        input_for(
            requesters.as_deref(),
            &usernames,
            &MaintenanceWatchContext::default(),
            None,
        )
    } else {
        let users = given["users"].as_array().cloned().unwrap_or_default();
        let unwatched = json!({ "isWatched": false, "viewCount": 0 });
        let history = &match given.get("watchState") {
            Some(state) if *state == unwatched => json!([]),
            Some(state) => return Err(format!("no translation for watch state {state}")),
            None => given["history"].clone(),
        };
        let mut signals = Vec::new();
        for (i, row) in history.as_array().into_iter().flatten().enumerate() {
            let user = match row.get("accountID") {
                Some(account) => users
                    .iter()
                    .find(|u| &u["plexId"] == account)
                    .and_then(|u| u["username"].as_str())
                    .map(str::to_string),
                None => Some(format!("account-{i}")),
            };
            let played_at = row
                .get("viewedAt")
                .and_then(date_of)
                .and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
                .map(|d| d.with_timezone(&Utc));
            signals.push(signal(i, user, played_at));
        }
        let context = MaintenanceWatchContext {
            freshness: if history.is_array() {
                WatchSignalFreshness::Fresh
            } else {
                WatchSignalFreshness::Unavailable("parity_history_unreadable")
            },
            linked_user_ids: signals
                .iter()
                .filter_map(|s| s.scryer_user_id.clone())
                .collect(),
            unswept_user_ids: HashSet::new(),
            enabled_connection_ids: HashSet::from([CONNECTION.to_string()]),
            audience: WatchSignalFreshness::Fresh,
        };
        input_for(None, &HashMap::new(), &context, Some(signals.as_slice()))
    };
    let value = observed(&input, fact);
    Ok(match (property, value) {
        ("isRequested", Value::Bool(b)) => json!(i32::from(b)),
        ("lastViewedAt", Value::String(s)) => json!({ "$date": rfc3339(&s) }),
        (_, v) => v,
    })
}

fn expected_value(property: &str, value: &Value) -> Value {
    match property {
        "lastViewedAt" if value.get("$date").is_some() => json!({ "$date": date_of(value) }),
        _ => value.clone(),
    }
}

#[test]
fn mnt_maintenance() {
    let Some(mut suite) = Suite::open(SUITE) else { return };
    let dir = "corpus/reference/maintainerr/maintenance";

    let cases: Vec<Value> = corpus(&format!("{dir}/rule_group.json"));
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if RULE_GROUP_CASES.contains(&id) && suite.wants(id) {
            rule_group(&mut suite, id, case);
        }
    }

    for file in ["plex_getter.json", "seerr_getter.json"] {
        let cases: Vec<Value> = corpus(&format!("{dir}/{file}"));
        for case in &cases {
            let id = case["id"].as_str().unwrap();
            if !GETTER_CASES.contains(&id) || !suite.wants(id) {
                continue;
            }
            let given = &case["input"]["given"];
            let property = given["property"].as_str().unwrap();
            match getter(given) {
                Ok(actual) => suite.check(
                    id,
                    expected_value(property, &case["expect"]["value"]),
                    actual,
                ),
                Err(e) => suite.record(id, false, e),
            }
        }
    }
    suite.finish();
}
