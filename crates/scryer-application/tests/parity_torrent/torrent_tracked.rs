//! Parity net, torrent area, tracked downloads (stream torrent_tracked).
//!
//! Suite: `scryer-application::parity_torrent::son_failed_check`: Sonarr
//! FailedDownloadService.ProcessFixture, SON-TORRENT-0100..0102
//! (corpus/reference/sonarr/torrent/FailedDownloadService.ProcessFixture.json,
//! input.op `failed_download.check`).
//!
//! Selection happens before `wants()`: only that file, only that op, and the id
//! set is asserted to be exactly 0100..0102.
//!
//! Scryer code under test: `failed_download_handler::check`
//! (import/failed_download.rs:16), sync, on a `TrackedDownload` built as a
//! struct literal (fields as in tests/sm_download_import.rs).
//!
//! Translation (reference -> Scryer):
//!
//! | Reference | Scryer |
//! |---|---|
//! | given.tracked_state `Downloading` | `TrackedDownload.state` via `TrackedDownloadState::from_str_opt(snake_case)` |
//! | download_item.status `Completed`/`Failed` | `client_item.state` = `DownloadQueueState::<same>` |
//! | grabbed_history_count 0 / n>0 | `client_item.is_scryer_origin` false / true (check's only "grabbed" evidence) |
//! | download_item.title | `client_item.title_name` |
//! | download_item.download_id | `client_item.download_client_item_id`, `TrackedDownload.id` = `client-1:<id>` |
//! | expect state `{not: Failed}` | actual `{not: Failed}` unless state is Failed/FailedPending |
//! | expect download_failed_events n | 1 if state is FailedPending/Failed, else 0 (check publishes nothing; the event comes from process_failed) |
//! | expect status_messages `{not_empty}` | `!status_messages.is_empty()` |
//!
//! Ignored input keys: download_item.output_path (check reads no path),
//! remote_episode.episode_ids (Scryer binds title_id; check reads neither),
//! note. Any other expect key panics.
//!
//! Self-check against a vacuous green: for every Failed-status case, the same
//! input with `is_scryer_origin = true` must reach FailedPending, so the
//! warning comes from the not-grabbed branch, not an early return.
//!
//! Unmapped (tests: []), with reasons:
//! - SON-TORRENT-0103..0108 (TrackedDownloadAlreadyImported): the rule lives in
//!   `verify_import_outcome_with_mode` (import/completed_download/verification.rs:215),
//!   an async fn on `&AppUseCase` reading import-artifact rows; no pure function,
//!   and Scryer keys on artifact rows, not event order per episode.
//! - SON-TORRENT-0109..0111, RAD-TORRENT-0007, 0008, 0012 (TrackedDownloadService
//!   mapping through grab history): `TrackedDownloadService::track`
//!   (integration/tracked_downloads.rs:519), `resolve_title` (:1607) take
//!   `&AppUseCase`/repositories; no public fakes. Note for 0110 and RAD-0008
//!   (indexer from the grab): Scryer exposes the grab's indexer as
//!   `DownloadQueueItem.source_provider` (integration/workflow/queue_queries.rs:526,
//!   from the submission). `TrackedDownload.indexer` (:104) is never filled from a
//!   submission (only merged at :291) and is read only by the seeding event
//!   (tracked_commands.rs:4139). A future mapping should compare source_provider;
//!   the verdict is open.
//! - RAD-TORRENT-0011 (state from latest history): `reconstruct_state` (:1755)
//!   reads the durable tracked-state marker through `&AppUseCase`.
//! - SON-TORRENT-0112, 0113: concept absent (a tracked download binds a title_id,
//!   not episode ids; nothing reacts to episode removal).
//! - SON-TORRENT-0114, RAD-TORRENT-0009, 0010 (series/movie deleted): the delete
//!   path sends `TrackedDownloadCommand::ForgetMany`, handled inline with
//!   `&AppUseCase` (integration/workflow/tracked_commands.rs:2970-2990).
//!   `forget_for_title` (tracked_downloads.rs:1421) is not what delete runs.
//! - SON-TORRENT-0115..0124, 0126 (DownloadService.DownloadReport): assertions are
//!   mock call counts; client choice and backoff live in scryer-infrastructure-acquisition
//!   (router.rs:915, :938; search_client.rs:4495), unreachable from this crate
//!   without a back dependency (D27). Finding: the router skips backed-off clients
//!   (router.rs:934-982) where 0124 expects a grab, so 0124 would be red.
//! - RAD-TORRENT-0001..0005 (CompletedDownloadService.Import/VerifyImport): movie
//!   rule inline in `verify_import_outcome_with_mode` (verification.rs:309-315)
//!   over repository artifacts, `&AppUseCase`; same reason as SON-IMPORT-0001..0020.

use scryer_application::failed_download_handler;
use scryer_application::tracked_downloads::TrackedDownload;
use scryer_domain::download_identity::DownloadId;
use scryer_domain::{
    DownloadQueueItem, DownloadQueueState, TitleMatchType, TrackedDownloadState,
    TrackedDownloadStatus,
};
use scryer_parity::{Suite, corpus};
use serde_json::{Value, json};

