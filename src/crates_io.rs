//! Typed crates.io access; all requests, retries and caching live here.
//!
//! Verified 2026-09-06: <https://crates.io/data-access> requires an identifying
//! User-Agent and at most 1 request/second. This project also requires an HTTPS
//! repository URL and email contact in its User-Agent.
//! Routes from <https://github.com/rust-lang/crates.io/tree/main/src/controllers/krate>:
//! search GET /api/v1/crates?q=...&per_page=..., metadata
//! GET /api/v1/crates/{name}, versions GET /api/v1/crates/{name}/versions.
//! Versions are unpaginated unless per_page is supplied. Deliberately omit it
//! so the display cap cannot conceal an older matching semver requirement.

use crate::error::Error;
use reqwest::{Client, Url};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, HashMap},
    net::IpAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::{
    sync::Mutex,
    time::{Instant, sleep, sleep_until, timeout},
};

const SEARCH_TTL: Duration = Duration::from_secs(600);
const METADATA_TTL: Duration = Duration::from_secs(3600);
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
const MAX_CACHE_BYTES: usize = 32 * 1024 * 1024;
const MAX_CACHE_ENTRIES: usize = 128;
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(45);
pub const DEFAULT_USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("CARGO_PKG_REPOSITORY"),
    "; ",
    env!("CARGO_PKG_AUTHORS"),
    ")"
);

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub crates: Vec<CrateRecord>,
}

#[derive(Debug, Deserialize)]
pub struct CrateRecord {
    pub name: String,
    pub description: Option<String>,
    pub max_stable_version: Option<String>,
    pub max_version: String,
    pub downloads: u64,
    pub num_versions: u64,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MetadataResponse {
    #[serde(rename = "crate")]
    pub krate: CrateRecord,
    pub versions: Vec<VersionRecord>,
    pub keywords: Vec<Keyword>,
    pub categories: Vec<Category>,
}

#[derive(Debug, Deserialize)]
pub struct Keyword {
    pub keyword: String,
}
#[derive(Debug, Deserialize)]
pub struct Category {
    pub slug: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionRecord {
    pub num: String,
    pub yanked: bool,
    pub rust_version: Option<String>,
    pub created_at: String,
    pub license: Option<String>,
    pub features: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct VersionsResponse {
    pub versions: Vec<VersionRecord>,
    pub meta: VersionsMeta,
}

#[derive(Debug, Deserialize)]
pub struct VersionsMeta {
    pub total: u64,
    pub next_page: Option<String>,
}

#[derive(Debug)]
pub struct Fetched<T> {
    pub data: T,
    /// Unix seconds of successful upstream response; unchanged on cache hits.
    pub fetched_at: u64,
}

struct CacheEntry {
    body: Vec<u8>,
    fetched_at: u64,
    inserted: Instant,
    expires: Instant,
}

/// Capacity-one token bucket: unused time cannot accumulate burst tokens.
struct TokenBucket {
    next_token: Instant,
}

impl TokenBucket {
    fn new() -> Self {
        Self {
            next_token: Instant::now(),
        }
    }

    async fn take(&mut self) {
        sleep_until(self.next_token).await;
        self.next_token = Instant::now() + Duration::from_secs(1);
    }
}

struct State {
    bucket: TokenBucket,
    cache: HashMap<String, CacheEntry>,
}

impl State {
    fn prune(&mut self, incoming: usize) {
        let now = Instant::now();
        self.cache.retain(|_, entry| entry.expires > now);
        while self.cache.len() >= MAX_CACHE_ENTRIES
            || self.cache.values().map(|e| e.body.len()).sum::<usize>() + incoming > MAX_CACHE_BYTES
        {
            let oldest = self
                .cache
                .iter()
                .min_by_key(|(_, entry)| entry.inserted)
                .map(|(key, _)| key.clone());
            match oldest {
                Some(key) => {
                    self.cache.remove(&key);
                }
                None => break,
            }
        }
    }
}

#[derive(Clone)]
pub struct CratesIo {
    http: Client,
    base: Url,
    identity_ready: bool,
    state: Arc<Mutex<State>>,
}

impl CratesIo {
    pub fn from_env() -> Result<Self, Error> {
        let base =
            std::env::var("CRATES_IO_BASE_URL").unwrap_or_else(|_| "https://crates.io".into());
        let user_agent = std::env::var("MCP_CRATES_USER_AGENT")
            .unwrap_or_else(|_| DEFAULT_USER_AGENT.to_owned());
        Self::new(&base, Some(&user_agent))
    }

    /// Clones share one limiter and bounded cache.
    pub fn new(base: &str, user_agent: Option<&str>) -> Result<Self, Error> {
        let base = Url::parse(base)
            .map_err(|_| Error::configuration("CRATES_IO_BASE_URL", "must be an absolute URL"))?;
        let loopback = base
            .host_str()
            .and_then(|host| host.trim_matches(['[', ']']).parse::<IpAddr>().ok())
            .is_some_and(|ip| ip.is_loopback());
        if !(base.scheme() == "https"
            && base.host_str() == Some("crates.io")
            && base.port().is_none()
            || loopback && matches!(base.scheme(), "http" | "https"))
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || base.path() != "/"
        {
            return Err(Error::configuration(
                "CRATES_IO_BASE_URL",
                "must be https://crates.io or a numeric HTTP(S) loopback origin, without credentials, path, query or fragment",
            ));
        }
        let identity_ready = loopback
            || user_agent.is_some_and(|ua| {
                ua.contains("mcp-crates/") && ua.contains("https://") && ua.contains('@')
            });
        if user_agent.is_some() && !identity_ready {
            return Err(Error::configuration(
                "MCP_CRATES_USER_AGENT",
                "must identify mcp-crates/version and include an HTTPS project URL and contact email",
            ));
        }
        let ua = user_agent.unwrap_or(concat!(
            "mcp-crates/",
            env!("CARGO_PKG_VERSION"),
            " (offline-development)"
        ));
        let mut builder = Client::builder()
            .user_agent(ua)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10));
        if loopback {
            builder = builder.no_proxy();
        }
        let http = builder.build().map_err(|_| {
            Error::configuration(
                "MCP_CRATES_USER_AGENT",
                "invalid HTTP header or TLS client configuration",
            )
        })?;
        Ok(Self {
            http,
            base,
            identity_ready,
            state: Arc::new(Mutex::new(State {
                bucket: TokenBucket::new(),
                cache: HashMap::new(),
            })),
        })
    }

