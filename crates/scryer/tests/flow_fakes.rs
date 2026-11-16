//! Unit tests for the flow-harness fakes (D91 F2); they double as usage
//! examples for `parity_flow` (F3).
// fork (D91 F2)

mod flow_fakes_support;

use flow_fakes_support::plex::*;
use flow_fakes_support::qbit::*;
use scryer_application::lists::ListPluginProvider;
use scryer_application::{
    DownloadClient, DownloadClientAddRequest, DownloadClientObservation,
    DownloadClientPluginProvider, DownloadSourceKind, MediaServerSignalSource, ProviderPlayedItem,
};
use scryer_domain::{DownloadClientConfig, MediaServerConnection, Title};
use scryer_plugin_sdk::{ListMediaKind, ListPluginFetchRequest, PluginResult};

fn title() -> Title {
    serde_json::from_value(serde_json::json!({
        "id": "t1", "library_id": "l1", "name": "Made Up Show", "facet": "series",
        "monitored": true, "tags": [], "external_ids": [], "root_folder_id": "r1",
        "created_by": null, "created_at": "2026-11-16T00:00:00Z", "catalog_sort_key": "made up show",
        "canonical_tags": [], "aliases": [], "tagged_aliases": []
    }))
    .expect("title fixture")
}

fn add_request() -> DownloadClientAddRequest {
    let mut r = DownloadClientAddRequest::from_legacy(
        &title(),
        Some("magnet:?xt=urn:btih:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA1&dn=x".into()),
        Some(DownloadSourceKind::MagnetUri),
        None,
        None,
        Some("tv-flow".into()),
    );
    r.download_directory = Some("/downloads/tv".into());
    r.seed_goal_ratio = Some(1.5);
    r.seed_goal_seconds = Some(3600);
    r.release_title = Some("Made.Up.Show.S01E02.1080p.WEB-DL-GRP".into());
    r
}

const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1";

#[tokio::test]
async fn qbit_add_records_category_path_and_seed_goals() {
    let qbit = FakeQbit::default();
    let result = qbit.submit_download(&add_request()).await.unwrap();
    assert_eq!(result.info_hash.as_deref(), Some(HASH));
    let add = &qbit.adds()[0];
    assert_eq!(add.category.as_deref(), Some("tv-flow"));
    assert_eq!(add.save_path.as_deref(), Some("/downloads/tv"));
    assert_eq!(
        (add.seed_goal_ratio, add.seed_goal_seconds),
        (Some(1.5), Some(3600))
    );
    assert_eq!(add.source_kind.as_deref(), Some("magnet_uri"));
    assert_eq!(qbit.calls(), vec![Call::Add(HASH.into())]);
}

#[tokio::test]
async fn qbit_progress_complete_stall_and_listing() {
    let qbit = FakeQbit::default();
    qbit.submit_download(&add_request()).await.unwrap();
    qbit.set_progress(HASH, 40);
    let queue = qbit.list_queue().await.unwrap();
    assert_eq!(queue[0].progress_percent, 40);
    assert!(qbit.list_completed_downloads().await.unwrap().is_empty());

    qbit.stall(HASH);
    assert!(qbit.list_queue().await.unwrap()[0].attention_required);

    qbit.complete(HASH, "/downloads/tv/Made.Up.Show.S01E02.1080p.WEB-DL-GRP");
    qbit.set_seeding(HASH, 0.5, 60);
    let done = qbit.list_completed_downloads().await.unwrap();
    assert_eq!(
        done[0].dest_dir,
        "/downloads/tv/Made.Up.Show.S01E02.1080p.WEB-DL-GRP"
    );
    assert_eq!(done[0].category.as_deref(), Some("tv-flow"));
    let seeding = qbit.list_queue().await.unwrap()[0].seeding.clone().unwrap();
    assert_eq!(
        (seeding.seed_ratio, seeding.seed_goal_ratio),
        (Some(0.5), Some(1.5))
    );
}

#[tokio::test]
async fn qbit_delete_removes_torrent_and_optionally_data() {
    let dir = tempfile::tempdir().unwrap();
    let payload = dir.path().join("payload");
    std::fs::create_dir(&payload).unwrap();
    let qbit = FakeQbit::default();
    qbit.submit_download(&add_request()).await.unwrap();
    qbit.complete(HASH, payload.to_str().unwrap());
    qbit.delete_queue_item(HASH, false, true).await.unwrap();
    assert!(qbit.torrents().is_empty());
    assert!(!payload.exists());
    assert_eq!(
        qbit.calls().last(),
        Some(&Call::Delete {
            hash: HASH.into(),
            remove_data: true
        })
    );
    let locator = scryer_application::ClientJobLocator {
        client_id: None,
        client_type: CLIENT_TYPE.into(),
        item_id: HASH.into(),
    };
    assert!(matches!(
        qbit.observe_download(&locator, 0).await.unwrap(),
        DownloadClientObservation::Absent
    ));
}

