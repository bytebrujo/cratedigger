#![forbid(unsafe_code)]
pub mod support;

#[test]
fn fixture_server_waits_for_fragmented_request_headers() {
    use std::{
        io::{Read, Write},
        net::TcpStream,
    };
    let mock = Mock::fixtures();
    let mut stream = TcpStream::connect(mock.url.trim_start_matches("http://")).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .write_all(b"GET /api/v1/crates/itoa HTTP/1.1\r\n")
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    stream.write_all(b"Host: 127.0.0.1\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    assert_eq!(mock.requests().len(), 1);
}

#[tokio::test]
async fn incomplete_version_lists_are_not_cached_or_resolved() {
    let mock = Mock::start(|_, index| {
        if index == 0 {
            let mut response: serde_json::Value =
                serde_json::from_str(include_str!("fixtures/versions_itoa.json")).unwrap();
            response["versions"].as_array_mut().unwrap().truncate(1);
            Reply::json(response.to_string())
        } else {
            Reply::json(include_str!("fixtures/versions_itoa.json"))
        }
    });
    let client = CratesIo::new(&mock.url, Some(TEST_USER_AGENT)).unwrap();
    assert!(matches!(
        client.versions("itoa").await,
        Err(Error::InvalidResponse(_))
    ));
    assert_eq!(
        client.versions("itoa").await.unwrap().data.versions.len(),
        37
    );
    assert_eq!(mock.requests().len(), 2);
}

#[tokio::test]
async fn nonempty_pagination_marker_is_rejected_even_if_totals_match() {
    let mock = Mock::start(|_, _| {
        let mut response: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/versions_itoa.json")).unwrap();
        response["meta"]["next_page"] = serde_json::json!("?page=2");
        Reply::json(response.to_string())
    });
    let client = CratesIo::new(&mock.url, Some(TEST_USER_AGENT)).unwrap();
    assert!(matches!(
        client.versions("itoa").await,
        Err(Error::InvalidResponse(_))
    ));
}

use mcp_crates::{crates_io::CratesIo, error::Error};
use std::time::{Duration, Instant};
use support::{Mock, Reply, TEST_USER_AGENT};

#[tokio::test]
async fn concurrent_cache_misses_are_coalesced_and_query_is_encoded() {
    let mock = Mock::fixtures();
    let client = CratesIo::new(&mock.url, Some(TEST_USER_AGENT)).unwrap();
    let (first, second, third) = tokio::join!(
        client.search("itoa & unicode λ", 3),
        client.search("itoa & unicode λ", 3),
        client.metadata("itoa")
    );
    assert_eq!(first.unwrap().fetched_at, second.unwrap().fetched_at);
    third.unwrap();
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].path.contains("q=itoa+%26+unicode+%CE%BB"));
    assert!(requests[1].at - requests[0].at >= Duration::from_millis(950));
}

#[tokio::test]
async fn retries_back_off_and_stop_after_three_attempts() {
    let mock = Mock::start(|_, _| Reply::status(503));
    let client = CratesIo::new(&mock.url, Some(TEST_USER_AGENT)).unwrap();
    assert!(matches!(
        client.versions("itoa").await,
        Err(Error::UpstreamStatus {
            status: 503,
            attempts: 3
        })
    ));
    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests[1].at - requests[0].at >= Duration::from_millis(950));
    assert!(requests[2].at - requests[1].at >= Duration::from_millis(1950));
}

#[tokio::test]
async fn honors_retry_after_then_caches_success() {
    let mock = Mock::start(|_, index| {
        if index == 0 {
            let mut reply = Reply::status(429);
            reply.headers.push(("Retry-After".into(), "2".into()));
            reply
        } else {
            Reply::json(include_str!("fixtures/versions_itoa.json"))
        }
    });
    let client = CratesIo::new(&mock.url, Some(TEST_USER_AGENT)).unwrap();
    let first = client.versions("itoa").await.unwrap();
    let second = client.versions("itoa").await.unwrap();
    assert_eq!(first.fetched_at, second.fetched_at);
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].at - requests[0].at >= Duration::from_millis(1950));
}

#[tokio::test]
async fn terminal_429_errors_are_structured() {
    let mock = Mock::start(|_, _| Reply::status(429));
    let client = CratesIo::new(&mock.url, Some(TEST_USER_AGENT)).unwrap();
    let error = client.versions("itoa").await.unwrap_err();
    assert!(matches!(
        error,
        Error::UpstreamStatus {
            status: 429,
            attempts: 3
        }
    ));
    let result = error.tool_result();
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.unwrap()["error"]["retryable"],
        true
    );
    assert_eq!(mock.requests().len(), 3);
}

#[tokio::test]
async fn not_found_and_redirects_do_not_retry_or_follow() {
    for status in [404, 302] {
        let destination = Mock::fixtures();
        let target = destination.url.clone();
        let mock = Mock::start(move |_, _| {
            let mut reply = Reply::status(status);
            reply
                .headers
                .push(("Location".into(), format!("{target}/api/v1/crates/itoa")));
            reply
        });
        let client = CratesIo::new(&mock.url, Some(TEST_USER_AGENT)).unwrap();
        assert!(matches!(
            client.metadata("itoa").await,
            Err(Error::UpstreamStatus { attempts: 1, .. })
        ));
        assert_eq!(mock.requests().len(), 1);
        assert!(destination.requests().is_empty());
    }
}

#[tokio::test]
async fn oversized_responses_are_rejected_before_reading_body() {
    let mock = Mock::start(|_, _| {
        let mut reply = Reply::json("");
        reply
            .headers
            .push(("Content-Length".into(), "8388609".into()));
        reply
    });
    let client = CratesIo::new(&mock.url, Some(TEST_USER_AGENT)).unwrap();
    assert!(matches!(
        client.metadata("itoa").await,
        Err(Error::ResponseTooLarge(_))
    ));
    assert_eq!(mock.requests().len(), 1);
}

#[tokio::test]
async fn stalled_response_has_a_deadline() {
    let mock = Mock::start(|_, _| {
        let mut reply = Reply::json("{}");
        reply.delay = Duration::from_secs(20);
        reply
    });
    let client = CratesIo::new(&mock.url, Some(TEST_USER_AGENT)).unwrap();
    let start = Instant::now();
    assert!(matches!(
        client.metadata("itoa").await,
        Err(Error::Network(_))
    ));
    assert!(start.elapsed() < Duration::from_secs(15));
}

#[tokio::test]
async fn production_needs_identity_and_test_urls_cannot_escape_loopback() {
    let client = CratesIo::new("https://crates.io", None).unwrap();
    assert!(matches!(
        client.metadata("itoa").await,
        Err(Error::Configuration { .. })
    ));
    for base in [
        "http://crates.io",
        "https://example.com",
        "http://127.0.0.1/private",
        "http://user@127.0.0.1",
    ] {
        assert!(
            CratesIo::new(base, Some(TEST_USER_AGENT)).is_err(),
            "{base}"
        );
    }
    assert!(CratesIo::new("https://crates.io", Some("reqwest")).is_err());
}