    pub async fn search(&self, query: &str, limit: u8) -> Result<Fetched<SearchResponse>, Error> {
        let mut url = self.endpoint("/api/v1/crates")?;
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("per_page", &limit.to_string());
        self.fetch(url, SEARCH_TTL, |_| Ok(())).await
    }

    pub async fn metadata(&self, name: &str) -> Result<Fetched<MetadataResponse>, Error> {
        validate_name(name)?;
        self.fetch(
            self.endpoint(&format!("/api/v1/crates/{name}"))?,
            METADATA_TTL,
            |response: &MetadataResponse| {
                if response.krate.num_versions != response.versions.len() as u64 {
                    return Err(Error::InvalidResponse(
                        "incomplete metadata version list".into(),
                    ));
                }
                Ok(())
            },
        )
        .await
    }

    pub async fn versions(&self, name: &str) -> Result<Fetched<VersionsResponse>, Error> {
        validate_name(name)?;
        self.fetch(
            self.endpoint(&format!("/api/v1/crates/{name}/versions"))?,
            METADATA_TTL,
            |response: &VersionsResponse| {
                if response.meta.next_page.is_some()
                    || response.meta.total != response.versions.len() as u64
                {
                    return Err(Error::InvalidResponse(
                        "incomplete version list; refusing partial semver resolution".into(),
                    ));
                }
                Ok(())
            },
        )
        .await
    }

    fn endpoint(&self, path: &str) -> Result<Url, Error> {
        self.base
            .join(path)
            .map_err(|_| Error::configuration("CRATES_IO_BASE_URL", "cannot construct endpoint"))
    }

    async fn fetch<T: DeserializeOwned>(
        &self,
        url: Url,
        ttl: Duration,
        validate: fn(&T) -> Result<(), Error>,
    ) -> Result<Fetched<T>, Error> {
        if !self.identity_ready {
            return Err(Error::configuration(
                "MCP_CRATES_USER_AGENT",
                "set mcp-crates/version (https://your-project-repository; contact@example.org) before live requests",
            ));
        }
        timeout(LOOKUP_TIMEOUT, self.fetch_inner(url, ttl, validate))
            .await
            .map_err(|_| Error::Timeout)?
    }

