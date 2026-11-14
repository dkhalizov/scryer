//! Parity net, area `maintenance`, stream "priv_maintenance" (stage 8, private
//! entries; catalogue files `*-config.yaml`, corpus under `corpus/captured/`).
//! No captured value is written here; data is read at run time by JSON key.
//!
//! Suites:
//! - `mnt_maintenance_config`: MNT-MAINTENANCE-5001 (settings.json key
//!   `rules_handler_job_cron`), 5002 (key `collection_handler_job_cron`), both
//!   in `corpus/captured/maintainerr/config/settings.json`.
//! - `prw_maintenance_config`: PRW-MAINTENANCE-5002 (the row with `taskName`
//!   CleanUpHistory, key `interval`, minutes, in
//!   `corpus/captured/prowlarr/config/system-task.json`).
//!
//! Scryer under test: `scryer_application::JobKey::interval_seconds()`
//! (jobs/definitions.rs): `MaintenanceRuleEvaluation`,
//! `LifecycleActionHandling`, `Housekeeping`. No seam needed.
//!
//! Translation table, reference -> Scryer:
//!
//! | reference | Scryer |
//! |---|---|
//! | `rules_handler_job_cron` (global rule-group schedule) | job `MaintenanceRuleEvaluation` (one global job, no per-group cron) |
//! | `collection_handler_job_cron` | job `LifecycleActionHandling` |
//! | cron `M H * * *`, H = `*/N` or `0-23/N` (24 % N == 0), M in 0..59, other fields `*` | period N*3600 s |
//! | any other cron shape | no translation: recorded failed, naming the key only |
//! | Prowlarr task `CleanUpHistory` | job `Housekeeping` (prunes Scryer's history) |
//! | `interval` (minutes) | period_seconds = interval * 60 |
//!
//! Compared: `{period_seconds}`. Ignored: the cron phase (minute and hour-0
//! anchor; Maintainerr fires at fixed wall-clock times, Scryer runs on an
//! interval with a per-instance jitter by design), every other settings.json
//! key, the `name` key and the other rows of system-task.json.
//!
//! Period-only greens (5001, 5002): the catalogue behaviour also names the
//! wall-clock anchor of the cron. That part is not compared: Maintainerr's
//! job scheduling is not ported (D33), and RFC 137 (jobs/definitions.rs, the
//! two interval constants) adopts the period as a default with no
//! clock-parity requirement. The green says "same period", not "same fire
//! times". If the stage owner wants the anchor held to D29 instead, add an
//! `anchored` key (expected true, Scryer false): both turn red.
//!
//! Weak green (PRW-5002): `Housekeeping.interval_seconds()` is the advertised
//! schedule; the scheduler loop keeps its own duration literal
//! (acquisition/workflow/task_runner.rs, housekeeping interval), so a change
//! to the real timer alone would not be caught. The job choice is not a
//! harness decision: Housekeeping is the only scheduled job that prunes
//! history records (jobs/housekeeping.rs), so the cadence has one
//! counterpart. PRW-5001's
//! retention has several (general `history_retention_days`, the fixed
//! release-decision and release-attempt retentions), hence unmapped.
//!
//! Unmapped (tests: []), with reasons:
//! - MNT-MAINTENANCE-5006..5011, 5014..5016 (rule groups, config/rules.json
//!   `rules[].ruleJson`): no expected answer. data/membership-*.json and
//!   data/cases/membership.json are empty, log rows carry no rule facts, no
//!   Plex corpus, Seerr requests carry no mediaAddedAt. Some rules use shapes
//!   outside the root operator table (BOOL custom value, NUMBER on DATE,
//!   Seerr media-added date). The D36 port decision is open.
//! - 5012, 5017, 5026 (collection arrAction, listExclusions): action
//!   execution is async AppUseCase code over repositories; a check against
//!   the action catalogue would measure a harness table. Known D29 gap for
//!   5012: Scryer has list exclusions (lists/ports.rs
//!   `ListExclusionRepository`), but maintenance deletion adds none
//!   (maintenance_rules/action_execution.rs, scoped_deletion.rs,
//!   sequence_execution.rs); expected red once execution is observable.
//! - 5023, 5027 (collection tagInArr off): Scryer tags members only through an
//!   explicit `AddTags` action step (maintenance_rules/action_catalog.rs), so
//!   "no tag written" holds for any rule without that step; the check would
//!   be vacuous. Map when a captured collection has tagInArr on.
//! - 5013, 5018 (media-server collection visibility): concept absent; the
//!   media-server client layer is not ported (D33).
//! - 5024, 5028 (log retention in months): Scryer prunes by days in async
//!   housekeeping; the value is an inline literal in async settings code, no
//!   visibility seam reaches it.
//! - 5003 (download client removal): maintenance deletion never calls a
//!   client; observing it needs async code and crate-private doubles.
//! - 5004, 5005 (tag in external arr on exclude): Scryer exclusions are
//!   internal and write no tags.
//! - 5020 (exclusions summary): the captured count (key `count`) is empty, so
//!   any check is vacuous; needs seams once rows exist.
//! - 5019 (metadata provider preference): out of scope under D33.
//! - 5021 (overlay settings): concept absent, no overlay job.
//! - PRW-MAINTENANCE-5001: history retention is an async setting plus private
//!   housekeeping constants; choosing one would be a harness decision.
//! - MNT-MAINTENANCE-6001..6011 (log patterns): counts of Maintainerr's own
//!   events, no facts a Scryer function can compute from.
//! - 5022, 5025 stay waived (D32).

