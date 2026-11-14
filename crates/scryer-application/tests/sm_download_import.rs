//! Parity net, phase 5: state-machine test of the download -> import lifecycle
//! (notes/11-parity-net-design.md § Checks beyond examples).
//!
//! # What runs for real
//!
//! The synchronous core of one poller tick for one tracked download, on a
//! real `TrackedDownloadService`, in the order
//! `integration/workflow/tracked_commands.rs` (Phase 1 of
//! `process_tracked_download_snapshot`) calls it:
//!
//! - `TrackedDownloadService::fail_persistent_warning_for_download` (24 h
//!   client-warning timeout, `now` passed in, so the clock is the model's);
//! - `failed_download_handler::check` (client failure -> FailedPending);
//! - `TrackedDownloadService::update_trackable` (client stopped listing it);
//! - `completed_download_handler::mark_importing` (dispatch starts an import);
//! - `TrackedDownload::schedule_import_execution_retry` (import failed to run,
//!   back to ImportPending with backoff);
//! - `TrackedDownload::import_retry_deferred` (the backoff half of the
//!   dispatch gate), compared with the model on every step.
//!
//! # What does not, and why
//!
//! `TrackedDownloadService::track`, `completed_download_handler::check`,
//! `completed_download_handler::import`, `failed_download_handler::process_failed`
//! and `process_tracked_download_snapshot` all take `&AppUseCase`. Building one
//! needs `AppServices` with every repository port; the in-crate fakes
//! (`null_repositories::test_nulls`, the `Test*` repos in
//! `tracked_downloads.rs` tests) are `#[cfg(test)]` and invisible here, and
//! `track()` also calls the download registry, submission repository, domain
//! events and acquisition-scope repositories on every observation. Driving
//! that from an integration test would mean re-implementing ~30 repository
//! traits; a seam would have to make `test_nulls` plus an `AppUseCase` test
//! builder `pub` (behind a `test-support` feature). Until then:
//!
//! - the *state written by async code* (Downloading -> ImportPending by the
//!   completed check, Importing -> Imported by the import result,
//!   FailedPending -> Failed by `process_failed`) is not faked: instead the
//!   initial tracked state is generated over all eleven states, so every
//!   state the async code can leave behind is covered as a starting point;
//! - a client observation is injected the way `track()` refreshes a known
//!   row: replace `client_item` (here: its `state` and attention fields).
//!   The other lines `track()` runs on refresh (status reset in Downloading,
//!   `is_trackable = true`) are product logic behind `AppUseCase` and are not
//!   copied; once the client stops listing a job the model never observes it
//!   again.
//!
//! The Seen-dispatch filter itself (`trackable_import_work_completed_lookup_items`
//! in `tracked_commands.rs`) is private and reads `Utc::now()`, so it is not
//! run. The model's `ImportStarts` precondition restates it (trackable,
//! ImportPending, backoff over), and the real backoff predicate
//! `import_retry_deferred` is checked against the model after every step.
//! "Imported at most once" therefore rests on that restated gate plus the
//! settled-state checks below; the filter's composition (in-flight set, wall
//! clock) is not covered.
//!
//! # Invariants (checked after every real step)
//!
//! - the real state, trackability, retry attempts, retry due time and
//!   backoff verdict equal the reference model's prediction;
//! - terminal states stay terminal, import-settled states stay put;
//! - an import-settled download never reaches FailedPending (the re-grab
//!   path: `process_failed` re-searches from there), so a completed import is
//!   never re-grabbed;
//! - a failed download (FailedPending/Failed) never reaches an import state;
//! - observing the same client state twice at the same instant changes
//!   nothing (state, status, messages, trackability).
//!
//! # Coverage floor
//!
//! The run counts how often the interesting preconditions are reached (an
//! imported download seeing a scryer-origin client failure, the warning
//! timeout firing, a retry window holding dispatch back, a re-dispatch after a
//! retry) and fails if any falls under a floor, so a generator change cannot
//! quietly make those checks vacuous.
//!
//! Deterministic: fixed seed, no failure persistence, bounded cases and steps,
//! fixed `DownloadId`, model clock. (`restore_cleanup_download` and the cache
//! prune read `Utc::now()` for cache TTL only; one entry never ages out within
//! a test.)