fn snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn client_state(name: &str) -> DownloadQueueState {
    match name {
        "Queued" => DownloadQueueState::Queued,
        "Paused" => DownloadQueueState::Paused,
        "Downloading" => DownloadQueueState::Downloading,
        "Completed" => DownloadQueueState::Completed,
        "Failed" => DownloadQueueState::Failed,
        "Warning" => DownloadQueueState::Warning,
        other => panic!("unmapped download_item.status {other}"),
    }
}

fn build(given: &Value, origin: bool) -> TrackedDownload {
    let item = &given["download_item"];
    let ref_id = item["download_id"].as_str().unwrap();
    let state_name = given["tracked_state"].as_str().unwrap();
    let state = TrackedDownloadState::from_str_opt(&snake_case(state_name))
        .unwrap_or_else(|| panic!("unmapped tracked_state {state_name}"));
    let client_item = DownloadQueueItem {
        id: "queue-1".to_string(),
        title_id: None,
        episode_id: None,
        title_name: item["title"].as_str().unwrap().to_string(),
        facet: Some("series".to_string()),
        category: None,
        client_id: "client-1".to_string(),
        client_name: "qBittorrent".to_string(),
        client_type: "qbittorrent".to_string(),
        state: client_state(item["status"].as_str().unwrap()),
        progress_percent: 50,
        import_transfer_phase: None,
        import_transfer_bytes: None,
        import_transfer_total_bytes: None,
        import_transfer_started_at: None,
        import_transfer_updated_at: None,
        size_bytes: None,
        remaining_seconds: None,
        queued_at: None,
        last_updated_at: None,
        attention_required: false,
        attention_reason_code: None,
        attention_reason: None,
        download_client_item_id: ref_id.to_string(),
        download_id: None,
        import_status: None,
        import_type: None,
        import_error_code: None,
        import_error_message: None,
        imported_at: None,
        delete_status: None,
        delete_error_message: None,
        source_provider: None,
        is_scryer_origin: origin,
        tracked_state: None,
        tracked_status: None,
        tracked_status_messages: vec![],
        tracked_match_type: None,
        seeding: None,
    };
    TrackedDownload {
        download_id: DownloadId::parse("5c0a7e1d-0000-4000-8000-000000000001").unwrap(),
        id: format!("client-1:{ref_id}"),
        client_id: "client-1".to_string(),
        client_type: "qbittorrent".to_string(),
        client_item,
        completed_source: None,
        state,
        status: TrackedDownloadStatus::Ok,
        status_messages: Vec::new(),
        title_id: Some("title-1".to_string()),
        facet: Some("series".to_string()),
        source_title: None,
        indexer: None,
        added_at: None,
        notified_manual_interaction: false,
        match_type: TitleMatchType::TitleParse,
        is_trackable: true,
        import_attempted: false,
        waiting_for_completed_history: false,
        path_missing_since: None,
        no_video_import_retry: None,
        import_execution_retry: None,
        import_hold: None,
        skip_reacquire_on_failure: false,
        manual_failure_cleanup: false,
        blocklist_confirmation_failed: false,
        burned_by_import_gate: false,
        snapshot_missing_since: None,
        retained_in_client_after_cleanup: false,
    }
}

#[test]
fn son_failed_check() {
    let Some(mut suite) = Suite::open("scryer-application::parity_torrent::son_failed_check") else {
        return;
    };
    let all: Vec<Value> =
        corpus("corpus/reference/sonarr/torrent/FailedDownloadService.ProcessFixture.json");
    let cases: Vec<Value> = all
        .into_iter()
        .filter(|c| c["input"]["op"] == "failed_download.check")
        .collect();
    let ids: Vec<&str> = cases.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["SON-TORRENT-0100", "SON-TORRENT-0101", "SON-TORRENT-0102"]);

    for c in &cases {
        let id = c["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let given = &c["input"]["given"];
        let origin = given["grabbed_history_count"].as_u64().unwrap() > 0;
        let mut td = build(given, origin);
        failed_download_handler::check(&mut td);

        if given["download_item"]["status"] == "Failed" {
            let mut twin = build(given, true);
            failed_download_handler::check(&mut twin);
            assert_eq!(twin.state, TrackedDownloadState::FailedPending, "{id} self-check");
        }

        let failed = matches!(
            td.state,
            TrackedDownloadState::Failed | TrackedDownloadState::FailedPending
        );
        let expect = c["expect"].as_object().unwrap();
        let mut actual = serde_json::Map::new();
        for (k, v) in expect {
            let a = match k.as_str() {
                "state" => {
                    // matcher {"not": X}: actual is {"not": X} only while production's
                    // state name differs from X, else {"is": <state>}.
                    let banned = v["not"].as_str().unwrap_or_else(|| panic!("{id}: state matcher"));
                    let name = format!("{:?}", td.state);
                    if failed || name == banned { json!({"is": name}) } else { json!({"not": banned}) }
                }
                "download_failed_events" => json!(u32::from(failed)),
                "status_messages" => json!({"not_empty": !td.status_messages.is_empty()}),
                other => panic!("{id}: unmapped expect key {other}"),
            };
            actual.insert(k.clone(), a);
        }
        suite.check(id, c["expect"].clone(), Value::Object(actual));
    }
    suite.finish();
}
