//! Plex stand-ins at the application ports (D91 F2). The watchlist reader is
//! an out-of-tree wasm list plugin, and the watched-state adapter reaches
//! plex.tv through a hard-coded URL, so neither can be pointed at an HTTP
//! fake without touching real plex.tv. Scryer makes no Plex refresh call (F1).
// fork (D91 F2)

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use scryer_application::lists::{ListPluginProvider, ListProviderClient};
use scryer_application::{
    AppError, AppResult, MediaServerSignalReader, MediaServerSignalSource, ProviderItemPlay,
    ProviderItemRecheck, ProviderPlayedItem, SignalReadFailure,
};
use scryer_domain::MediaServerConnection;
use scryer_plugin_sdk::{
    ListAuthBadge, ListCredential, ListExternalId, ListMediaKind, ListPluginAccountResponse,
    ListPluginFetchRequest, ListPluginFetchResponse, ListPluginHealthResponse, ListPluginItem,
    ListProviderDescriptor, ListProviderGroup, ListProviderItem, PluginDescriptor, PluginResult,
    ProviderDescriptor,
};

pub const WATCHLIST_SOURCE: &str = "watchlist";

/// A watchlist entry as the plugin would return it.
pub fn watchlist_item(
    key: &str,
    title: &str,
    year: i32,
    kind: ListMediaKind,
    ids: &[(&str, &str)],
) -> ListPluginItem {
    ListPluginItem {
        item_key: key.into(),
        title: Some(title.into()),
        year: Some(year),
        kind_hint: Some(kind),
        external_ids: ids
            .iter()
            .map(|(source, id)| ListExternalId {
                source: (*source).into(),
                kind: None,
                id: (*id).into(),
            })
            .collect(),
        ..Default::default()
    }
}

/// Serves a user's Plex watchlist through the list-provider ports.
pub struct FakePlexWatchlist {
    descriptor: PluginDescriptor,
    provider_type: String,
    items: Mutex<Vec<ListPluginItem>>,
    fetched: Mutex<Vec<ListPluginFetchRequest>>,
}

impl FakePlexWatchlist {
    pub fn new(provider_type: &str) -> Arc<Self> {
        let mut provider: ListProviderDescriptor =
            serde_json::from_value(serde_json::json!({ "provider_type": provider_type }))
                .expect("minimal list descriptor");
        provider.groups = vec![ListProviderGroup {
            label: "Watchlist".into(),
            auth_badge: ListAuthBadge::MemberAccount,
            items: vec![ListProviderItem {
                id: format!("{provider_type}:{WATCHLIST_SOURCE}"),
                name: "Watchlist".into(),
                description: None,
                kinds: vec![ListMediaKind::Movie, ListMediaKind::Series],
                source_type: WATCHLIST_SOURCE.into(),
                params: Vec::new(),
                personal: true,
                default_interval_seconds: 900,
            }],
        }];
        Arc::new(Self {
            descriptor: PluginDescriptor {
                settings: Vec::new(),
                id: provider_type.into(),
                name: "Plex (fake)".into(),
                version: "1.0.0".into(),
                sdk_version: scryer_plugin_sdk::SDK_VERSION.into(),
                sdk_constraint: scryer_plugin_sdk::current_sdk_constraint(),
                socket_permissions: Vec::new(),
                provider: ProviderDescriptor::ListProvider(provider),
            },
            provider_type: provider_type.into(),
            items: Mutex::default(),
            fetched: Mutex::default(),
        })
    }

    /// Replace the watchlist the next fetch returns.
    pub fn set_items(&self, items: Vec<ListPluginItem>) {
        *self.items.lock().unwrap() = items;
    }

    pub fn fetched(&self) -> Vec<ListPluginFetchRequest> {
        self.fetched.lock().unwrap().clone()
    }

