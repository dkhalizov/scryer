//! Parity net, torrent area (notes/11-parity-net-design.md, parity/README.md).
//!
//! Lives in scryer-infrastructure-acquisition because the config suites run
//! Scryer's own Prowlarr sync (indexers/providers/prowlarr.rs), which is this
//! crate's; the seed-goal resolver it feeds is scryer-application's.
//!
//! Suites:
//! - `scryer-infrastructure-acquisition::parity_torrent::son_torrent`:
//!   corpus/reference/sonarr/torrent/SeedConfigProviderFixture.json
//! - `...::rad_torrent`: corpus/reference/radarr/torrent/SeedConfigProviderFixture.json
//! - `...::son_torrent_config`, `rad_torrent_config`, `prw_torrent_config`:
//!   config entries whose ref is `corpus/captured/<app>/config/indexer.json#id=N/seedCriteria`,
//!   checked from that JSON.
//!
//! Scryer code under test:
//! - `SeedGoalResolver::resolve` (scryer-application acquisition/seed_goals.rs:167),
//!   the one answer to "what seed goals does this grab get". Its Prowlarr tier
//!   (`prowlarr_managed_goal_profile`, seed_goals.rs:408) reads the criteria a
//!   Prowlarr-managed child carries in `managed_metadata_json`; the comment
//!   there names Sonarr's SeedConfigProvider as the model.
//! - For the config suites, the criteria come from Scryer's own Prowlarr sync:
//!   `ProwlarrManagementClient::plan_sync` (prowlarr.rs:1463, plan built by
//!   build_managed_child_plan :1582) run against a loopback wiremock that
//!   serves the captured Prowlarr `system-status.json`, `indexer.json` and
//!   `appprofile.json` unchanged. Each child plan becomes an IndexerConfig the
//!   way the sync stores it (scryer-application
//!   integration/workflow/managed_indexers.rs:358-372: name, provider type,
//!   flags and managed metadata copied; parent id set).
//!
//! Translation table (reference -> Scryer):
//!
//! | Reference | Scryer |
//! |---|---|
//! | release `indexer_id` 0 (no indexer) | `SeedGoalRequest.indexer_id = None` |
//! | release `indexer_id` n | `indexer_id = Some("n")` |
//! | `indexer_settings: null` (no cached settings) | no IndexerConfig row for that id |
//! | indexer `seed_criteria` {seed_ratio, seed_time, season_pack_seed_time} | a Prowlarr-managed child (parent id set) with managed metadata {seed_ratio, seed_time_minutes, season_pack_seed_time_minutes} |
//! | `parsed.full_season` | `SeedGoalRequest.season_pack` |
//! | `seed_config: null` / not null | `ResolvedSeedGoals::is_resolved()` false / true |
//! | `seed_time_min` | `seed_goal_seconds / 60` |
//! | config `seedCriteria.seedRatio`, `.seedTime`, `.seasonPackSeedTime` (Sonarr, Radarr) | goals of the Prowlarr child of the same name without the " (Prowlarr)" suffix: `seed_goal_ratio`, `seed_goal_seconds / 60` for a single release, and for a season pack |
//! | config `torrentBaseSettings.seedRatio`, `.seedTime`, `.packSeedTime` (Prowlarr) | the same, for the child whose key is the Prowlarr indexer id; the season-pack time is `packSeedTime ?? seedTime`, what Prowlarr pushes to Sonarr (Prowlarr v2.7.0.5723 Applications/Sonarr/Sonarr.cs:285) |
//!
//! A season pack keeps the indexer's ratio in the reference
//! (Sonarr Indexers/SeedConfigProvider.cs:52-57: Ratio = SeedRatio for every
//! release, only the time switches), so the expected season-pack goal is
//! {ratio, season-pack time}. Radarr has no season packs: one goal.
//! "No seed goals (the client's own limits apply)" is ratio and time null.
//!
//! Ignored keys: release `protocol` (every case is Torrent; the resolver is
//! only called for torrent grabs), `indexer_settings.fail_downloads` (not a
//! seed setting).
//!
//! Unmapped (tests: []), with reasons. Sonarr (torrent.sonarr.yaml):
//! - QBittorrentFixture (SON-TORRENT-0001..0075): queue state mapping, import
//!   paths, add/magnet handling, removability by ratio/seeding/inactive limits,
//!   category and JSON parsing all happen inside the qBittorrent download
//!   client, which in Scryer is an out-of-tree WASM plugin (registry
//!   "qbittorrent", see scryer-plugins seeding_trust.rs:58). The host only
//!   sees its DownloadItemState and `can_remove`; the rules under test are not
//!   in this repo.
//! - CompletedDownloadService Import/Process (0077..0096): waived, D21
//!   duplicates of SON-IMPORT-0001..0020.
//! - FailedDownloadService (0097..0102), TrackedDownloadAlreadyImported
//!   (0103..0108), TrackedDownloadService (0109..0114), DownloadService
//!   (0115..0126): orchestration through AppUseCase (tracked downloads,
//!   history, grab dispatch, indexer backoff) with repositories and events;
//!   no in-memory seam, assertions are mock calls and published events.
//! - Radarr (torrent.radarr.yaml) RAD-TORRENT-0001..0005 (completed download
//!   import) and 0007..0012 (tracked downloads): as for Sonarr.
//!
//! Config entries unmapped:
//! - SON-TORRENT-5001..5004, RAD-TORRENT-5001..5005, RAD-TORRENT-5010
//!   (download client category, priorities, remove completed/failed, failed
//!   redownload, poll interval, RefreshMonitoredDownloads task): Scryer
//!   download-client routing settings and job schedules resolved through
//!   AppUseCase and its settings repository (settings/runtime/routing.rs:2);
//!   no pure function holds them.
//! - PRW-TORRENT-5002..5028 even/redirect entries (preferMagnetUrl, redirect):
//!   the link a grab uses is chosen by Prowlarr when it writes its feed; the
//!   captured config has no feed, so there is no Scryer input to compare.
//! - PRW-TORRENT-5019 (Prowlarr has no download client): Prowlarr-side; Scryer
//!   always grabs through its own clients.
//! - QBT-TORRENT-* (all, so no qbt_torrent_config test): qBittorrent's own
//!   preferences, categories, pod hooks, sidecars and CronJobs. Scryer does not
//!   configure or replace qBittorrent and has no field for any of them.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scryer_application::{
    AppError, AppResult, IndexerConfigRepository, IndexerConfigUpdate, IndexerManagementClient,
    ManagedIndexerChildPlan, NullIndexerErrorRepository, NullSettingsRepository,
    ResolvedSeedGoals, SeedGoalRequest, SeedGoalResolver, SeedingProfileRepository,
};
use scryer_domain::{IndexerConfig, SeedingProfile};
use scryer_infrastructure_acquisition::indexers::providers::prowlarr::ProwlarrManagementClient;
use scryer_parity::{Catalogue, Suite};
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PARENT_ID: &str = "prowlarr";

