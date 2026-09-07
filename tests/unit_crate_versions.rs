#![forbid(unsafe_code)]
use mcp_crates::{
    crates_io::{Fetched, VersionsResponse},
    tools::crate_versions,
};

#[test]
fn recorded_versions_map_msrv_and_publication() {
    let data: VersionsResponse =
        serde_json::from_str(include_str!("fixtures/versions_itoa.json")).unwrap();
    let result = crate_versions::shape(Fetched {
        data,
        fetched_at: 42,
    })
    .unwrap();
    assert_eq!(result.versions[0].num, "1.0.18");
    assert_eq!(result.versions[0].msrv.as_deref(), Some("1.68"));
    assert!(!result.versions[0].published_at.is_empty());
    assert_eq!(result.fetched_at, 42);
}

#[test]
fn sorts_numerically_before_capping_and_preserves_yanks() {
    let mut data: VersionsResponse =
        serde_json::from_str(include_str!("fixtures/versions_itoa.json")).unwrap();
    let template = data.versions[0].clone();
    data.versions = (0..70)
        .map(|n| {
            let mut record = template.clone();
            record.num = format!("1.0.{n}");
            record.yanked = n == 69;
            record.rust_version = None;
            record
        })
        .collect();
    let result = crate_versions::shape(Fetched {
        data,
        fetched_at: 42,
    })
    .unwrap();
    assert_eq!(result.versions.len(), 50);
    assert_eq!(result.versions[0].num, "1.0.69");
    assert_eq!(result.versions[49].num, "1.0.20");
    assert!(result.versions[0].yanked);
    assert!(result.versions[0].msrv.is_none());
}
