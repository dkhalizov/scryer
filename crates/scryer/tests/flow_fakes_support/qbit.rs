//! qBittorrent stand-in at the `DownloadClient` port (D91 F2).
//!
//! The real client is an out-of-tree wasm plugin, so this fake is the
//! plugin's seat: it sits behind the real `PrioritizedDownloadClientRouter`
//! (via [`FakeQbitProvider`]) and the real routing/category/seed-goal code
//! runs in front of it. The wire protocol is tier 2.
// fork (D91 F2)

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use scryer_application::{
    AppResult, ClientJobLocator, DownloadCleanupPayload, DownloadClient, DownloadClientAddRequest,
    DownloadClientMarkImportedRequest, DownloadClientObservation, DownloadClientPluginProvider,
    DownloadGrabResult,
};
use scryer_domain::{
    CompletedDownload, DownloadClientConfig, DownloadQueueItem, DownloadQueueState,
    DownloadSeedingSnapshot,
};

pub const CLIENT_TYPE: &str = "qbittorrent";

/// What one add looked like, as the client received it.
#[derive(Clone, Debug, PartialEq)]
pub struct AddRecord {
    pub hash: String,
    pub title_id: String,
    pub title_name: String,
    pub category: Option<String>,
    pub save_path: Option<String>,
    pub seed_goal_ratio: Option<f64>,
    pub seed_goal_seconds: Option<i64>,
    pub source_kind: Option<String>,
    pub source_hint: Option<String>,
    pub release_title: Option<String>,
    pub indexer_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TorrentState {
    Downloading,
    Stalled,
    Failed,
    /// Finished; seeding.
    Complete,
}

#[derive(Clone, Debug)]
pub struct Torrent {
    pub add: AddRecord,
    pub name: String,
    pub state: TorrentState,
    pub progress: u8,
    pub size_bytes: i64,
    /// Where the payload sits once complete (`CompletedDownload.dest_dir`).
    pub content_path: String,
    pub seed_ratio: Option<f64>,
    pub seed_time_seconds: Option<i64>,
}

/// Every call the app made, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Add(String),
    Delete { hash: String, remove_data: bool },
    MarkImported(String),
}

#[derive(Default)]
struct State {
    torrents: Vec<Torrent>,
    calls: Vec<Call>,
    next: u64,
}

#[derive(Clone)]
pub struct FakeQbit {
    state: Arc<Mutex<State>>,
    client_id: String,
    client_name: String,
}

impl Default for FakeQbit {
    fn default() -> Self {
        Self::new("qbit-fake", "qBittorrent (fake)")
    }
}

impl FakeQbit {
    pub fn new(client_id: &str, client_name: &str) -> Self {
        Self {
            state: Arc::default(),
            client_id: client_id.into(),
            client_name: client_name.into(),
        }
    }

    pub fn torrents(&self) -> Vec<Torrent> {
        self.state.lock().unwrap().torrents.clone()
    }

    pub fn torrent(&self, hash: &str) -> Option<Torrent> {
        self.torrents().into_iter().find(|t| t.add.hash == hash)
    }

    pub fn adds(&self) -> Vec<AddRecord> {
        self.torrents().into_iter().map(|t| t.add).collect()
    }

    pub fn calls(&self) -> Vec<Call> {
        self.state.lock().unwrap().calls.clone()
    }

    pub fn set_progress(&self, hash: &str, percent: u8) {
        self.edit(hash, |t| {
            t.progress = percent.min(99);
            t.state = TorrentState::Downloading;
        });
    }

    /// Finish the torrent; the payload is at `content_path`.
    pub fn complete(&self, hash: &str, content_path: &str) {
        self.edit(hash, |t| {
            t.progress = 100;
            t.state = TorrentState::Complete;
            t.content_path = content_path.to_string();
        });
    }

    pub fn stall(&self, hash: &str) {
        self.edit(hash, |t| t.state = TorrentState::Stalled);
    }

    pub fn fail(&self, hash: &str) {
        self.edit(hash, |t| t.state = TorrentState::Failed);
    }

    /// What the client reports for seeding progress.
    pub fn set_seeding(&self, hash: &str, ratio: f64, seconds: i64) {
        self.edit(hash, |t| {
            t.seed_ratio = Some(ratio);
            t.seed_time_seconds = Some(seconds);
        });
    }

    fn edit(&self, hash: &str, f: impl FnOnce(&mut Torrent)) {
        let mut state = self.state.lock().unwrap();
        let torrent = state
            .torrents
            .iter_mut()
            .find(|t| t.add.hash == hash)
            .unwrap_or_else(|| panic!("fake qbit: no torrent {hash}"));
        f(torrent);
    }

