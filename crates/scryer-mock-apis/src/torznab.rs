//! A Torznab indexer for the flow harness: `t=caps` plus one RSS feed per
//! search type, from synthetic fixtures, recording every request.
// fork (D91 F2)

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tokio::net::TcpListener;

const CAPS: &str = include_str!("../../../tests/fixtures/flow/torznab/caps.xml");
const SEARCH: &str = include_str!("../../../tests/fixtures/flow/torznab/search.xml");
const TVSEARCH: &str = include_str!("../../../tests/fixtures/flow/torznab/tvsearch.xml");
const MOVIE: &str = include_str!("../../../tests/fixtures/flow/torznab/movie.xml");
/// A minimal valid single-file torrent, served for the enclosure links.
const STUB_TORRENT: &[u8] =
    b"d4:infod6:lengthi4e4:name8:stub.mkv12:piece lengthi16384e6:pieces20:AAAAAAAAAAAAAAAAAAAAee";

/// One request the fake received: the `t` value and every query parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedQuery {
    pub t: String,
    pub params: BTreeMap<String, String>,
}

impl RecordedQuery {
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(String::as_str)
    }
}

#[derive(Default)]
struct Shared {
    feeds: Mutex<BTreeMap<String, String>>,
    queries: Mutex<Vec<RecordedQuery>>,
    base: Mutex<String>,
    /// When set, every request answers 401 (bad key) or this status.
    fail_status: Mutex<Option<u16>>,
}

/// A running fake. Dropping it aborts the server.
pub struct TorznabFake {
    shared: Arc<Shared>,
    base_url: String,
    task: tokio::task::JoinHandle<()>,
}

