//! Parity net, flow area (notes/11-parity-net-design.md § Flow layer, P45).
//!
//! One check per `STK-FLOW-<n>` ID, each one assertion on chain state after a
//! step. The real app runs in-process (`common::TestContext`, temp SQLite);
//! the F2 fakes stand in for Torznab (HTTP), qBittorrent and Plex (ports).
//! Jobs are called directly, never waited on. Media files are tiny stubs or
//! the committed one-second import fixtures. A check that cannot be driven
//! yet fails with `SEAM: <what is missing>`.
// fork (P45)
#![recursion_limit = "256"]

mod common;
mod flow_fakes_support;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::TestContext;
use flow_fakes_support::qbit::{FakeQbit, FakeQbitProvider};
use scryer_application::DownloadClientPluginProvider;
use scryer_infrastructure_acquisition::downloads::{clients::PrioritizedDownloadClientRouter, config_store::DownloadClientConfigStore};
use scryer_application::testing::AppUseCaseTestExt;
use scryer_application::{
    AppUseCase, BlocklistRepository, DownloadClientConfigRepository, DownloadQueuePollerOptions, IndexerConfigRepository,
    DOWNLOAD_CLIENT_ROUTING_SETTINGS_KEY, LibraryRepository, LibraryRootDraft, MediaFileRepository,
    RECYCLE_BIN_ENABLED_KEY, SETTINGS_SCOPE_MEDIA, SaveQualityProfileSettings, SeedingProfileRepository,
    SettingsRepository,
    SubmissionConflictPolicy, import_completed_download, start_download_queue_poller_with_options,
};
use scryer_domain::{ExternalId, IndexerConfig, MediaFacet, NewTitle, Title, User};
use scryer_infrastructure_acquisition::downloads::seeding_profile_store::SeedingProfileStore;
use scryer_infrastructure_acquisition::proxy_config_store::ProxyConfigStore;
use scryer_infrastructure_acquisition::indexers::{artifact_resolver::AcquisitionIndexerArtifactResolver, config_store::IndexerConfigStore};
use scryer_infrastructure_sql::types::SettingDefinitionSeed;
use scryer_mock_apis::torznab::TorznabFake;
use scryer_infrastructure_workflow::workflow::{file_importer::FsFileImporter, stores::ImportStore};
use scryer_parity::Suite;

type Outcome = Result<(), String>;

fn seam(what: &str) -> Outcome {
    Err(format!("SEAM: {what}"))
}

fn ensure(cond: bool, msg: impl FnOnce() -> String) -> Outcome {
    if cond { Ok(()) } else { Err(msg()) }
}

/// The app wired to the fakes, plus a temp media tree.
struct Flow {
    ctx: TestContext,
    app: AppUseCase,
    user: User,
    qbit: FakeQbit,
    media: tempfile::TempDir,
}

