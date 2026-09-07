#![forbid(unsafe_code)]
use mcp_crates::{
    crates_io::{Fetched, MetadataResponse},
    tools::crate_info,
};
use std::collections::BTreeMap;

fn fixture() -> MetadataResponse {
    serde_json::from_str(include_str!("fixtures/crate_itoa.json")).unwrap()
}
#[test]
fn shapes_recorded_metadata_and_selects_features_from_same_stable_version() {
    let mut data = fixture();
    let mut stable = data.versions[0].clone();
    stable.num = "10.0.0".into();
    stable.features = BTreeMap::from([("default".into(), vec!["std".into()])]);
    stable.license = Some("MIT".into());
    let mut prerelease = stable.clone();
    prerelease.num = "11.0.0-beta.1".into();
    prerelease.license = Some("Apache-2.0".into());
    let mut yanked = prerelease.clone();
    yanked.num = "12.0.0".into();
    yanked.yanked = true;
    data.versions.extend([yanked, prerelease, stable]);
    let result = crate_info::shape(Fetched {
        data,
        fetched_at: 123,
    })
    .unwrap();
    assert_eq!(result.name, "itoa");
    assert_eq!(result.latest_version, "10.0.0");
    assert_eq!(result.license.as_deref(), Some("MIT"));
    assert_eq!(result.features["default"], ["std"]);
    assert_eq!(result.yanked_versions[0], "12.0.0");
    assert_eq!(result.fetched_at, 123);
}

#[test]
fn prerelease_fallback_and_all_yanked_and_oversized_features() {
    let mut data = fixture();
    data.versions.truncate(1);
    data.versions[0].num = "1.0.0-rc.1".into();
    assert_eq!(
        crate_info::shape(Fetched {
            data,
            fetched_at: 1
        })
        .unwrap()
        .latest_version,
        "1.0.0-rc.1"
    );
    let mut data = fixture();
    for version in &mut data.versions {
        version.yanked = true;
    }
    assert!(
        crate_info::shape(Fetched {
            data,
            fetched_at: 1
        })
        .is_err()
    );
    let mut data = fixture();
    data.versions[0]
        .features
        .insert("large".into(), vec!["dep".into(); 257]);
    assert!(
        crate_info::shape(Fetched {
            data,
            fetched_at: 1
        })
        .is_err()
    );
}