use chrono::{DateTime, Duration, TimeZone, Utc};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, RngSeed, TestRunner};
use proptest_state_machine::{ReferenceStateMachine, StateMachineTest};
use scryer_application::failed_download_handler;
use scryer_application::parity_seams::mark_importing;
use scryer_application::tracked_downloads::{TrackedDownload, TrackedDownloadService};
use scryer_domain::download_identity::DownloadId;
use scryer_domain::{
    DownloadQueueItem, DownloadQueueState, TitleMatchType, TrackedDownloadState,
    TrackedDownloadStatus,
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

const HOUR: i64 = 3600;
const WARNING_TIMEOUT_SECS: i64 = 24 * HOUR; // TrackedDownloadService::WARNING_FAILURE_TIMEOUT

/// Backoff after the n-th failed import execution, as documented on
/// `import_execution_retry_delay`: 30 s, 2 min, 5 min, then 15 min.
fn retry_delay_secs(attempts: u32) -> i64 {
    match attempts {
        0 | 1 => 30,
        2 => 120,
        3 => 300,
        _ => 900,
    }
}
const ID: &str = "client-1:sm-1";

fn download_id() -> DownloadId {
    DownloadId::parse("5c0a7e1d-0000-4000-8000-000000000001").unwrap()
}

fn epoch() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
}

const ALL_STATES: [TrackedDownloadState; 11] = [
    TrackedDownloadState::Downloading,
    TrackedDownloadState::Missing,
    TrackedDownloadState::ImportPending,
    TrackedDownloadState::Importing,
    TrackedDownloadState::Imported,
    TrackedDownloadState::ImportedSeeding,
    TrackedDownloadState::ImportBlocked,
    TrackedDownloadState::FailedPending,
    TrackedDownloadState::Failed,
    TrackedDownloadState::ImportDismissed,
    TrackedDownloadState::Ignored,
];

const CLIENT_STATES: [DownloadQueueState; 6] = [
    DownloadQueueState::Queued,
    DownloadQueueState::Downloading,
    DownloadQueueState::Paused,
    DownloadQueueState::Completed,
    DownloadQueueState::Failed,
    DownloadQueueState::Warning,
];

// ── Reference model ─────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Model {
    state: TrackedDownloadState,
    client: DownloadQueueState,
    scryer_origin: bool,
    submission_match: bool,
    blocklist_confirmation_failed: bool,
    trackable: bool,
    listed: bool,
    now_s: i64,
    warning_since_s: Option<i64>,
    retry_attempts: u32,
    retry_due_s: Option<i64>,
}

impl Model {
    fn check_eligible(&self) -> bool {
        !self.blocklist_confirmation_failed
            && matches!(
                self.state,
                TrackedDownloadState::Downloading
                    | TrackedDownloadState::ImportPending
                    | TrackedDownloadState::ImportBlocked
            )
    }
    fn foreign(&self) -> bool {
        !self.scryer_origin && !self.submission_match
    }
    fn retry_deferred(&self) -> bool {
        self.retry_due_s.is_some_and(|due| due > self.now_s)
    }
}

#[derive(Clone, Debug)]
enum Step {
    /// Client reports this state on a poll; `timeout_applies` is the caller's
    /// `warning_timeout_applies` (false under a seeding profile).
    Observe {
        client: DownloadQueueState,
        timeout_applies: bool,
    },
    Tick { secs: i64 },
    /// An authoritative listing no longer contains the job.
    ClientRemoves,
    ImportStarts,
    ImportExecutionFails,
}

fn client_state() -> impl Strategy<Value = DownloadQueueState> {
    client_state_weighted(3)
}

fn client_state_weighted(warning: u32) -> impl Strategy<Value = DownloadQueueState> {
    prop_oneof![
        warning => Just(DownloadQueueState::Warning),
        2 => Just(DownloadQueueState::Failed),
        1 => proptest::sample::select(&CLIENT_STATES[..]),
    ]
}

// Coverage counters (see "Coverage floor"); one test per process under nextest.
static IMPORTED_SEES_FAILURE: AtomicUsize = AtomicUsize::new(0);
static WARNING_TIMEOUT_FIRES: AtomicUsize = AtomicUsize::new(0);
static DISPATCH_HELD_BY_BACKOFF: AtomicUsize = AtomicUsize::new(0);
static REDISPATCH_AFTER_RETRY: AtomicUsize = AtomicUsize::new(0);

struct Lifecycle;

impl ReferenceStateMachine for Lifecycle {
    type State = Model;
    type Transition = Step;