impl TorznabFake {
    /// Bind `127.0.0.1:0` and serve. Point the indexer at `api_url()`.
    pub async fn start() -> Self {
        let shared = Arc::new(Shared::default());
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind torznab fake");
        let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
        *shared.base.lock().unwrap() = base_url.clone();
        let app = Router::new()
            .route("/api", get(api))
            .route("/dl/{name}", get(download))
            .with_state(shared.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        Self {
            shared,
            base_url,
            task,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The URL an indexer config uses as its base (`{base}/api`).
    pub fn api_url(&self) -> String {
        format!("{}/api", self.base_url)
    }

    /// Replace the feed served for `t` (`search`, `tvsearch`, `movie`). `{BASE}`
    /// in the body becomes the fake's base URL.
    pub fn set_feed(&self, t: &str, xml: impl Into<String>) {
        self.shared
            .feeds
            .lock()
            .unwrap()
            .insert(t.to_string(), xml.into());
    }

    /// Answer every request with `status` (e.g. 429, 503) until cleared.
    pub fn fail_with(&self, status: Option<u16>) {
        *self.shared.fail_status.lock().unwrap() = status;
    }

    /// Every `/api` request so far, oldest first.
    pub fn queries(&self) -> Vec<RecordedQuery> {
        self.shared.queries.lock().unwrap().clone()
    }

    /// Only the search requests (everything but `t=caps`).
    pub fn search_queries(&self) -> Vec<RecordedQuery> {
        self.queries()
            .into_iter()
            .filter(|q| q.t != "caps")
            .collect()
    }

    pub fn clear_queries(&self) {
        self.shared.queries.lock().unwrap().clear();
    }
}

impl Drop for TorznabFake {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn api(
    State(shared): State<Arc<Shared>>,
    Query(params): Query<BTreeMap<String, String>>,
) -> Response {
    let t = params.get("t").cloned().unwrap_or_default();
    shared.queries.lock().unwrap().push(RecordedQuery {
        t: t.clone(),
        params: params.clone(),
    });
    if let Some(status) = *shared.fail_status.lock().unwrap() {
        return StatusCode::from_u16(status)
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            .into_response();
    }
    if params.get("apikey").is_none_or(|key| key.is_empty()) && t != "caps" {
        return (
            StatusCode::UNAUTHORIZED,
            xml_error(100, "Incorrect user credentials"),
        )
            .into_response();
    }
    let body = match t.as_str() {
        "caps" => CAPS.to_string(),
        "search" | "tvsearch" | "movie" => shared
            .feeds
            .lock()
            .unwrap()
            .get(&t)
            .cloned()
            .unwrap_or_else(|| default_feed(&t).to_string()),
        _ => return (StatusCode::BAD_REQUEST, xml_error(202, "No such function")).into_response(),
    };
    let body = body.replace("{BASE}", &shared.base.lock().unwrap());
    ([(header::CONTENT_TYPE, "application/xml")], body).into_response()
}

fn default_feed(t: &str) -> &'static str {
    match t {
        "tvsearch" => TVSEARCH,
        "movie" => MOVIE,
        _ => SEARCH,
    }
}

fn xml_error(code: u16, description: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><error code="{code}" description="{description}"/>"#
    )
}

async fn download(Path(_name): Path<String>) -> Response {
    (
        [(header::CONTENT_TYPE, "application/x-bittorrent")],
        STUB_TORRENT,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Minimal HTTP/1.0 GET, so the unit tests need no client dependency.
    async fn get(fake: &TorznabFake, path_and_query: &str) -> (u16, String) {
        let addr = fake.base_url().trim_start_matches("http://").to_string();
        let mut stream = tokio::net::TcpStream::connect(&addr).await.unwrap();
        stream
            .write_all(format!("GET {path_and_query} HTTP/1.0\r\nHost: {addr}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.unwrap();
        let text = String::from_utf8_lossy(&raw).into_owned();
        let status = text.split_whitespace().nth(1).unwrap().parse().unwrap();
        let body = text
            .split_once("\r\n\r\n")
            .map(|(_, b)| b.to_string())
            .unwrap_or_default();
        (status, body)
    }

    #[tokio::test]
    async fn caps_lists_search_modes_and_categories() {
        let fake = TorznabFake::start().await;
        let (status, body) = get(&fake, "/api?t=caps").await;
        assert_eq!(status, 200);
        assert!(body.contains("<tv-search available=\"yes\""));
        assert!(body.contains("<subcat id=\"5040\""));
    }

    #[tokio::test]
    async fn tvsearch_serves_feed_and_records_query() {
        let fake = TorznabFake::start().await;
        let (status, body) = get(
            &fake,
            "/api?t=tvsearch&apikey=k&q=Made+Up+Show&season=1&ep=2&cat=5030,5040",
        )
        .await;
        assert_eq!(status, 200);
        assert!(body.contains("Made.Up.Show.S01E02.1080p.WEB-DL"));
        assert!(!body.contains("{BASE}"));
        assert!(body.contains(&format!("{}/dl/", fake.base_url())));
        let queries = fake.search_queries();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].t, "tvsearch");
        assert_eq!(queries[0].param("cat"), Some("5030,5040"));
        assert_eq!(queries[0].param("season"), Some("1"));
        assert_eq!(queries[0].param("q"), Some("Made Up Show"));
    }

    #[tokio::test]
    async fn set_feed_overrides_and_missing_key_is_refused() {
        let fake = TorznabFake::start().await;
        fake.set_feed("movie", "<rss><channel/></rss>");
        let (_, body) = get(&fake, "/api?t=movie&apikey=k").await;
        assert_eq!(body, "<rss><channel/></rss>");
        let (status, _) = get(&fake, "/api?t=movie").await;
        assert_eq!(status, 401);
        fake.fail_with(Some(503));
        assert_eq!(get(&fake, "/api?t=search&apikey=k").await.0, 503);
        assert_eq!(fake.queries().len(), 3);
    }

    #[tokio::test]
    async fn enclosure_download_serves_a_torrent_stub() {
        let fake = TorznabFake::start().await;
        let (status, body) = get(&fake, "/dl/abc.torrent").await;
        assert_eq!(status, 200);
        assert!(body.starts_with("d4:info"));
    }
}