use scryer_application::JobKey;
use scryer_parity::{Suite, corpus};
use serde_json::{Value, json};

/// `M H * * *` with H = `*/N` or `0-23/N`, 24 % N == 0 -> N*3600.
fn cron_period_seconds(cron: &str) -> Option<i64> {
    let f: Vec<&str> = cron.split_whitespace().collect();
    if f.len() != 5 || f[2..].iter().any(|x| *x != "*") {
        return None;
    }
    if f[0].parse::<u32>().ok()? >= 60 {
        return None;
    }
    let n: i64 = f[1]
        .strip_prefix("*/")
        .or_else(|| f[1].strip_prefix("0-23/"))?
        .parse()
        .ok()?;
    (1..=24).contains(&n).then_some(())?;
    (24 % n == 0).then_some(n * 3600)
}

#[test]
fn mnt_maintenance_config() {
    let Some(mut suite) =
        Suite::open("scryer-application::parity_maintenance::mnt_maintenance_config")
    else {
        return;
    };
    let settings: Value = corpus("corpus/captured/maintainerr/config/settings.json");
    let rows = [
        ("MNT-MAINTENANCE-5001", "rules_handler_job_cron", JobKey::MaintenanceRuleEvaluation),
        ("MNT-MAINTENANCE-5002", "collection_handler_job_cron", JobKey::LifecycleActionHandling),
    ];
    for (id, key, job) in rows {
        if !suite.wants(id) {
            continue;
        }
        match settings[key].as_str().and_then(cron_period_seconds) {
            Some(p) => suite.check(
                id,
                json!({"period_seconds": p}),
                json!({"period_seconds": job.interval_seconds()}),
            ),
            None => suite.record(
                id,
                false,
                &format!("no translation for the cron at settings.json key {key}"),
            ),
        }
    }
    suite.finish();
}

#[test]
fn prw_maintenance_config() {
    let Some(mut suite) =
        Suite::open("scryer-application::parity_maintenance::prw_maintenance_config")
    else {
        return;
    };
    let tasks: Vec<Value> = corpus("corpus/captured/prowlarr/config/system-task.json");
    let id = "PRW-MAINTENANCE-5002";
    if suite.wants(id) {
        let minutes = tasks
            .iter()
            .find(|r| r["taskName"] == "CleanUpHistory")
            .and_then(|r| r["interval"].as_i64());
        match minutes {
            Some(m) => suite.check(
                id,
                json!({"period_seconds": m * 60}),
                json!({"period_seconds": JobKey::Housekeeping.interval_seconds()}),
            ),
            None => suite.record(id, false, "no CleanUpHistory row"),
        }
    }
    suite.finish();
}