    fn init_state() -> BoxedStrategy<Model> {
        (
            // Settled states are weighted up for the re-grab check, and the
            // check-eligible ones for the failure and warning paths.
            prop_oneof![
                2 => proptest::sample::select(&ALL_STATES[..]),
                1 => proptest::sample::select(&[
                    TrackedDownloadState::Imported,
                    TrackedDownloadState::ImportedSeeding,
                ][..]),
                1 => proptest::sample::select(&[
                    TrackedDownloadState::Downloading,
                    TrackedDownloadState::ImportPending,
                    TrackedDownloadState::ImportBlocked,
                ][..]),
            ],
            client_state(),
            proptest::bool::weighted(0.75),
            any::<bool>(),
            proptest::bool::weighted(0.1),
        )
            .prop_map(|(state, client, scryer_origin, submission_match, bcf)| Model {
                state,
                client,
                scryer_origin,
                submission_match,
                blocklist_confirmation_failed: bcf,
                trackable: true,
                listed: true,
                now_s: 0,
                warning_since_s: None,
                retry_attempts: 0,
                retry_due_s: None,
            })
            .boxed()
    }

    fn transitions(m: &Model) -> BoxedStrategy<Step> {
        // Warning and Failed are weighted up so client failures on every
        // tracked state are common; while a warning clock runs, Warning is
        // weighted further, and hour ticks added, so warn -> 24 h -> warn runs
        // reach the timeout.
        let clock = m.warning_since_s.is_some();
        let (warning, long_ticks) = if clock { (12, 6) } else { (3, 0) };
        prop_oneof![
            8 => (client_state_weighted(warning), proptest::bool::weighted(0.8))
                .prop_map(|(client, timeout_applies)| Step::Observe { client, timeout_applies }),
            // Short ticks hit the retry backoff boundaries (30 s .. 15 min),
            // hour ticks cluster around the 24 h warning timeout.
            4 => proptest::sample::select(
                &[30i64, 90, 120, 300, 900, HOUR, 6 * HOUR, 12 * HOUR, 23 * HOUR, 24 * HOUR, 25 * HOUR][..],
            )
            .prop_map(|secs| Step::Tick { secs }),
            long_ticks => proptest::sample::select(&[12 * HOUR, 23 * HOUR, 24 * HOUR][..])
                .prop_map(|secs| Step::Tick { secs }),
            // Rare: once removed the job is never observed again.
            1 => Just(Step::ClientRemoves),
            2 => Just(Step::ImportStarts),
            2 => Just(Step::ImportExecutionFails),
        ]
        .boxed()
    }

    fn preconditions(m: &Model, step: &Step) -> bool {
        match step {
            Step::Observe { .. } | Step::ClientRemoves => m.listed,
            // The Seen-dispatch filter in tracked_commands.rs only hands
            // trackable, ImportPending rows outside their retry backoff to the
            // import worker.
            Step::ImportStarts => {
                m.trackable && m.state == TrackedDownloadState::ImportPending && !m.retry_deferred()
            }
            // The import worker reschedules only the import it was running.
            Step::ImportExecutionFails => m.state == TrackedDownloadState::Importing,
            Step::Tick { .. } => true,
        }
    }

    fn apply(mut m: Model, step: &Step) -> Model {
        match *step {
            Step::Observe {
                client,
                timeout_applies,
            } => {
                m.client = client;
                // fail_persistent_warning_for_download
                let actionable = m.scryer_origin
                    && m.check_eligible()
                    && client == DownloadQueueState::Warning;
                if !actionable || !timeout_applies {
                    m.warning_since_s = None;
                } else {
                    let since = *m.warning_since_s.get_or_insert(m.now_s);
                    if m.now_s - since >= WARNING_TIMEOUT_SECS {
                        m.state = TrackedDownloadState::FailedPending;
                        m.warning_since_s = None;
                    }
                }
                // failed_download_handler::check
                if m.check_eligible() && client == DownloadQueueState::Failed && m.scryer_origin
                {
                    m.state = TrackedDownloadState::FailedPending;
                }
            }
            Step::Tick { secs } => m.now_s += secs,
            Step::ClientRemoves => {
                m.listed = false;
                let import_under_way =
                    m.state == TrackedDownloadState::Importing || m.retry_attempts > 0;
                let preserve = matches!(
                    m.state,
                    TrackedDownloadState::ImportPending
                        | TrackedDownloadState::Importing
                        | TrackedDownloadState::ImportBlocked
                        | TrackedDownloadState::FailedPending
                ) && (!m.foreign() || import_under_way);
                if !preserve {
                    m.trackable = false;
                    m.warning_since_s = None;
                }
            }
            Step::ImportStarts => m.state = TrackedDownloadState::Importing,
            Step::ImportExecutionFails => {
                m.state = TrackedDownloadState::ImportPending;
                m.retry_attempts += 1;
                m.retry_due_s = Some(m.now_s + retry_delay_secs(m.retry_attempts));
            }
        }
        m
    }
}