struct Indexers(Vec<IndexerConfig>);

#[async_trait]
impl IndexerConfigRepository for Indexers {
    async fn list(&self, _provider_type: Option<String>) -> AppResult<Vec<IndexerConfig>> {
        Ok(self.0.clone())
    }
    async fn get_by_id(&self, id: &str) -> AppResult<Option<IndexerConfig>> {
        Ok(self.0.iter().find(|i| i.id == id).cloned())
    }
    async fn create(&self, config: IndexerConfig) -> AppResult<IndexerConfig> {
        Ok(config)
    }
    async fn touch_last_error(&self, _id: &str) -> AppResult<()> {
        Ok(())
    }
    async fn update(&self, _update: IndexerConfigUpdate) -> AppResult<IndexerConfig> {
        Err(AppError::Repository("read-only test repository".into()))
    }
    async fn delete(&self, _id: &str) -> AppResult<()> {
        Ok(())
    }
}

/// No seeding profiles and (NullSettingsRepository) no default profile: the
/// indexer's own criteria are the only source, as in the reference.
struct NoProfiles;

#[async_trait]
impl SeedingProfileRepository for NoProfiles {
    async fn list(&self) -> AppResult<Vec<SeedingProfile>> {
        Ok(Vec::new())
    }
    async fn get_by_id(&self, _id: &str) -> AppResult<Option<SeedingProfile>> {
        Ok(None)
    }
    async fn create(&self, profile: SeedingProfile) -> AppResult<SeedingProfile> {
        Ok(profile)
    }
    async fn update(&self, profile: SeedingProfile) -> AppResult<SeedingProfile> {
        Ok(profile)
    }
    async fn delete(&self, _id: &str) -> AppResult<()> {
        Ok(())
    }
}