impl Flow {
    async fn new() -> Self {
        let ctx = TestContext::new().await;
        ctx.settings_store
            .batch_ensure_setting_definitions(vec![
                seed("quality.profiles", "json", "[]"),
                seed("quality.profile_id", "string", "\"1080p\""),
                seed(DOWNLOAD_CLIENT_ROUTING_SETTINGS_KEY, "json", "{}"),
                SettingDefinitionSeed {
                    scope: SETTINGS_SCOPE_MEDIA.into(),
                    ..seed(RECYCLE_BIN_ENABLED_KEY, "boolean", "true")
                },
            ])
            .await
            .expect("seed quality settings");
        // our captured config: Radarr's download client files grabs under
        // `radarr`, removes completed and failed downloads, and its media
        // management has the recycle bin off
        for (scope, key, id, value) in [
            (
                "system",
                DOWNLOAD_CLIENT_ROUTING_SETTINGS_KEY,
                Some("movie"),
                r#"{"flow-qbit":{"enabled":true,"category":"radarr","removeCompleted":true,"removeFailed":true,"priority":1}}"#,
            ),
            (SETTINGS_SCOPE_MEDIA, RECYCLE_BIN_ENABLED_KEY, None, "false"),
        ] {
            ctx.settings_store
                .upsert_setting_json(scope, key, id.map(String::from), value.into(), "parity_flow", None)
                .await
                .expect("flow setting");
        }
        let user = ctx.app.find_or_create_default_user().await.expect("user");
        ctx.app
            .save_quality_profile_settings(
                &user,
                SaveQualityProfileSettings {
                    profiles: vec![scryer_application::builtin_default_quality_profile()],
                    replace_existing: true,
                    global_profile_id: Some(
                        scryer_application::BUILTIN_DEFAULT_QUALITY_PROFILE_ID.to_string(),
                    ),
                    category_selections: Vec::new(),
                    global_scoring_persona: None,
                    category_persona_selections: Vec::new(),
                },
            )
            .await
            .expect("quality profile");
        // acquisition idles without an enabled client row; the fake answers for it
        let now = chrono::Utc::now();
        let cfg_store = DownloadClientConfigStore::new(ctx.db.datastore(), ctx.db.encryption_key_state());
        let qrow = scryer_domain::DownloadClientConfig {
            id: "flow-qbit".into(),
            name: "qbit".into(),
            client_type: "qbittorrent".into(),
            config_json: "{}".into(),
            client_priority: 0,
            is_enabled: true,
            status: scryer_domain::DownloadClientStatus::Healthy,
            last_error: None,
            last_seen_at: None,
            proxy_config_id: None,
            created_at: now,
            updated_at: now,
        };
        cfg_store
        .create(qrow.clone())
        .await
        .expect("download client row");
        // the real router and category code run in front of the fake client
        let provider = FakeQbitProvider::default();
        provider.client_for_config(&qrow).expect("fake qbit");
        let qbit = provider.qbit().expect("fake qbit state");
        let datastore = ctx.db.datastore();
        let key = ctx.db.encryption_key_state();
        let indexer_configs = Arc::new(IndexerConfigStore::new(datastore.clone(), key.clone()));
        let proxy_configs = Arc::new(ProxyConfigStore::new(datastore.clone(), key));
        let pipeline = Arc::new(tokio::sync::Semaphore::new(4));
        let resolver = Arc::new(AcquisitionIndexerArtifactResolver::new(
            indexer_configs.clone(),
            proxy_configs.clone(),
            ctx.staged_nzb_store.clone(),
            pipeline.clone(),
        ));
        // wired as main.rs wires the router
        let router = PrioritizedDownloadClientRouter::new(
            Arc::new(cfg_store),
            ctx.settings_store.clone(),
            ctx.staged_nzb_store.clone(),
            pipeline,
            Some(Arc::new(provider.clone())),
        )
        .with_indexer_config_repositories(indexer_configs, proxy_configs)
        .with_indexer_artifact_resolver(resolver)
        .with_seed_goal_resolution(Arc::new(SeedingProfileStore::new(datastore)));
        let client = Arc::new(router);
        let provider = Arc::new(provider);
        let imports = Arc::new(ImportStore::new(ctx.db.datastore()));
        let files = Arc::new(ctx.media_files.clone());
        let scope = Arc::new(ctx.library_state.clone());
        let app = ctx.app.with_test_overrides(move |b| {
            b.with_download_client(client)
                .with_download_client_plugin_provider(provider)
                .with_imports(imports)
                .with_file_importer(Arc::new(FsFileImporter::new()))
                .with_media_files(files)
                .with_acquisition_scope_states(scope)
        });
        Self { ctx, app, user, qbit, media: tempfile::tempdir().expect("media tree") }
    }

    async fn add_torznab(&self, id: &str, fake: &TorznabFake) {
        let now = chrono::Utc::now();
        // our Prowlarr-synced indexers carry seed criteria: ratio 1, 7 days
        SeedingProfileStore::new(self.ctx.db.datastore())
            .create(scryer_domain::SeedingProfile {
                id: "prowlarr-seed".into(),
                name: "prowlarr-seed".into(),
                ratio: Some(1.0),
                seed_time_minutes: Some(10_080),
                season_pack_mode: scryer_domain::SeasonPackSeedMode::Inherit,
                season_pack_ratio: None,
                season_pack_seed_time_minutes: None,
                honor_tracker_minimums: false,
                goal_met_action: scryer_domain::SeedGoalMetAction::RemoveEntry,
                never_remove: false,
                minimum_seeders: None,
                post_import_tracking: scryer_domain::PostImportTracking::Park,
                created_at: now,
                updated_at: now,
            })
            .await
            .expect("seeding profile");
        let url = fake.api_url();
        let config = IndexerConfig {
            last_error_code: None,
            id: id.into(),
            name: id.into(),
            provider_type: "torznab".into(),
            base_url: url.clone(),
            api_key_encrypted: Some("test-api-key".into()),
            is_enabled: true,
            enable_interactive_search: true,
            enable_auto_search: true,
            proxy_config_id: None,
            download_client_id: None,
            seeding_profile_id: Some("prowlarr-seed".into()),
            managed_parent_config_id: None,
            managed_child_key: None,
            managed_metadata_json: None,
            caps_snapshot_json: None,
            rate_limit_seconds: Some(0),
            rate_limit_burst: None,
            max_queries_per_minute: None,
            disabled_until: None,
            last_health_status: None,
            last_error_message: None,
            last_error_at: None,
            config_json: Some(
                serde_json::json!({"base_url": url, "api_key": "test-api-key"}).to_string(),
            ),
            created_at: now,
            updated_at: now,
        };
        IndexerConfigStore::new(self.ctx.db.datastore(), self.ctx.db.encryption_key_state())
            .create(config)
            .await
            .expect("indexer config");
    }