// ── System under test ───────────────────────────────────────────────────────

fn client_item(m: &Model) -> DownloadQueueItem {
    DownloadQueueItem {
        id: "queue-1".to_string(),
        title_id: None,
        episode_id: None,
        title_name: "Some.Show.S01E01.1080p.WEB-DL-GRP".to_string(),
        facet: Some("series".to_string()),
        category: None,
        client_id: "client-1".to_string(),
        client_name: "qBittorrent".to_string(),
        client_type: "qbittorrent".to_string(),
        state: m.client,
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
        download_client_item_id: "sm-1".to_string(),
        download_id: None,
        import_status: None,
        import_type: None,
        import_error_code: None,
        import_error_message: None,
        imported_at: None,
        delete_status: None,
        delete_error_message: None,
        source_provider: None,
        is_scryer_origin: m.scryer_origin,
        tracked_state: None,
        tracked_status: None,
        tracked_status_messages: vec![],
        tracked_match_type: None,
        seeding: None,
    }
}

fn set_client_state(item: &mut DownloadQueueItem, client: DownloadQueueState) {
    item.state = client;
    let attention = matches!(
        client,
        DownloadQueueState::Failed | DownloadQueueState::Warning
    );
    item.attention_required = attention;
    item.attention_reason = attention.then(|| "files are missing".to_string());
}

struct Sut {
    svc: TrackedDownloadService,
}

impl Sut {
    fn td(&self) -> &TrackedDownload {
        self.svc
            .get_by_download_id(download_id())
            .expect("the single tracked download stays cached")
    }
    fn td_mut(&mut self) -> &mut TrackedDownload {
        self.svc.get_mut_by_download_id(download_id()).unwrap()
    }
    /// One observation as tracked_commands.rs Phase 1 runs it.
    fn observe(&mut self, client: DownloadQueueState, now: DateTime<Utc>, applies: bool) {
        set_client_state(&mut self.td_mut().client_item, client);
        self.svc
            .fail_persistent_warning_for_download(download_id(), now, applies);
        failed_download_handler::check(self.td_mut());
    }
    fn snapshot(&self) -> (TrackedDownloadState, TrackedDownloadStatus, Vec<String>, bool) {
        let td = self.td();
        (
            td.state,
            td.status,
            td.status_messages.iter().map(|m| m.text.clone()).collect(),
            td.is_trackable,
        )
    }
}

struct LifecycleTest;

impl StateMachineTest for LifecycleTest {
    type SystemUnderTest = Sut;
    type Reference = Lifecycle;

    fn init_test(m: &Model) -> Sut {
        let td = TrackedDownload {
            download_id: download_id(),
            id: ID.to_string(),
            client_id: "client-1".to_string(),
            client_type: "qbittorrent".to_string(),
            client_item: client_item(m),
            completed_source: None,
            state: m.state,
            status: TrackedDownloadStatus::Ok,
            status_messages: Vec::new(),
            title_id: Some("title-1".to_string()),
            facet: Some("series".to_string()),
            source_title: None,
            indexer: None,
            added_at: None,
            notified_manual_interaction: false,
            match_type: if m.submission_match {
                TitleMatchType::Submission
            } else {
                TitleMatchType::TitleParse
            },
            is_trackable: true,
            import_attempted: false,
            waiting_for_completed_history: false,
            path_missing_since: None,
            no_video_import_retry: None,
            import_execution_retry: None,
            import_hold: None,
            skip_reacquire_on_failure: false,
            manual_failure_cleanup: false,
            blocklist_confirmation_failed: m.blocklist_confirmation_failed,
            burned_by_import_gate: false,
            snapshot_missing_since: None,
            retained_in_client_after_cleanup: false,
        };
        let mut svc = TrackedDownloadService::new();
        svc.restore_cleanup_download(td);
        Sut { svc }
    }