fn resolver(indexers: Vec<IndexerConfig>) -> SeedGoalResolver {
    SeedGoalResolver::new(
        Arc::new(NoProfiles),
        Some(Arc::new(Indexers(indexers))),
        Arc::new(NullSettingsRepository),
    )
}

fn indexer(id: &str, name: &str, provider_type: &str) -> IndexerConfig {
    let epoch: DateTime<Utc> = DateTime::from_timestamp(0, 0).expect("epoch");
    IndexerConfig {
        id: id.to_string(),
        name: name.to_string(),
        provider_type: provider_type.to_string(),
        base_url: String::new(),
        api_key_encrypted: None,
        rate_limit_seconds: None,
        rate_limit_burst: None,
        max_queries_per_minute: None,
        disabled_until: None,
        is_enabled: true,
        enable_interactive_search: true,
        enable_auto_search: true,
        proxy_config_id: None,
        download_client_id: None,
        seeding_profile_id: None,
        managed_parent_config_id: None,
        managed_child_key: None,
        managed_metadata_json: None,
        caps_snapshot_json: None,
        last_health_status: None,
        last_error_message: None,
        last_error_code: None,
        last_error_at: None,
        config_json: None,
        created_at: epoch,
        updated_at: epoch,
    }
}

fn child_id(child_key: &str) -> String {
    format!("{PARENT_ID}-child-{child_key}")
}

/// A synced child as the sync stores it (managed_indexers.rs:358-372).
fn child_config(child: &ManagedIndexerChildPlan) -> IndexerConfig {
    let mut config = indexer(&child_id(&child.child_key), &child.name, &child.provider_type);
    config.is_enabled = child.is_enabled;
    config.enable_interactive_search = child.enable_interactive_search;
    config.enable_auto_search = child.enable_auto_search;
    config.managed_parent_config_id = Some(PARENT_ID.to_string());
    config.managed_child_key = Some(child.child_key.clone());
    config.managed_metadata_json = child.managed_metadata_json.clone();
    config.caps_snapshot_json = child.caps_snapshot_json.clone();
    config
}

/// Scryer's Prowlarr sync over the captured Prowlarr API answers.
async fn prowlarr_children() -> Vec<ManagedIndexerChildPlan> {
    let server = MockServer::start().await;
    for (route, file) in [
        ("/api/v1/system/status", "system-status"),
        ("/api/v1/indexer", "indexer"),
        ("/api/v1/appprofile", "appprofile"),
    ] {
        let body_path = scryer_parity::parity_dir()
            .join(format!("corpus/captured/prowlarr/config/{file}.json"));
        let mut body = std::fs::read_to_string(&body_path)
            .unwrap_or_else(|e| panic!("{}: {e}", body_path.display()));
        if file == "system-status" {
            // The capture keeps only `version` (capture/prowlarr.py:69); the
            // sync's identity check (prowlarr.rs resolve_status) needs the
            // appName every Prowlarr reports. The captured server is Prowlarr.
            let mut status: Value = serde_json::from_str(&body).expect("system-status");
            status["appName"] = json!("Prowlarr");
            body = status.to_string();
        }
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .mount(&server)
            .await;
    }
    let mut parent = indexer(PARENT_ID, "Prowlarr", "prowlarr");
    parent.base_url = server.uri();
    parent.config_json = Some(json!({ "base_url": server.uri(), "api_key": "parity" }).to_string());
    let client = ProwlarrManagementClient::new_with_indexer_error_repository(
        &parent,
        Arc::new(NullIndexerErrorRepository),
    );
    client
        .plan_sync(PARENT_ID)
        .await
        .expect("Prowlarr sync plan")
        .children
}