    fn queue_item(&self, t: &Torrent) -> DownloadQueueItem {
        let (state, attention, reason) = match t.state {
            TorrentState::Downloading => (DownloadQueueState::Downloading, false, None),
            TorrentState::Stalled => (DownloadQueueState::Warning, true, Some("stalled")),
            TorrentState::Failed => (DownloadQueueState::Failed, true, Some("error")),
            TorrentState::Complete => (DownloadQueueState::Completed, false, None),
        };
        DownloadQueueItem {
            id: t.add.hash.clone(),
            title_id: Some(t.add.title_id.clone()),
            episode_id: None,
            title_name: t.add.title_name.clone(),
            facet: None,
            category: t.add.category.clone(),
            client_id: self.client_id.clone(),
            client_name: self.client_name.clone(),
            client_type: CLIENT_TYPE.into(),
            state,
            progress_percent: t.progress,
            import_transfer_phase: None,
            import_transfer_bytes: None,
            import_transfer_total_bytes: None,
            import_transfer_started_at: None,
            import_transfer_updated_at: None,
            size_bytes: Some(t.size_bytes),
            remaining_seconds: None,
            queued_at: None,
            last_updated_at: None,
            attention_required: attention,
            attention_reason: reason.map(str::to_string),
            attention_reason_code: None,
            download_client_item_id: t.add.hash.clone(),
            download_id: None,
            import_status: None,
            import_type: None,
            import_error_code: None,
            import_error_message: None,
            imported_at: None,
            delete_status: None,
            delete_error_message: None,
            is_scryer_origin: false,
            source_provider: None,
            tracked_state: None,
            tracked_status: None,
            tracked_status_messages: Vec::new(),
            tracked_match_type: None,
            seeding: Some(DownloadSeedingSnapshot {
                seed_ratio: t.seed_ratio,
                seed_time_seconds: t.seed_time_seconds,
                seed_goal_ratio: t.add.seed_goal_ratio,
                seed_goal_seconds: t.add.seed_goal_seconds,
                ..Default::default()
            }),
        }
    }

    fn completed(&self, t: &Torrent) -> CompletedDownload {
        CompletedDownload {
            client_type: CLIENT_TYPE.into(),
            client_id: self.client_id.clone(),
            download_client_item_id: t.add.hash.clone(),
            download_id: None,
            name: t.name.clone(),
            release_name: Some(t.name.clone()),
            dest_dir: t.content_path.clone(),
            category: t.add.category.clone(),
            size_bytes: Some(t.size_bytes),
            completed_at: Some(chrono::Utc::now()),
            parameters: Vec::new(),
            posted_file_names: Vec::new(),
            subtitle_paths: Vec::new(),
        }
    }
}

/// `btih:` of a magnet link, lower-cased, when it is 40-hex.
fn magnet_hash(link: &str) -> Option<String> {
    let rest = link.split("btih:").nth(1)?;
    let hash: String = rest
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect();
    (hash.len() == 40).then(|| hash.to_ascii_lowercase())
}

#[async_trait]
impl DownloadClient for FakeQbit {
    async fn submit_download(&self, r: &DownloadClientAddRequest) -> AppResult<DownloadGrabResult> {
        let mut state = self.state.lock().unwrap();
        state.next += 1;
        let hash = r
            .info_hash_hint
            .as_deref()
            .map(str::to_ascii_lowercase)
            .or_else(|| r.source_hint.as_deref().and_then(magnet_hash))
            .unwrap_or_else(|| format!("{:040x}", state.next));
        let name = r
            .release_title
            .clone()
            .or_else(|| r.source_title.clone())
            .unwrap_or_else(|| r.title.name.clone());
        let add = AddRecord {
            hash: hash.clone(),
            title_id: r.title.id.clone(),
            title_name: r.title.name.clone(),
            category: r.category.clone(),
            save_path: r.download_directory.clone(),
            seed_goal_ratio: r.seed_goal_ratio,
            seed_goal_seconds: r.seed_goal_seconds,
            source_kind: r.source_kind.map(|k| k.as_str().to_string()),
            source_hint: r.source_hint.clone(),
            release_title: r.release_title.clone(),
            indexer_name: r.indexer_name.clone(),
        };
        let content_path = format!("{}/{name}", add.save_path.clone().unwrap_or_default());
        state.torrents.push(Torrent {
            add,
            name,
            state: TorrentState::Downloading,
            progress: 0,
            size_bytes: 1_000_000,
            content_path,
            seed_ratio: None,
            seed_time_seconds: None,
        });
        state.calls.push(Call::Add(hash.clone()));
        Ok(DownloadGrabResult {
            job_id: hash.clone(),
            client_id: Some(self.client_id.clone()),
            client_type: CLIENT_TYPE.into(),
            info_hash: Some(hash),
            download_id: r.download_id.clone(),
            seed_goals: None,
            grabbed_file_names: Vec::new(),
        })
    }