    async fn set_root(&self, facet: MediaFacet) -> String {
        let library_id = scryer_domain::default_library_id_for_facet(&facet);
        let library = self.ctx.libraries.get_by_id(&library_id).await.unwrap().unwrap();
        let updated = self
            .ctx
            .libraries
            .update(
                &library_id,
                library.name,
                library.slug,
                vec![LibraryRootDraft {
                    id: None,
                    path: self.media.path().display().to_string(),
                    is_default: true,
                }],
            )
            .await
            .expect("root");
        updated.roots.iter().find(|r| r.is_default).expect("root").id.clone()
    }

    async fn add_movie(&self, name: &str, tmdb: &str) -> Title {
        let root = self.set_root(MediaFacet::Movie).await;
        let title = self
            .app
            .add_title(
                &self.user,
                NewTitle {
                    name: name.into(),
                    facet: MediaFacet::Movie,
                    monitored: true,
                    external_ids: vec![ExternalId::new("tmdb", tmdb)],
                    root_folder_id: Some(root),
                    year: Some(2024),
                    min_availability: Some("announced".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("add movie");
        title
    }

    /// The 20 s stubs must pass the runtime-sample check. Set after the grab:
    /// a one-minute runtime would make the 8 GB feed items fail the size limit.
    async fn shorten(&self, title: &Title) {
        let done = sqlx::query("UPDATE titles SET runtime_minutes = 1 WHERE id = ?")
            .bind(&title.id)
            .execute(&self.ctx.db.pool)
            .await
            .expect("short runtime");
        assert_eq!(done.rows_affected(), 1);
    }

    /// Queue the title's wanted search, then run one acquisition cycle.
    async fn search_and_grab(&self, title: &Title) {
        let outcome = self
            .app
            .trigger_title_wanted_search(&self.user, &title.id, SubmissionConflictPolicy::Skip)
            .await
            .expect("queue search");
        assert_eq!(outcome.queued_count, 1, "wanted search queued nothing: {outcome:?}");
        self.app.run_acquisition_cycle_for_test().await;
    }
}

fn seed(key: &str, ty: &str, default: &str) -> SettingDefinitionSeed {
    SettingDefinitionSeed {
        category: "media".into(),
        scope: "system".into(),
        key_name: key.into(),
        data_type: ty.into(),
        default_value_json: default.into(),
        is_sensitive: false,
        validation_json: None,
    }
}

// --- checks ---------------------------------------------------------------

fn fixture(res: u32, dest: &Path) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        // 20 s of black at 1 fps: the import gate rejects anything under 15 s
        .join("../../tests/fixtures/flow/media")
        .join(format!("h264_{res}p_20s.mkv"));
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::fs::copy(src, dest).expect("copy import fixture");
}

/// Movie chain: add -> search -> grab. Shared by 0004..0006, 0010, 0011, 0016.
struct Grabbed {
    flow: Flow,
    fake: TorznabFake,
    title: Title,
}

async fn grabbed_movie() -> Grabbed {
    let flow = Flow::new().await;
    let fake = TorznabFake::start().await;
    // caps carry no movie search, so Scryer asks `t=search`: serve the movie feed there
    let movie = include_str!("../../../tests/fixtures/flow/torznab/movie.xml");
    fake.set_feed("search", movie);
    fake.set_feed("movie", movie);
    flow.add_torznab("idx-a", &fake).await;
    let title = flow.add_movie("Invented Movie", "9001").await;
    flow.search_and_grab(&title).await;
    Grabbed { flow, fake, title }
}

impl Grabbed {
    fn hash(&self) -> Result<String, String> {
        self.flow
            .qbit
            .adds()
            .first()
            .map(|a| a.hash.clone())
            .ok_or_else(|| "no grab reached the client".to_string())
    }

    /// Put a finished payload in a temp downloads tree and mark it complete.
    async fn complete(&self, hash: &str) -> PathBuf {
        self.flow.shorten(&self.title).await;
        let dir = self.flow.media.path().join("downloads").join("payload");
        fixture(1080, &dir.join("Invented.Movie.2024.1080p.BluRay.x264-FAKEGRP.mkv"));
        self.flow.qbit.complete(hash, dir.to_str().unwrap());
        dir
    }

    /// A tracked-download poller on a short tick (the loop is the app's own;
    /// ponytail: real interval timer, there is no single-tick entry point).
    fn poller(&self) -> (tokio_util::sync::CancellationToken, tokio::task::JoinHandle<()>) {
        let token = tokio_util::sync::CancellationToken::new();
        let (_tx, rx) = tokio::sync::mpsc::channel(8);
        let (_stx, srx) = tokio::sync::mpsc::channel(1);
        let options = DownloadQueuePollerOptions {
            interval: std::time::Duration::from_millis(50),
            ..Default::default()
        };
        let handle = tokio::spawn(start_download_queue_poller_with_options(
            self.flow.app.clone(),
            token.clone(),
            rx,
            srx,
            options,
        ));
        (token, handle)
    }

    async fn imported_paths(&self) -> Vec<String> {
        self.flow
            .ctx
            .media_files
            .list_media_files_for_title(&self.title.id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|f| f.file_path)
            .collect()
    }

}

async fn c0003() -> Outcome {
    seam("watchlist-to-title add: submit_media_request/approve_media_request exist but the default quality profile, series tag, season-folder and min-availability rules come from Seerr config that Scryer has no request-default settings for in the test context")
}

async fn c0004() -> Outcome {
    let g = grabbed_movie().await;
    ensure(!g.fake.search_queries().is_empty(), || {
        format!("the add made no search request; queries: {:?}", g.fake.queries())
    })?;
    seam("indexer priority: IndexerConfig has no priority field and search order is not priority-driven")
}

async fn c0005() -> Outcome {
    let g = grabbed_movie().await;
    let adds = g.flow.qbit.adds();
    ensure(adds.len() == 1, || format!("expected one grab, got {}", adds.len()))?;
    let title = adds[0].release_title.clone().unwrap_or_default();
    // Our captured profile and custom formats are private; the built-in
    // profile must pick the 1080p BluRay over the 2160p WEB-DL DV feed item.
    ensure(title.contains("1080p"), || format!("grabbed {title:?}"))
}

async fn c0006() -> Outcome {
    let g = grabbed_movie().await;
    let adds = g.flow.qbit.adds();
    let add = adds.first().ok_or("no grab reached the client")?;
    ensure(add.category.as_deref() == Some("radarr"), || {
        format!("category {:?}, want radarr", add.category)
    })
}

/// Direct movie import of one 1080p fixture; returns the import result.
async fn import_movie(
    flow: &Flow,
    title: &Title,
    release: &str,
    item: &str,
    res: u32,
) -> scryer_domain::ImportResult {
    let dir = flow.media.path().join("downloads").join(item);
    fixture(res, &dir.join(format!("{release}.mkv")));
    let completed = scryer_domain::CompletedDownload {
        subtitle_paths: Vec::new(),
        client_type: "qbittorrent".into(),
        client_id: "flow-qbit".into(),
        download_client_item_id: item.into(),
        download_id: None,
        name: release.into(),
        release_name: Some(release.into()),
        dest_dir: dir.display().to_string(),
        category: Some("radarr".into()),
        size_bytes: None,
        completed_at: None,
        parameters: vec![
            ("*scryer_title_id".into(), title.id.clone()),
            ("*scryer_facet".into(), "movie".into()),
        ],
        posted_file_names: Vec::new(),
    };
    import_completed_download(&flow.app, &flow.user, &completed)
        .await
        .expect("import")
}

async fn c0007() -> Outcome {
    let flow = Flow::new().await;
    let title = flow.add_movie("Invented Movie: Reckoning", "9002").await;
    flow.shorten(&title).await;
    let r = import_movie(&flow, &title, "Invented.Movie.Reckoning.2024.1080p.BluRay.x264-FAKEGRP", "i7", 1080).await;
    let dest = r.dest_path.clone().ok_or_else(|| format!("not imported: {:?} {:?}", r.decision, r.error_message))?;
    let name = Path::new(&dest).file_name().unwrap().to_string_lossy().to_string();
    ensure(r.link_type == Some(scryer_domain::ImportStrategy::HardLink), || {
        format!("link type {:?}, want hardlink", r.link_type)
    })?;
    ensure(!name.contains(':') && name.contains("Invented Movie - Reckoning"), || {
        format!("file name {name:?}: colon not replaced by ' - ' (Smart colons)")
    })
}

async fn c0008() -> Outcome {
    let flow = Flow::new().await;
    let title = flow.add_movie("Invented Movie", "9003").await;
    flow.shorten(&title).await;
    let r = import_movie(&flow, &title, "Invented.Movie.2024.1080p.BluRay.x264-FAKEGRP", "i8", 1080).await;
    let dest = r.dest_path.ok_or("not imported")?;
    ensure(Path::new(&dest).starts_with(flow.media.path()) && Path::new(&dest).is_file(), || {
        format!("{dest} is not a file under the root folder {}", flow.media.path().display())
    })
}

async fn c0009() -> Outcome {
    seam("subtitles: no Bazarr-equivalent fake; the SubtitleSearch job needs a subtitle plugin provider and language settings not wired in TestContext")
}

async fn c0010() -> Outcome {
    let g = grabbed_movie().await;
    let hash = g.hash()?;
    g.complete(&hash).await;
    g.flow.qbit.set_seeding(&hash, 99.0, 999_999);
    let (token, handle) = g.poller();
    let imported = loop_until(|| async { !g.imported_paths().await.is_empty() }).await;
    let gone = loop_until(|| async { g.flow.qbit.torrents().is_empty() }).await;
    token.cancel();
    let _ = handle.await;
    ensure(imported, || "the completed download was not imported".into())?;
    ensure(gone, || "the torrent was not removed after its seed goal".into())?;
    let paths = g.imported_paths().await;
    ensure(paths.iter().all(|p| Path::new(p).is_file()), || {
        format!("imported file missing after torrent removal: {paths:?}")
    })
}

async fn loop_until<F: Fn() -> Fut, Fut: Future<Output = bool>>(f: F) -> bool {
    for _ in 0..200 {
        if f().await {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    false
}

async fn c0011() -> Outcome {
    let g = grabbed_movie().await;
    let hash = g.hash()?;
    g.flow.qbit.fail(&hash);
    let before = g.fake.search_queries().len();
    let (token, handle) = g.poller();
    // D96: like the arrs, the torrent itself goes only once the client says it
    // can be removed (qBittorrent: seed limit reached), so it is not asserted.
    let blocklisted = loop_until(|| async {
        !g.flow.ctx.library_state.blocklist.list_for_title(&g.title.id, 10).await.unwrap_or_default().is_empty()
    })
    .await;
    g.flow.app.run_acquisition_cycle_for_test().await;
    token.cancel();
    let _ = handle.await;
    ensure(blocklisted, || "the failed release was not blocklisted".into())?;
    ensure(g.fake.search_queries().len() > before, || "no new search ran after the failure".into())
}

async fn c0012() -> Outcome {
    let flow = Flow::new().await;
    let title = flow.add_movie("Invented Movie", "9004").await;
    flow.shorten(&title).await;
    let first = import_movie(&flow, &title, "Invented.Movie.2024.720p.BluRay.x264-FAKEGRP", "u1", 720).await;
    let old = first.dest_path.ok_or("first import failed")?;
    let second = import_movie(&flow, &title, "Invented.Movie.2024.1080p.BluRay.x264-FAKEGRP", "u2", 1080).await;
    let new = second.dest_path.clone().ok_or_else(|| format!("upgrade not imported: {:?} {:?}", second.decision, second.error_message))?;
    ensure(Path::new(&new).is_file(), || format!("new file {new} missing"))?;
    ensure(new == old || !Path::new(&old).exists(), || format!("old file {old} still on disk (no recycle bin wanted)"))?;
    let recycled = walk(flow.media.path()).into_iter().any(|p| p.to_string_lossy().to_lowercase().contains("recycle"));
    ensure(!recycled, || "the old file went to a recycle bin".into())
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            out.push(e.path());
            if e.path().is_dir() {
                out.extend(walk(&e.path()));
            }
        }
    }
    out
}

async fn c0013() -> Outcome {
    seam("season packs: needs a series with aired-season metadata (SMG mock series fixture) and a tvsearch feed with a pack; TestContext has no series fixture builder for aired seasons")
}

async fn c0014() -> Outcome {
    seam("cleanup rules: needs maintenance rules (requested + watched-by-requester + age) over FakePlexServer signals and an injected clock; no clock seam in the maintenance evaluation")
}

async fn c0015() -> Outcome {
    seam("cleanup scope: same missing seams as STK-FLOW-0014 (no cleanup run to drive against a foreign file)")
}

async fn c0016() -> Outcome {
    let g = grabbed_movie().await;
    let hash = g.hash()?;
    // First process: tracks the download while it is still running, then dies.
    let (token, handle) = g.poller();
    g.flow.qbit.set_progress(&hash, 40);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    token.cancel();
    let _ = handle.await;
    // Second process: the download finishes while Scryer is down.
    g.complete(&hash).await;
    let (token, handle) = g.poller();
    let imported = loop_until(|| async { !g.imported_paths().await.is_empty() }).await;
    token.cancel();
    let _ = handle.await;
    ensure(imported, || "after a restart the completed download was not imported".into())
}

async fn c0017() -> Outcome {
    let flow = Flow::new().await;
    let title = flow.add_movie("Invented Movie", "9005").await;
    flow.shorten(&title).await;
    let r = import_movie(&flow, &title, "Invented.Movie.2024.1080p.BluRay.x264-FAKEGRP", "d1", 1080).await;
    ensure(r.dest_path.is_some(), || "import failed".into())?;
    let file = flow.ctx.media_files.list_media_files_for_title(&title.id).await.unwrap().into_iter().next().ok_or("no media file row")?;
    flow.app.delete_media_file(&flow.user, &file.id, false, None).await.map_err(|e| format!("delete: {e}"))?;
    let after = flow.app.get_title(&flow.user, &title.id).await.unwrap().ok_or("title gone")?;
    ensure(!after.monitored, || "movie still monitored after its file was deleted".into())
}

async fn run<F: Future<Output = Outcome> + Send + 'static>(f: F) -> Outcome {
    match tokio::spawn(f).await {
        Ok(o) => o,
        Err(e) if e.is_panic() => {
            let p = e.into_panic();
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            Err(format!("panicked: {msg}"))
        }
        Err(e) => Err(e.to_string()),
    }
}

async fn c0001() -> Outcome {
    seam("watchlist sync: TestContext wires no list store (with_list_store) or list subscription for the FakePlexWatchlistProvider, so ListSync cannot run")
}

async fn c0002() -> Outcome {
    seam("watchlist sync (series, all seasons, specials off): same missing list store wiring as STK-FLOW-0001")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stack_flow() {
    if std::env::var_os("FLOW_TRACE").is_some_and(|v| !v.is_empty()) {
        tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).with_writer(std::io::stderr).init();
    }
    let mut suite = Suite::new("scryer::parity_flow::stack_flow");
    macro_rules! go {
        ($id:literal, $f:expr) => {
            if suite.wants($id) {
                match run($f).await {
                    Ok(()) => suite.record($id, true, ""),
                    Err(why) => {
                        eprintln!("{}: {why}", $id);
                        suite.record($id, false, why)
                    }
                }
            }
        };
    }
    go!("STK-FLOW-0001", c0001());
    go!("STK-FLOW-0002", c0002());
    go!("STK-FLOW-0003", c0003());
    go!("STK-FLOW-0004", c0004());
    go!("STK-FLOW-0005", c0005());
    go!("STK-FLOW-0006", c0006());
    go!("STK-FLOW-0007", c0007());
    go!("STK-FLOW-0008", c0008());
    go!("STK-FLOW-0009", c0009());
    go!("STK-FLOW-0010", c0010());
    go!("STK-FLOW-0011", c0011());
    go!("STK-FLOW-0012", c0012());
    go!("STK-FLOW-0013", c0013());
    go!("STK-FLOW-0014", c0014());
    go!("STK-FLOW-0015", c0015());
    go!("STK-FLOW-0016", c0016());
    go!("STK-FLOW-0017", c0017());
    suite.finish();
}