    async fn fetch_inner<T: DeserializeOwned>(
        &self,
        url: Url,
        ttl: Duration,
        validate: fn(&T) -> Result<(), Error>,
    ) -> Result<Fetched<T>, Error> {
        // Serialize misses to coalesce identical lookups. The deadline includes
        // lock acquisition, and cancellation drops the guard without poisoning.
        let mut state = self.state.lock().await;
        let key = url.to_string();
        if let Some(entry) = state
            .cache
            .get(&key)
            .filter(|entry| entry.expires > Instant::now())
        {
            let data = serde_json::from_slice(&entry.body)
                .map_err(|error| Error::InvalidResponse(error.to_string()))?;
            return Ok(Fetched {
                data,
                fetched_at: entry.fetched_at,
            });
        }
        let mut attempt = 1;
        loop {
            state.bucket.take().await;
            let mut response = self
                .http
                .get(url.clone())
                .send()
                .await
                .map_err(|error| Error::Network(format!("{:?}", error.without_url())))?;
            let status = response.status();
            if !status.is_success() {
                let error = Error::UpstreamStatus {
                    status: status.as_u16(),
                    attempts: attempt,
                };
                if status.as_u16() == 429 || status.is_server_error() {
                    let retry_after = response
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<u64>().ok());
                    let delay =
                        Duration::from_secs(retry_after.unwrap_or(0).max(1 << (attempt - 1)));
                    // Persist Retry-After cooldown even after the last attempt.
                    if let Some(until) = Instant::now().checked_add(delay) {
                        state.bucket.next_token = state.bucket.next_token.max(until);
                    }
                    if attempt < 3 && delay < LOOKUP_TIMEOUT {
                        drop(response);
                        sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                }
                return Err(error);
            }
            if response
                .content_length()
                .is_some_and(|size| size > MAX_BODY_BYTES as u64)
            {
                return Err(Error::ResponseTooLarge(MAX_BODY_BYTES));
            }
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|error| Error::Network(format!("{:?}", error.without_url())))?
            {
                if body.len() + chunk.len() > MAX_BODY_BYTES {
                    return Err(Error::ResponseTooLarge(MAX_BODY_BYTES));
                }
                body.extend_from_slice(&chunk);
            }
            let data = serde_json::from_slice(&body)
                .map_err(|error| Error::InvalidResponse(error.to_string()))?;
            validate(&data)?;
            let fetched_at = unix_seconds();
            state.prune(body.len());
            state.cache.insert(
                key,
                CacheEntry {
                    body,
                    fetched_at,
                    inserted: Instant::now(),
                    expires: Instant::now() + ttl,
                },
            );
            return Ok(Fetched { data, fetched_at });
        }
    }
}

pub fn validate_name(name: &str) -> Result<(), Error> {
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_alphabetic()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(Error::input(
            "name",
            "must be 1–64 ASCII letters, digits, hyphens or underscores and start with a letter",
        ));
    }
    Ok(())
}

pub fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn lookup_deadline_includes_waiting_for_shared_client() {
        let client = CratesIo::new("http://127.0.0.1:9", None).unwrap();
        let guard = client.state.lock().await;
        let start = Instant::now();
        assert!(matches!(client.metadata("itoa").await, Err(Error::Timeout)));
        assert_eq!(Instant::now() - start, LOOKUP_TIMEOUT);
        drop(guard);
        assert!(client.state.try_lock().is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn cached_snapshot_keeps_timestamp_until_expiry() {
        let client = CratesIo::new("http://127.0.0.1:9", None).unwrap();
        let key = "http://127.0.0.1:9/api/v1/crates?q=itoa&per_page=3".into();
        client.state.lock().await.cache.insert(
            key,
            CacheEntry {
                body: include_bytes!("../tests/fixtures/search_itoa.json").to_vec(),
                fetched_at: 42,
                inserted: Instant::now(),
                expires: Instant::now() + SEARCH_TTL,
            },
        );
        sleep(SEARCH_TTL - Duration::from_secs(1)).await;
        assert_eq!(client.search("itoa", 3).await.unwrap().fetched_at, 42);
        sleep(Duration::from_secs(1)).await;
        let mut state = client.state.lock().await;
        state.prune(0);
        assert!(state.cache.is_empty());
        state.cache.insert(
            "metadata".into(),
            CacheEntry {
                body: vec![],
                fetched_at: 123,
                inserted: Instant::now(),
                expires: Instant::now() + METADATA_TTL,
            },
        );
        sleep(SEARCH_TTL).await;
        state.prune(0);
        assert!(state.cache.contains_key("metadata"));
        sleep(METADATA_TTL - SEARCH_TTL).await;
        state.prune(0);
        assert!(state.cache.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn bucket_spaces_requests_and_does_not_accumulate_bursts() {
        let mut bucket = TokenBucket::new();
        bucket.take().await;
        let first = Instant::now();
        bucket.take().await;
        assert!(Instant::now() - first >= Duration::from_secs(1));
        sleep(Duration::from_secs(100)).await;
        bucket.take().await;
        let after_idle = Instant::now();
        bucket.take().await;
        assert!(Instant::now() - after_idle >= Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn cache_expiry_and_memory_bounds() {
        let mut state = State {
            bucket: TokenBucket::new(),
            cache: HashMap::new(),
        };
        for i in 0..MAX_CACHE_ENTRIES {
            state.cache.insert(
                i.to_string(),
                CacheEntry {
                    body: vec![0; 10],
                    fetched_at: 1,
                    inserted: Instant::now(),
                    expires: Instant::now() + SEARCH_TTL,
                },
            );
        }
        state.prune(10);
        assert_eq!(state.cache.len(), MAX_CACHE_ENTRIES - 1);
        sleep(SEARCH_TTL).await;
        state.prune(10);
        assert!(state.cache.is_empty());
        for i in 0..4 {
            state.cache.insert(
                i.to_string(),
                CacheEntry {
                    body: vec![0; MAX_BODY_BYTES],
                    fetched_at: 1,
                    inserted: Instant::now(),
                    expires: Instant::now() + METADATA_TTL,
                },
            );
        }
        state.prune(MAX_BODY_BYTES);
        assert!(
            state.cache.values().map(|e| e.body.len()).sum::<usize>() + MAX_BODY_BYTES
                <= MAX_CACHE_BYTES
        );
    }
}