    fn fingerprint(items: &[ListPluginItem]) -> String {
        items
            .iter()
            .map(|i| i.item_key.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[async_trait]
impl ListProviderClient for FakePlexWatchlist {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    async fn fetch(
        &self,
        request: ListPluginFetchRequest,
    ) -> AppResult<PluginResult<ListPluginFetchResponse>> {
        self.fetched.lock().unwrap().push(request.clone());
        let items = self.items.lock().unwrap().clone();
        let fingerprint = Self::fingerprint(&items);
        if request.since_fingerprint.as_deref() == Some(fingerprint.as_str()) {
            return Ok(PluginResult::Ok(ListPluginFetchResponse {
                unchanged: true,
                fingerprint: Some(fingerprint),
                ..Default::default()
            }));
        }
        Ok(PluginResult::Ok(ListPluginFetchResponse {
            total_hint: u32::try_from(items.len()).ok(),
            items,
            fingerprint: Some(fingerprint),
            ..Default::default()
        }))
    }

    async fn account(
        &self,
        _credential: ListCredential,
    ) -> AppResult<PluginResult<ListPluginAccountResponse>> {
        Err(AppError::Validation(
            "fake plex watchlist: account not scripted".into(),
        ))
    }

    async fn health(&self) -> AppResult<PluginResult<ListPluginHealthResponse>> {
        Err(AppError::Validation(
            "fake plex watchlist: health not scripted".into(),
        ))
    }
}

/// `ListPluginProvider` that hands out one [`FakePlexWatchlist`].
pub struct FakePlexWatchlistProvider(pub Arc<FakePlexWatchlist>);

impl ListPluginProvider for FakePlexWatchlistProvider {
    fn client_for_provider(
        &self,
        provider_type: &str,
        _config: &BTreeMap<String, String>,
    ) -> Option<Arc<dyn ListProviderClient>> {
        (provider_type == self.0.provider_type)
            .then(|| self.0.clone() as Arc<dyn ListProviderClient>)
    }

    fn descriptors(&self) -> Vec<PluginDescriptor> {
        vec![self.0.descriptor.clone()]
    }

    fn available_provider_types(&self) -> Vec<String> {
        vec![self.0.provider_type.clone()]
    }
}

#[derive(Default)]
struct ServerState {
    /// Played items per Plex user id.
    played: HashMap<String, Vec<ProviderPlayedItem>>,
    /// `(user id)` of every `fetch_played_items`, in order.
    reads: Vec<String>,
    /// `(item ids, participants)` of every `recheck_items`.
    rechecks: Vec<(Vec<String>, Vec<String>)>,
    open_failure: Option<SignalReadFailure>,
}

/// Plex server watched state, through `MediaServerSignalSource`.
#[derive(Clone, Default)]
pub struct FakePlexServer {
    state: Arc<Mutex<ServerState>>,
}

impl FakePlexServer {
    pub fn set_played(&self, user_id: &str, items: Vec<ProviderPlayedItem>) {
        self.state
            .lock()
            .unwrap()
            .played
            .insert(user_id.into(), items);
    }

    /// Make `open` fail, as an unreachable server would.
    pub fn fail_open(&self, failure: Option<SignalReadFailure>) {
        self.state.lock().unwrap().open_failure = failure;
    }

    pub fn reads(&self) -> Vec<String> {
        self.state.lock().unwrap().reads.clone()
    }

    pub fn rechecks(&self) -> Vec<(Vec<String>, Vec<String>)> {
        self.state.lock().unwrap().rechecks.clone()
    }
}

#[async_trait]
impl MediaServerSignalSource for FakePlexServer {
    async fn open(
        &self,
        _connection: &MediaServerConnection,
    ) -> Result<Box<dyn MediaServerSignalReader>, SignalReadFailure> {
        if let Some(failure) = self.state.lock().unwrap().open_failure.clone() {
            return Err(failure);
        }
        Ok(Box::new(self.clone()))
    }
}

#[async_trait]
impl MediaServerSignalReader for FakePlexServer {
    async fn fetch_played_items(
        &self,
        external_user_id: &str,
    ) -> Result<Vec<ProviderPlayedItem>, SignalReadFailure> {
        let mut state = self.state.lock().unwrap();
        state.reads.push(external_user_id.into());
        Ok(state
            .played
            .get(external_user_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn recheck_items(
        &self,
        provider_item_ids: &[String],
        participants: &[String],
    ) -> Result<BTreeMap<String, ProviderItemRecheck>, SignalReadFailure> {
        let mut state = self.state.lock().unwrap();
        state
            .rechecks
            .push((provider_item_ids.to_vec(), participants.to_vec()));
        let play = |user: &String, id: &String| {
            let item = state
                .played
                .get(user)
                .and_then(|items| items.iter().find(|i| &i.provider_item_id == id));
            ProviderItemPlay {
                external_user_id: user.clone(),
                played: item.is_some_and(|i| i.played),
                last_played_at: item.and_then(|i| i.last_played_at),
            }
        };
        Ok(provider_item_ids
            .iter()
            .map(|id| {
                let participants: Vec<_> = participants.iter().map(|u| play(u, id)).collect();
                (
                    id.clone(),
                    ProviderItemRecheck {
                        history: participants.iter().filter(|p| p.played).cloned().collect(),
                        participants,
                    },
                )
            })
            .collect())
    }
}