    fn apply(mut sut: Sut, after: &Model, step: Step) -> Sut {
        let before = sut.td().state;
        let now = epoch() + Duration::seconds(after.now_s);
        match step {
            Step::Observe {
                client,
                timeout_applies,
            } => {
                if client == DownloadQueueState::Failed
                    && before.counts_as_imported()
                    && after.scryer_origin
                    && !after.blocklist_confirmation_failed
                {
                    IMPORTED_SEES_FAILURE.fetch_add(1, Relaxed);
                }
                if client == DownloadQueueState::Warning
                    && before != TrackedDownloadState::FailedPending
                    && after.state == TrackedDownloadState::FailedPending
                {
                    WARNING_TIMEOUT_FIRES.fetch_add(1, Relaxed);
                }
                sut.observe(client, now, timeout_applies);
                let once = sut.snapshot();
                sut.observe(client, now, timeout_applies);
                assert_eq!(
                    sut.snapshot(),
                    once,
                    "observing {client:?} twice at the same instant must change nothing"
                );
            }
            Step::Tick { .. } => {}
            Step::ClientRemoves => {
                sut.svc.update_trackable(&HashSet::new());
            }
            Step::ImportStarts => {
                if after.retry_attempts > 0 {
                    REDISPATCH_AFTER_RETRY.fetch_add(1, Relaxed);
                }
                mark_importing(sut.td_mut());
            }
            Step::ImportExecutionFails => {
                sut.td_mut()
                    .schedule_import_execution_retry(now, |n, _| format!("retry {n}"));
            }
        }
        let after_state = sut.td().state;
        if after.state == TrackedDownloadState::ImportPending && after.trackable && after.retry_deferred() {
            DISPATCH_HELD_BY_BACKOFF.fetch_add(1, Relaxed);
        }

        if after_state == TrackedDownloadState::FailedPending && before != after_state {
            assert_eq!(sut.td().status, TrackedDownloadStatus::Error);
        }

        if before.is_terminal() || before.is_import_settled() {
            assert_eq!(after_state, before, "settled state {before:?} moved");
        }
        if before.counts_as_imported() {
            assert_ne!(
                after_state,
                TrackedDownloadState::FailedPending,
                "imported download entered the re-grab path"
            );
        }
        if matches!(
            before,
            TrackedDownloadState::FailedPending | TrackedDownloadState::Failed
        ) {
            assert!(
                !matches!(
                    after_state,
                    TrackedDownloadState::ImportPending
                        | TrackedDownloadState::Importing
                        | TrackedDownloadState::Imported
                        | TrackedDownloadState::ImportedSeeding
                ),
                "failed download reached import state {after_state:?}"
            );
        }
        sut
    }

    fn check_invariants(sut: &Sut, m: &Model) {
        let td = sut.td();
        assert_eq!(td.state, m.state, "state diverged from the model");
        assert_eq!(td.is_trackable, m.trackable, "trackability diverged");
        assert_eq!(
            td.import_execution_retry.as_ref().map_or(0, |r| r.attempts),
            m.retry_attempts,
            "retry attempts diverged"
        );
        assert_eq!(
            td.import_execution_retry.as_ref().map(|r| r.next_retry_at),
            m.retry_due_s.map(|s| epoch() + Duration::seconds(s)),
            "retry due time diverged"
        );
        assert_eq!(
            td.import_retry_deferred(epoch() + Duration::seconds(m.now_s)),
            m.retry_deferred(),
            "dispatch backoff verdict diverged"
        );
    }
}

/// Each coverage floor is about a third of what the fixed seed reaches today.
const COVERAGE_FLOORS: [(&str, &AtomicUsize, usize); 4] = [
    ("imported download sees a scryer-origin client failure", &IMPORTED_SEES_FAILURE, FLOOR_IMPORTED),
    ("warning timeout fires", &WARNING_TIMEOUT_FIRES, FLOOR_WARNING),
    ("dispatch held back by retry backoff", &DISPATCH_HELD_BY_BACKOFF, FLOOR_HELD),
    ("re-dispatch after an import retry", &REDISPATCH_AFTER_RETRY, FLOOR_REDISPATCH),
];
const FLOOR_IMPORTED: usize = 70;
const FLOOR_WARNING: usize = 7;
const FLOOR_HELD: usize = 50;
const FLOOR_REDISPATCH: usize = 20;

#[test]
fn download_import_lifecycle() {
    let config = ProptestConfig {
        cases: 512,
        rng_seed: RngSeed::Fixed(0x5d0a_1e0c),
        failure_persistence: None,
        max_shrink_iters: 4096,
        ..ProptestConfig::default()
    };
    let mut runner = TestRunner::new(config.clone());
    let result = runner.run(&Lifecycle::sequential_strategy(1..40), |(init, steps, seen)| {
        LifecycleTest::test_sequential(config.clone(), init, steps, seen);
        Ok(())
    });
    if let Err(err) = result {
        panic!("{err}\n{runner}");
    }
    for (what, counter, floor) in COVERAGE_FLOORS {
        let hits = counter.load(Relaxed);
        eprintln!("coverage: {what}: {hits}");
        assert!(hits >= floor, "coverage floor: {what} reached {hits} times, floor {floor}");
    }
}