async fn goals(resolver: &SeedGoalResolver, indexer_id: Option<String>, season_pack: bool) -> ResolvedSeedGoals {
    resolver
        .resolve(&SeedGoalRequest {
            indexer_id,
            season_pack,
            ..SeedGoalRequest::default()
        })
        .await
        .expect("resolve seed goals")
}

fn goal_json(goals: &ResolvedSeedGoals) -> Value {
    json!({ "ratio": goals.seed_goal_ratio, "seed_time_min": goals.seed_goal_seconds.map(|s| s / 60) })
}

/// The corpus `expect` conventions used here: `{"not": v}` and partial objects
/// (only the keys the reference asserts are compared).
fn matches(expect: &Value, actual: &Value) -> bool {
    match expect {
        Value::Object(map) if map.len() == 1 && map.contains_key("not") => actual != &map["not"],
        Value::Object(map) => map
            .iter()
            .all(|(k, v)| matches(v, actual.get(k).unwrap_or(&Value::Null))),
        _ => expect == actual,
    }
}

/// SeedConfigProviderFixture cases (op `seed_config.get`).
async fn seed_config_cases(suite: &mut Suite, file: &str) {
    let cases: Vec<Value> = scryer_parity::corpus(file);
    for case in &cases {
        let id = case["id"].as_str().expect("case id");
        if !suite.wants(id) || case["input"]["op"] != "seed_config.get" {
            continue;
        }
        let given = &case["input"]["given"];
        let indexer_id = given["release"]["indexer_id"].as_i64().expect("indexer id");
        let mut indexers = Vec::new();
        if let Some(criteria) = given["indexer_settings"].get("seed_criteria") {
            let mut child = indexer(&indexer_id.to_string(), "Torznab", "torznab");
            child.managed_parent_config_id = Some(PARENT_ID.to_string());
            child.managed_metadata_json = Some(
                json!({
                    "seed_ratio": criteria.get("seed_ratio"),
                    "seed_time_minutes": criteria.get("seed_time"),
                    "season_pack_seed_time_minutes": criteria.get("season_pack_seed_time"),
                })
                .to_string(),
            );
            indexers.push(child);
        }
        let season_pack = given["parsed"]["full_season"].as_bool().unwrap_or(false);
        let request_id = (indexer_id != 0).then(|| indexer_id.to_string());
        let resolved = goals(&resolver(indexers), request_id, season_pack).await;
        let actual = json!({
            "seed_config": if resolved.is_resolved() { goal_json(&resolved) } else { Value::Null },
            "seed_time_min": resolved.seed_goal_seconds.map(|s| s / 60),
        });
        let expect = &case["expect"];
        let pass = matches(expect, &actual);
        let detail = if pass { String::new() } else { format!("expected {expect}, got {actual}") };
        suite.record(id, pass, detail);
    }
}

#[tokio::test]
async fn son_torrent() {
    let Some(mut suite) = Suite::open("scryer-infrastructure-acquisition::parity_torrent::son_torrent") else { return };
    seed_config_cases(&mut suite, "corpus/reference/sonarr/torrent/SeedConfigProviderFixture.json").await;
    suite.finish();
}

#[tokio::test]
async fn rad_torrent() {
    let Some(mut suite) = Suite::open("scryer-infrastructure-acquisition::parity_torrent::rad_torrent") else { return };
    seed_config_cases(&mut suite, "corpus/reference/radarr/torrent/SeedConfigProviderFixture.json").await;
    suite.finish();
}

/// `corpus/captured/<app>/config/<name>.json#<loc>` -> (app, name, loc).
fn split_ref(reference: &str) -> Option<(&str, &str, &str)> {
    let (file, loc) = reference.split_once('#')?;
    let (app, name) = file.strip_prefix("corpus/captured/")?.split_once("/config/")?;
    Some((app, name.strip_suffix(".json")?, loc))
}

fn captured(app: &str, name: &str) -> Value {
    scryer_parity::corpus(&format!("corpus/captured/{app}/config/{name}.json"))
}

fn by_id(list: &Value, id: &str) -> Option<Value> {
    list.as_array()?.iter().find(|x| x["id"].to_string() == id).cloned()
}