#[test]
fn qbit_provider_serves_only_qbittorrent_and_shares_state() {
    let provider = FakeQbitProvider::default();
    let now = chrono::Utc::now();
    let config = |kind: &str| DownloadClientConfig {
        id: "c1".into(),
        name: "qbit".into(),
        client_type: kind.into(),
        config_json: "{}".into(),
        client_priority: 0,
        is_enabled: true,
        status: Default::default(),
        last_error: None,
        last_seen_at: None,
        proxy_config_id: None,
        created_at: now,
        updated_at: now,
    };
    assert!(provider.client_for_config(&config("sabnzbd")).is_none());
    assert!(provider.client_for_config(&config("qbittorrent")).is_some());
    assert_eq!(provider.qbit().unwrap().torrents().len(), 0);
}

#[tokio::test]
async fn plex_watchlist_serves_items_and_honours_fingerprint() {
    let fake = FakePlexWatchlist::new("plex");
    fake.set_items(vec![
        watchlist_item(
            "k1",
            "Made Up Show",
            2020,
            ListMediaKind::Series,
            &[("tvdb", "111")],
        ),
        watchlist_item(
            "k2",
            "Invented Movie",
            2024,
            ListMediaKind::Movie,
            &[("tmdb", "222")],
        ),
    ]);
    let provider = FakePlexWatchlistProvider(fake.clone());
    assert!(
        provider
            .client_for_provider("other", &Default::default())
            .is_none()
    );
    let client = provider
        .client_for_provider("plex", &Default::default())
        .unwrap();
    let request = |since: Option<String>| ListPluginFetchRequest {
        source_type: WATCHLIST_SOURCE.into(),
        params: Default::default(),
        credential: None,
        page_cursor: None,
        since_fingerprint: since,
    };
    let PluginResult::Ok(first) = client.fetch(request(None)).await.unwrap() else {
        panic!("not ok")
    };
    assert_eq!(first.items.len(), 2);
    assert_eq!(first.items[0].external_ids[0].id, "111");
    let PluginResult::Ok(again) = client
        .fetch(request(first.fingerprint.clone()))
        .await
        .unwrap()
    else {
        panic!("not ok")
    };
    assert!(again.unchanged && again.items.is_empty());
    assert_eq!(fake.fetched().len(), 2);
}

fn connection() -> MediaServerConnection {
    serde_json::from_value(serde_json::json!({
        "id": "m1", "provider": "plex", "display_name": "Plex", "base_url": "http://plex.invalid",
        "enabled": true, "login_enabled": false, "linking_enabled": false, "auto_add_enabled": false,
        "default_app_permissions": 0, "default_library_grants": [], "machine_id": null,
        "emby_server_id": null, "emby_connect_enabled": false, "path_mappings": [],
        "created_at": "2026-11-16T00:00:00Z", "updated_at": "2026-11-16T00:00:00Z"
    }))
    .expect("connection fixture")
}

#[tokio::test]
async fn plex_server_serves_played_state_and_records_reads() {
    let server = FakePlexServer::default();
    server.set_played(
        "u1",
        vec![ProviderPlayedItem {
            provider_item_id: "rk1".into(),
            played: true,
            play_count: 1,
            ..Default::default()
        }],
    );
    let reader = server.open(&connection()).await.unwrap();
    assert_eq!(reader.fetch_played_items("u1").await.unwrap().len(), 1);
    assert!(reader.fetch_played_items("u2").await.unwrap().is_empty());
    let rechecked = reader
        .recheck_items(&["rk1".to_string()], &["u1".to_string(), "u2".to_string()])
        .await
        .unwrap();
    let plays = &rechecked["rk1"].participants;
    assert!(plays[0].played && !plays[1].played);
    assert_eq!(server.reads(), vec!["u1", "u2"]);
    assert_eq!(server.rechecks().len(), 1);

    server.fail_open(Some(scryer_application::SignalReadFailure::new(
        "WATCH_SYNC_PLEX_SERVER_UNREACHABLE",
        "down",
    )));
    assert!(server.open(&connection()).await.is_err());
}
