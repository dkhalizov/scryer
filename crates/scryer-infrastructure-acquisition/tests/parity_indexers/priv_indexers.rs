//! Parity net, indexers stream, private config entries (Stage 8).
//!
//! Suite: `scryer-infrastructure-acquisition::parity_indexers::prw_indexers_sync_cadence`
//! IDs: PRW-INDEXERS-5031 (selected by ref, `corpus/captured/prowlarr/config/
//! system-task.json#taskName=ApplicationIndexerSync`; the other task in that
//! file stays unmapped).
//!
//! Scryer code under test: `JobKey::ProwlarrSync.interval_seconds()`
//! (scryer-application jobs/definitions.rs, pub), the declared schedule of
//! Scryer's pull from Prowlarr.
//!
//! Translation table (reference -> Scryer):
//!
//! | Prowlarr | Scryer |
//! |---|---|
//! | task ApplicationIndexerSync (Prowlarr pushes its indexers to every app) | job ProwlarrSync (Scryer pulls Prowlarr's indexers; the pull replaces the push) |
//! | system-task.json key `interval` (minutes) | `interval_seconds() / 60` (None -> null) |
//!
//! Expected is read from the corpus at run time. Ignored key of system-task.json:
//! `name` (display label).
//! Known limit: the task runner's tick uses inline literals that agree with
//! `interval_seconds` (task_runner.rs:6699, :7076) but do not read it, so drift
//! between the two would not be caught.
//!
//! Unmapped (tests: []), with reasons. All of these run on the hop from
//! Prowlarr to the tracker or in Prowlarr's own host, scheduler or Cardigann
//! engine, which stay in Prowlarr (D33); Scryer reads Prowlarr's Torznab output.
//! - PRW-INDEXERS-5020..5028 (`indexer.json#id=N/settings`): tracker URL,
//!   definition settings, advertised result limits. Scryer stores caps limits
//!   (ProwlarrCapsSnapshot) but parses them from caps XML the corpus lacks;
//!   building XML from `capabilities` and parsing it back would echo the input.
//! - PRW-INDEXERS-5006, 5009, 5011, 5014 (titleRewrite): Prowlarr applies it
//!   when building results; Scryer receives titles already rewritten and has
//!   no rewrite step.
//! - PRW-INDEXERS-5029, 5030 (`host.json` keys proxy, certificateValidation):
//!   Prowlarr's own outbound HTTP to trackers, a hop Scryer never makes (D33).
//!   Scryer proxies are per indexer, and synced children inherit the Prowlarr
//!   parent's proxy (scryer-application integration/workflow/indexers.rs:1726,
//!   :1806), which covers Scryer's hop to Prowlarr, not this one. Observing
//!   that inheritance needs the AppUseCase sync workflow with repositories,
//!   more than a visibility seam. Scryer has no certificate-validation setting.
//! - PRW-INDEXERS-5032 (task IndexerDefinitionUpdate): Scryer's
//!   PluginRegistryRefresh refreshes Scryer's own indexer plugins, but synced
//!   Prowlarr children run on Prowlarr's definitions, which Prowlarr keeps
//!   refreshing (D33). Comparing the two intervals would test an unrelated
//!   schedule.
//! - PRW-INDEXERS-5033..5037 (custom cookie-auth definition): Cardigann
//!   interpreter, tracker login and category table, none in Scryer.
//! - PRW-INDEXERS-5019 (`indexerproxy.json#id=1`, challenge-solver indexer
//!   proxy): Scryer has per-indexer challenge-solver proxies, and synced
//!   children inherit the Prowlarr parent's proxy (see 5029), which covers
//!   Scryer's hop to Prowlarr. Prowlarr's indexer proxy covers the hop from
//!   Prowlarr to the tracker, which Scryer never makes (D33). Observing the
//!   inheritance needs the AppUseCase sync workflow, more than a seam.

use scryer_application::JobKey;
use scryer_parity::{Catalogue, Suite};
use serde_json::json;

use super::{captured, list, split_ref};

#[test]
fn prw_indexers_sync_cadence() {
    let Some(mut suite) = Suite::open(
        "scryer-infrastructure-acquisition::parity_indexers::prw_indexers_sync_cadence",
    ) else {
        return;
    };
    for (_, entry) in Catalogue::load().entries {
        if !entry.id.starts_with("PRW-INDEXERS-")
            || split_ref(&entry.reference)
                != Some(("prowlarr", "system-task", "taskName=ApplicationIndexerSync"))
            || !suite.wants(&entry.id)
        {
            continue;
        }
        let tasks = list(&captured("prowlarr", "system-task"));
        let task = tasks
            .iter()
            .find(|t| t["taskName"] == "ApplicationIndexerSync")
            .expect("task in corpus");
        let actual = json!(JobKey::ProwlarrSync.interval_seconds().map(|s| s / 60));
        suite.check(&entry.id, task["interval"].clone(), actual);
    }
    suite.finish();
}