fn fields(x: &Value) -> BTreeMap<String, Value> {
    x["fields"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| {
            (
                f["name"].as_str().unwrap_or_default().to_string(),
                f.get("value").cloned().unwrap_or(Value::Null),
            )
        })
        .collect()
}

struct Synced {
    children: Vec<ManagedIndexerChildPlan>,
    resolver: SeedGoalResolver,
}

impl Synced {
    async fn load() -> Self {
        let children = prowlarr_children().await;
        let resolver = resolver(children.iter().map(child_config).collect());
        Self { children, resolver }
    }

    async fn goals(&self, child: Option<&ManagedIndexerChildPlan>, season_pack: bool) -> Value {
        match child {
            Some(c) => goal_json(&goals(&self.resolver, Some(child_id(&c.child_key)), season_pack).await),
            None => Value::Null,
        }
    }
}

/// `indexer.json#id=N/seedCriteria`: the goals the reference attaches to grabs
/// from indexer N, against Scryer's goals for the matching synced child.
async fn seed_check(synced: &Synced, app: &str, loc: &str) -> Option<(Value, Value)> {
    let id = loc.strip_prefix("id=")?.strip_suffix("/seedCriteria")?;
    let entry = by_id(&captured(app, "indexer"), id)?;
    let f = fields(&entry);
    let (ratio, time, pack, child) = if app == "prowlarr" {
        (
            "torrentBaseSettings.seedRatio",
            "torrentBaseSettings.seedTime",
            "torrentBaseSettings.packSeedTime",
            synced.children.iter().find(|c| c.child_key == id),
        )
    } else {
        let name = entry["name"].as_str()?.strip_suffix(" (Prowlarr)")?;
        (
            "seedCriteria.seedRatio",
            "seedCriteria.seedTime",
            "seedCriteria.seasonPackSeedTime",
            synced.children.iter().find(|c| c.name == name),
        )
    };
    let minutes = |key: &str| f.get(key).and_then(Value::as_i64);
    let goal = |seed_time_min: Option<i64>| {
        json!({ "ratio": f.get(ratio).and_then(Value::as_f64), "seed_time_min": seed_time_min })
    };
    // Prowlarr pushes Sonarr seasonPackSeedTime = PackSeedTime ?? SeedTime
    // (Prowlarr Applications/Sonarr/Sonarr.cs:285); Sonarr's own field is taken as is.
    let pack_min = if app == "prowlarr" { minutes(pack).or(minutes(time)) } else { minutes(pack) };
    let mut expected = json!({ "single": goal(minutes(time)) });
    let mut actual = json!({ "single": synced.goals(child, false).await });
    if app != "radarr" {
        expected["season_pack"] = goal(pack_min);
        actual["season_pack"] = synced.goals(child, true).await;
    }
    Some((expected, actual))
}

async fn run_config(suite: &mut Suite, prefix: &str) {
    let synced = Synced::load().await;
    for (_, entry) in Catalogue::load().entries {
        if !entry.id.starts_with(prefix) || !suite.wants(&entry.id) {
            continue;
        }
        let Some((app, "indexer", loc)) = split_ref(&entry.reference) else {
            continue;
        };
        if let Some((expected, actual)) = seed_check(&synced, app, loc).await {
            suite.check(&entry.id, expected, actual);
        }
    }
}

#[tokio::test]
async fn son_torrent_config() {
    let Some(mut suite) = Suite::open("scryer-infrastructure-acquisition::parity_torrent::son_torrent_config") else { return };
    run_config(&mut suite, "SON-TORRENT-").await;
    suite.finish();
}

#[tokio::test]
async fn rad_torrent_config() {
    let Some(mut suite) = Suite::open("scryer-infrastructure-acquisition::parity_torrent::rad_torrent_config") else { return };
    run_config(&mut suite, "RAD-TORRENT-").await;
    suite.finish();
}

#[tokio::test]
async fn prw_torrent_config() {
    let Some(mut suite) = Suite::open("scryer-infrastructure-acquisition::parity_torrent::prw_torrent_config") else { return };
    run_config(&mut suite, "PRW-TORRENT-").await;
    suite.finish();
}
