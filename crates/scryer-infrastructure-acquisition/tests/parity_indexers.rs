//! Parity net, indexers area (notes/11-parity-net-design.md, parity/README.md).
//!
//! Lives in scryer-infrastructure-acquisition because it runs Scryer's own
//! Prowlarr sync (indexers/providers/prowlarr.rs), which is this crate's.
//!
//! Suites (config entries, each checked from the captured JSON its ref names):
//! - `scryer-infrastructure-acquisition::parity_indexers::prw_indexers_config`
//! - `...::son_indexers_config`, `rad_indexers_config`
//! - `...::prw_indexers_caps_categories` (submodule indexers.rs)
//! - `...::prw_indexers_sync_cadence` (submodule priv_indexers.rs)
//!
//! Scryer code under test:
//! - `ProwlarrManagementClient::plan_sync` (prowlarr.rs:1463, children built by
//!   build_managed_child_plan :1582: flags :1592-1602 and :1672, routing scopes
//!   collect_routing_categories :1747 / classify_scope :1777, seed and seeder
//!   metadata :1643-1665), run against a loopback wiremock serving the
//!   captured Prowlarr `system-status.json`, `indexer.json` and
//!   `appprofile.json` unchanged. A child plan's routing scopes are the
//!   categories Scryer searches that indexer with per scope (the sync seeds
//!   them as routing entries, scryer-application
//!   integration/workflow/managed_indexers.rs:30).
//! - `SeedGoalResolver::resolve_minimum_seeders` (scryer-application
//!   acquisition/seed_goals.rs:233) on each child stored as the sync stores it
//!   (managed_indexers.rs:358-372), for the seeder floor a grab must clear.
//! - `parse_rss_target_interval(None)` / `parse_rss_sync_tick(None)`
//!   (acquisition/rss.rs:189 / :160): the shipped RSS cadence and worker tick,
//!   with no environment override.
//!
//! Translation table (reference -> Scryer):
//!
//! | Reference | Scryer |
//! |---|---|
//! | Sonarr/Radarr indexer 'X (Prowlarr)' | the synced child named 'X' |
//! | Prowlarr indexer id N | the synced child with key "N" |
//! | `protocol` torrent / usenet | child `provider_type` torznab / newznab |
//! | `enableRss`, `enableAutomaticSearch` | child `enable_auto_search` (one flag for RSS and automatic search, prowlarr.rs:1672) |
//! | `enableInteractiveSearch` | child `enable_interactive_search` |
//! | `enable` (Prowlarr) | child `is_enabled` |
//! | `priority` | none derived from it, compared as null, so these cases fail. Scryer does have a per-scope indexer routing priority (`IndexerRoutingSettingsEntry.priority`, scryer-application settings/runtime/routing.rs:17), but the sync seeds it as an ordinal, max + 1 in sync order, and after that it belongs to the operator (integration/workflow/indexers.rs:136-160, next_indexer_routing_priority / upsert_indexer_routing_entry); it is never taken from Prowlarr's priority. Prowlarr's priority reaches Scryer only in the child's managed metadata (prowlarr.rs:1636), which nothing reads for search or decisions (types.rs:2860 keeps only enable_automatic_search of it) |
//! | Radarr `categories` / offered to Radarr | child routing scope `movie` |
//! | Sonarr `categories` / offered to Sonarr | child routing scope `series` |
//! | Sonarr `animeCategories` / anime categories | child routing scope `anime` |
//! | `minimumSeeders`, app profile minimum, `appMinimumSeeders` | `resolve_minimum_seeders(child)` |
//! | app 'Radarr' receives indexer | child enabled with a `movie` scope |
//! | app 'Sonarr' receives indexer | child enabled with a `series` or `anime` scope |
//! | `rssSyncInterval` (min) | `parse_rss_target_interval(None)` in minutes |
//! | task `RssSync` interval (min) | `parse_rss_sync_tick(None)` in minutes |
//!
//! What Prowlarr offers an app is computed from the captured JSON the way
//! parity/capture/prowlarr.py `pushed` does (Prowlarr
//! IndexerCapabilitiesCategories.cs:124 SupportedCategories, Applications/
//! Sonarr/Sonarr.cs:125, Radarr/Radarr.cs:124): the caps tree and its first
//! level of subcategories, kept when in the app's sync categories, for an
//! indexer with a search mode and a matching tag. Category lists are compared
//! as sorted sets (the reference order is the caps tree order, not behaviour).
//!
//! Ignored keys:
//! - The display name suffix " (Prowlarr)": the label the app shows; the
//!   indexer is matched by it, not compared.
//! - Prowlarr app `enabled`, `sync level`, `tags`, anime standard-format
//!   search; indexer definition, privacy, language, tags, query/grab limits:
//!   settings of Prowlarr's own sync and search engine, which stay in
//!   Prowlarr (Scryer pulls the result).
//! - Facets Scryer has no concept for, while our value is the off state:
//!   Radarr `requiredFlags`, `multiLanguages`, `failDownloads`, `removeYear`,
//!   and `rejectBlocklistedTorrentHashesWhileGrabbing` (both apps). Absent is
//!   the same behaviour as off. If a capture turns one on, it is added to the
//!   expected value and compared against null, so the case fails.
//! - Sonarr `additionalParameters` (null), `animeStandardFormatSearch`: not in
//!   the catalogue behaviour.
//!
//! Unmapped (tests: []), with reasons:
//! - PRW-INDEXERS-0001..0003 are now mapped in parity_indexers/indexers.rs
//!   (prw_indexers_caps_categories); 0004..0006 stay unmapped (tracker-category
//!   table Scryer lacks, see the indexers.rs header).
//! - PRW-INDEXERS titleRewrite (5006, 5009, 5011, 5014), settings (5020..5028),
//!   host (5029, 5030), task 5032, cookie-auth definition (5033..5037),
//!   challenge-solver proxy (5019): Prowlarr's own engine and host, which stay
//!   in Prowlarr. Reasons per group are in the parity_indexers/priv_indexers.rs
//!   header. (Scryer does have caps limits, per-indexer proxies and challenge
//!   solvers; the hop they cover is Prowlarr's, not Scryer's.)
//! - PRW-INDEXERS-5031 (Prowlarr's ApplicationIndexerSync interval) is now
//!   mapped in parity_indexers/priv_indexers.rs (prw_indexers_sync_cadence),
//!   against the public `JobKey::ProwlarrSync.interval_seconds()`.
//! - SON-INDEXERS / RAD-INDEXERS: none beyond the ignored keys above.
//!
//! Notify (no parity_notify test file): SON-NOTIFY-5001, RAD-NOTIFY-5001,
//! PRW-NOTIFY-5001 (no connection, nothing sent) and BAZ-NOTIFY-5001..5003,
//! 5006 (rescan / Plex refresh after a subtitle download, no providers) need
//! the notification dispatcher (scryer-application
//! notifications/dispatcher.rs:103, takes an AppUseCase) or the subtitle
//! workflow; no seam without one. BAZ-NOTIFY-5004, 5005 and SER-NOTIFY-5001
//! are in_scope: false.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scryer_application::{
    AppError, AppResult, IndexerConfigRepository, IndexerConfigUpdate, IndexerManagementClient,
    ManagedIndexerChildPlan, NullIndexerErrorRepository, NullSettingsRepository,
    SeedGoalResolver, SeedingProfileRepository, parse_rss_sync_tick, parse_rss_target_interval,
};
use scryer_domain::{IndexerConfig, SeedingProfile};
use scryer_infrastructure_acquisition::indexers::providers::prowlarr::ProwlarrManagementClient;
use scryer_parity::{Catalogue, Suite};
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[path = "parity_indexers/indexers.rs"]
mod indexers;
#[path = "parity_indexers/priv_indexers.rs"]
mod priv_indexers;

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