    async fn list_queue(&self) -> AppResult<Vec<DownloadQueueItem>> {
        Ok(self.torrents().iter().map(|t| self.queue_item(t)).collect())
    }

    async fn list_history(&self) -> AppResult<Vec<DownloadQueueItem>> {
        Ok(Vec::new())
    }

    async fn list_completed_downloads(&self) -> AppResult<Vec<CompletedDownload>> {
        Ok(self
            .torrents()
            .iter()
            .filter(|t| t.state == TorrentState::Complete)
            .map(|t| self.completed(t))
            .collect())
    }

    async fn get_cleanup_payload_for_source(
        &self,
        _client_id: &str,
        _client_type: &str,
        item_id: &str,
    ) -> AppResult<Option<DownloadCleanupPayload>> {
        Ok(self.torrent(item_id).map(|t| DownloadCleanupPayload {
            download: self.completed(&t),
            native_delete_paths: Vec::new(),
        }))
    }

    /// The fake's list is the whole truth, so absence is provable.
    async fn observe_download(
        &self,
        locator: &ClientJobLocator,
        _history_offset: usize,
    ) -> AppResult<DownloadClientObservation> {
        Ok(match self.torrent(&locator.item_id) {
            Some(t) => DownloadClientObservation::Present(Box::new(self.queue_item(&t))),
            None => DownloadClientObservation::Absent,
        })
    }

    fn supports_native_data_removal(&self) -> bool {
        true
    }

    async fn delete_queue_item(
        &self,
        id: &str,
        _is_history: bool,
        remove_data: bool,
    ) -> AppResult<()> {
        let mut state = self.state.lock().unwrap();
        if let Some(pos) = state.torrents.iter().position(|t| t.add.hash == id) {
            let torrent = state.torrents.remove(pos);
            if remove_data {
                let path = std::path::Path::new(&torrent.content_path);
                let _ = if path.is_dir() {
                    std::fs::remove_dir_all(path)
                } else {
                    std::fs::remove_file(path)
                };
            }
        }
        state.calls.push(Call::Delete {
            hash: id.to_string(),
            remove_data,
        });
        Ok(())
    }

    async fn mark_imported(&self, request: &DownloadClientMarkImportedRequest) -> AppResult<()> {
        self.state
            .lock()
            .unwrap()
            .calls
            .push(Call::MarkImported(request.client_item_id.clone()));
        Ok(())
    }

    async fn test_connection(&self) -> AppResult<String> {
        Ok("fake qBittorrent".into())
    }
}

/// Hands the router a [`FakeQbit`] (sharing one state) for client type
/// `qbittorrent`, named after the config row so routing by client id works.
#[derive(Clone, Default)]
pub struct FakeQbitProvider {
    shared: Arc<Mutex<Option<FakeQbit>>>,
}

impl FakeQbitProvider {
    /// The fake behind the first config the router asked about.
    pub fn qbit(&self) -> Option<FakeQbit> {
        self.shared.lock().unwrap().clone()
    }
}

impl DownloadClientPluginProvider for FakeQbitProvider {
    fn client_for_config(&self, config: &DownloadClientConfig) -> Option<Arc<dyn DownloadClient>> {
        if !config.client_type.eq_ignore_ascii_case(CLIENT_TYPE) {
            return None;
        }
        let mut slot = self.shared.lock().unwrap();
        let first = slot.get_or_insert_with(|| FakeQbit::new(&config.id, &config.name));
        let mut client = first.clone();
        client.client_id = config.id.clone();
        client.client_name = config.name.clone();
        Some(Arc::new(client))
    }

    fn available_provider_types(&self) -> Vec<String> {
        vec![CLIENT_TYPE.to_string()]
    }

    // fork (P45): release search asks the provider what a client type accepts
    // and whether its plugin is loaded; without these it drops every torrent.
    fn accepted_inputs_for_provider(&self, provider_type: &str) -> Vec<String> {
        if provider_type.eq_ignore_ascii_case(CLIENT_TYPE) {
            vec!["torrent_file".to_string(), "magnet_uri".to_string()]
        } else {
            vec![]
        }
    }

    fn plugin_name_for_provider(&self, provider_type: &str) -> Option<String> {
        provider_type
            .eq_ignore_ascii_case(CLIENT_TYPE)
            .then(|| "qBittorrent (fake)".to_string())
    }
}