/// No seeding profiles and (NullSettingsRepository) no default profile or
/// floor override: the synced child's own metadata decides.
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

struct Synced {
    children: Vec<ManagedIndexerChildPlan>,
    resolver: SeedGoalResolver,
}

impl Synced {
    async fn load() -> Self {
        let children = prowlarr_children().await;
        let resolver = SeedGoalResolver::new(
            Arc::new(NoProfiles),
            Some(Arc::new(Indexers(children.iter().map(child_config).collect()))),
            Arc::new(NullSettingsRepository),
        );
        Self { children, resolver }
    }

    fn by_key(&self, key: &str) -> Option<&ManagedIndexerChildPlan> {
        self.children.iter().find(|c| c.child_key == key)
    }

    fn by_name(&self, name: &str) -> Option<&ManagedIndexerChildPlan> {
        self.children.iter().find(|c| c.name == name)
    }

    async fn min_seeders(&self, child: &ManagedIndexerChildPlan) -> i32 {
        self.resolver
            .resolve_minimum_seeders(Some(&child_id(&child.child_key)))
            .await
            .expect("minimum seeders")
    }
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

fn list(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

fn by_id(v: &Value, id: &str) -> Option<Value> {
    list(v).into_iter().find(|x| x["id"].to_string() == id)
}

fn fields(x: &Value) -> BTreeMap<String, Value> {
    list(&x["fields"])
        .into_iter()
        .map(|f| {
            let name = f["name"].as_str().unwrap_or_default().to_string();
            (name, f.get("value").cloned().unwrap_or(Value::Null))
        })
        .collect()
}

fn ints(v: Option<&Value>) -> Vec<i64> {
    let mut out: Vec<i64> = v.map(list).unwrap_or_default().iter().filter_map(Value::as_i64).collect();
    out.sort_unstable();
    out
}

fn protocol(provider_type: &str) -> &str {
    match provider_type {
        "torznab" => "torrent",
        "newznab" => "usenet",
        other => other,
    }
}

fn scope(child: &ManagedIndexerChildPlan, scope_id: &str) -> Option<Vec<i64>> {
    let found = child.routing_scopes.iter().find(|s| s.scope_id == scope_id)?;
    let mut out: Vec<i64> = found
        .categories
        .iter()
        .map(|c| c.parse().unwrap_or_else(|_| panic!("non-numeric category {c}")))
        .collect();
    out.sort_unstable();
    Some(out)
}

/// The scopes standing for one app (translation table).
fn app_scopes(app: &str) -> &'static [&'static str] {
    if app == "sonarr" { &["series", "anime"] } else { &["movie"] }
}

/// Whether Scryer searches this child for the app's media.
fn scryer_uses(child: Option<&ManagedIndexerChildPlan>, app: &str) -> bool {
    child.is_some_and(|c| c.is_enabled && app_scopes(app).iter().any(|s| scope(c, s).is_some()))
}

/// Prowlarr IndexerCapabilitiesCategories.SupportedCategories: the caps tree
/// and its first level of subcategories, kept when wanted.
fn supported(caps: &Value, wanted: &[i64]) -> Vec<i64> {
    let tree = list(&caps["categories"]);
    let subs: Vec<Value> = tree.iter().flat_map(|c| list(&c["subCategories"])).collect();
    tree.iter()
        .chain(subs.iter())
        .filter_map(|c| c["id"].as_i64())
        .filter(|id| wanted.contains(id))
        .collect()
}

/// What Prowlarr's sync sends one app for one indexer; None when skipped.
fn pushed(indexer: &Value, app: &Value) -> Option<(Vec<i64>, Vec<i64>)> {
    let caps = &indexer["capabilities"];
    let f = fields(app);
    let mode = if app["implementation"] == "Sonarr" { "tvSearchParams" } else { "movieSearchParams" };
    let search = !list(&caps["searchParams"]).is_empty() || !list(&caps[mode]).is_empty();
    let app_tags = list(&app["tags"]);
    let tagged = app_tags.is_empty() || app_tags.iter().any(|t| list(&indexer["tags"]).contains(t));
    let mut cats = supported(caps, &ints(f.get("syncCategories")));
    let mut anime = supported(caps, &ints(f.get("animeSyncCategories")));
    cats.sort_unstable();
    anime.sort_unstable();
    (search && tagged && (!cats.is_empty() || !anime.is_empty())).then_some((cats, anime))
}

fn app_key(app: &Value) -> String {
    app["implementation"].as_str().unwrap_or_default().to_ascii_lowercase()
}

/// `appprofile.json#id=N`: every indexer on the profile syncs with its flags
/// and seeder floor.
async fn app_profile_check(s: &Synced, loc: &str) -> Option<(Value, Value)> {
    let id = loc.strip_prefix("id=")?;
    let profile = by_id(&captured("prowlarr", "appprofile"), id)?;
    let (mut expected, mut actual) = (Vec::new(), Vec::new());
    for ix in list(&captured("prowlarr", "indexer")) {
        if ix["appProfileId"].to_string() != id {
            continue;
        }
        let own = fields(&ix).get("torrentBaseSettings.appMinimumSeeders").and_then(Value::as_i64);
        expected.push(json!({
            "indexer": ix["name"],
            "rss": profile["enableRss"],
            "automatic_search": profile["enableAutomaticSearch"],
            "interactive_search": profile["enableInteractiveSearch"],
            "min_seeders": own.or(profile["minimumSeeders"].as_i64()),
        }));
        actual.push(match s.by_key(&ix["id"].to_string()) {
            Some(c) => json!({
                "indexer": c.name,
                "rss": c.enable_auto_search,
                "automatic_search": c.enable_auto_search,
                "interactive_search": c.enable_interactive_search,
                "min_seeders": s.min_seeders(c).await,
            }),
            None => json!({ "indexer": ix["name"], "synced": false }),
        });
    }
    Some((json!(expected), json!(actual)))
}

/// `applications.json#id=N`: the set of indexers the app receives.
fn application_check(s: &Synced, loc: &str) -> Option<(Value, Value)> {
    let app = by_id(&captured("prowlarr", "applications"), loc.strip_prefix("id=")?)?;
    let key = app_key(&app);
    let mut expected: Vec<String> = list(&captured("prowlarr", "indexer"))
        .iter()
        .filter(|ix| pushed(ix, &app).is_some())
        .filter_map(|ix| ix["name"].as_str().map(str::to_string))
        .collect();
    let mut actual: Vec<String> = s
        .children
        .iter()
        .filter(|c| scryer_uses(Some(*c), &key))
        .map(|c| c.name.clone())
        .collect();
    expected.sort();
    actual.sort();
    Some((json!(expected), json!(actual)))
}

/// Prowlarr `indexer.json#id=N`: protocol, priority, enabled, and what each
/// app is offered.
fn prowlarr_indexer_check(s: &Synced, loc: &str) -> Option<(Value, Value)> {
    let id = loc.strip_prefix("id=")?;
    if id.contains('/') {
        return None;
    }
    let ix = by_id(&captured("prowlarr", "indexer"), id)?;
    let child = s.by_key(id);
    let mut expected = json!({ "protocol": ix["protocol"], "priority": ix["priority"], "enabled": ix["enable"] });
    let mut actual = match child {
        Some(c) => json!({ "protocol": protocol(&c.provider_type), "priority": Value::Null, "enabled": c.is_enabled }),
        None => json!({ "protocol": Value::Null, "priority": Value::Null, "enabled": Value::Null }),
    };
    for app in list(&captured("prowlarr", "applications")) {
        let key = app_key(&app);
        expected[key.as_str()] = match pushed(&ix, &app) {
            None => Value::Null,
            Some((cats, _)) if key == "radarr" => json!({ "categories": cats }),
            Some((cats, anime)) => json!({ "categories": cats, "anime": anime }),
        };
        actual[key.as_str()] = match child {
            Some(c) if scryer_uses(Some(c), &key) && key == "radarr" => {
                json!({ "categories": scope(c, "movie").unwrap_or_default() })
            }
            Some(c) if scryer_uses(Some(c), &key) => json!({
                "categories": scope(c, "series").unwrap_or_default(),
                "anime": scope(c, "anime").unwrap_or_default(),
            }),
            _ => Value::Null,
        };
    }
    Some((expected, actual))
}

/// `<app>/config/indexer.json#absent=X (Prowlarr)`: whether the app ends up
/// using the indexer Prowlarr offered it.
fn absent_check(s: &Synced, app: &str, loc: &str) -> Option<(Value, Value)> {
    let name = loc.strip_prefix("absent=")?;
    let present = list(&captured(app, "indexer")).iter().any(|x| x["name"] == name);
    let child = s.by_name(name.strip_suffix(" (Prowlarr)")?);
    Some((json!({ "used": present }), json!({ "used": scryer_uses(child, app) })))
}

/// Facets Scryer has no concept for: expected only when not off (header).
fn is_on(v: &Value) -> bool {
    match v {
        Value::Null | Value::Bool(false) => false,
        Value::Array(a) => !a.is_empty(),
        Value::String(t) => !t.is_empty(),
        _ => true,
    }
}

/// Sonarr/Radarr `indexer.json#id=N`: how the app uses the indexer.
async fn app_indexer_check(s: &Synced, app: &str, loc: &str) -> Option<(Value, Value)> {
    let id = loc.strip_prefix("id=")?;
    let ix = by_id(&captured(app, "indexer"), id)?;
    let f = fields(&ix);
    let mut expected = json!({
        "protocol": ix["protocol"],
        "priority": ix["priority"],
        "rss": ix["enableRss"],
        "automatic_search": ix["enableAutomaticSearch"],
        "interactive_search": ix["enableInteractiveSearch"],
        "categories": ints(f.get("categories")),
        "min_seeders": f.get("minimumSeeders").cloned().unwrap_or(Value::Null),
    });
    if app == "sonarr" {
        expected["anime_categories"] = json!(ints(f.get("animeCategories")));
    }
    for key in ["requiredFlags", "multiLanguages", "failDownloads", "removeYear", "rejectBlocklistedTorrentHashesWhileGrabbing"] {
        if let Some(v) = f.get(key).filter(|v| is_on(v)) {
            expected[key] = v.clone();
        }
    }
    let Some(c) = ix["name"].as_str().and_then(|n| n.strip_suffix(" (Prowlarr)")).and_then(|n| s.by_name(n)) else {
        return Some((expected, Value::Null));
    };
    let mut actual = json!({
        "protocol": protocol(&c.provider_type),
        "priority": Value::Null,
        "rss": c.enable_auto_search,
        "automatic_search": c.enable_auto_search,
        "interactive_search": c.enable_interactive_search,
        "categories": scope(c, if app == "sonarr" { "series" } else { "movie" }).unwrap_or_default(),
        "min_seeders": s.min_seeders(c).await,
    });
    if app == "sonarr" {
        actual["anime_categories"] = json!(scope(c, "anime").unwrap_or_default());
    }
    Some((expected, actual))
}

fn minutes(d: std::time::Duration) -> Value {
    json!(d.as_secs() / 60)
}

async fn check(s: &Synced, reference: &str) -> Option<(Value, Value)> {
    let (app, name, loc) = split_ref(reference)?;
    match (app, name) {
        ("prowlarr", "appprofile") => app_profile_check(s, loc).await,
        ("prowlarr", "applications") => application_check(s, loc),
        ("prowlarr", "indexer") => prowlarr_indexer_check(s, loc),
        (_, "indexer") if loc.starts_with("absent=") => absent_check(s, app, loc),
        (_, "indexer") => app_indexer_check(s, app, loc).await,
        (_, "indexerconfig") if loc == "rssSyncInterval" => Some((
            captured(app, name)["rssSyncInterval"].clone(),
            minutes(parse_rss_target_interval(None)),
        )),
        (_, "system-task") if loc == "taskName=RssSync" => {
            let task = list(&captured(app, name)).into_iter().find(|t| t["taskName"] == "RssSync")?;
            Some((task["interval"].clone(), minutes(parse_rss_sync_tick(None))))
        }
        _ => None,
    }
}

async fn run_config(suite: &mut Suite, prefix: &str) {
    let synced = Synced::load().await;
    for (_, entry) in Catalogue::load().entries {
        if !entry.id.starts_with(prefix) || !suite.wants(&entry.id) {
            continue;
        }
        if let Some((expected, actual)) = check(&synced, &entry.reference).await {
            suite.check(&entry.id, expected, actual);
        }
    }
}

#[tokio::test]
async fn prw_indexers_config() {
    let Some(mut suite) = Suite::open("scryer-infrastructure-acquisition::parity_indexers::prw_indexers_config") else { return };
    run_config(&mut suite, "PRW-INDEXERS-").await;
    suite.finish();
}

#[tokio::test]
async fn son_indexers_config() {
    let Some(mut suite) = Suite::open("scryer-infrastructure-acquisition::parity_indexers::son_indexers_config") else { return };
    run_config(&mut suite, "SON-INDEXERS-").await;
    suite.finish();
}

#[tokio::test]
async fn rad_indexers_config() {
    let Some(mut suite) = Suite::open("scryer-infrastructure-acquisition::parity_indexers::rad_indexers_config") else { return };
    run_config(&mut suite, "RAD-INDEXERS-").await;
    suite.finish();
}
