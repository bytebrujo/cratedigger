#![forbid(unsafe_code)]
use mcp_crates::{
    crates_io::{Fetched, VersionsResponse},
    tools::resolve_version,
};
use semver::VersionReq;

fn resolve(req: &str, versions: &[(&str, bool)]) -> Option<String> {
    let mut data: VersionsResponse =
        serde_json::from_str(include_str!("fixtures/versions_itoa.json")).unwrap();
    let template = data.versions[0].clone();
    data.versions = versions
        .iter()
        .map(|(num, yanked)| {
            let mut record = template.clone();
            record.num = (*num).into();
            record.yanked = *yanked;
            record
        })
        .collect();
    let result = resolve_version::shape(
        Fetched {
            data,
            fetched_at: 42,
        },
        &VersionReq::parse(req).unwrap(),
    )
    .unwrap();
    assert!(result.req_valid);
    assert_eq!(result.fetched_at, Some(42));
    result.resolved
}

#[test]
fn cargo_requirements_yanks_and_prereleases() {
    let versions = [
        ("1.0.0", false),
        ("1.2.0", false),
        ("1.2.8", false),
        ("1.2.9", true),
        ("1.3.0", false),
        ("2.0.0", false),
        ("3.0.0-beta.1", false),
    ];
    for (req, expected) in [
        ("^1.0", Some("1.3.0")),
        ("~1.2", Some("1.2.8")),
        ("=1.2.9", None),
        ("1.2.*", Some("1.2.8")),
        (">=1.1, <1.3", Some("1.2.8")),
        ("*", Some("2.0.0")),
        ("^3.0.0-beta.1", Some("3.0.0-beta.1")),
        (">=4", None),
    ] {
        assert_eq!(resolve(req, &versions).as_deref(), expected, "{req}");
    }
    assert_eq!(
        resolve("^0.2.1", &[("0.2.8", false), ("0.3.0", false)]).as_deref(),
        Some("0.2.8")
    );
}

#[test]
fn old_matches_are_not_hidden_by_the_display_cap() {
    let versions: Vec<_> = (0..100).map(|n| (format!("1.0.{n}"), false)).collect();
    let refs: Vec<_> = versions
        .iter()
        .map(|(version, yanked)| (version.as_str(), *yanked))
        .collect();
    assert_eq!(resolve("=1.0.1", &refs).as_deref(), Some("1.0.1"));
}

#[test]
fn recorded_version_resolution_and_invalid_upstream_semver() {
    let parse = || {
        serde_json::from_str::<VersionsResponse>(include_str!("fixtures/versions_itoa.json"))
            .unwrap()
    };
    let result = resolve_version::shape(
        Fetched {
            data: parse(),
            fetched_at: 1,
        },
        &VersionReq::parse("^1").unwrap(),
    )
    .unwrap();
    assert_eq!(result.resolved.as_deref(), Some("1.0.18"));
    let mut data = parse();
    data.versions[0].num = "invalid".into();
    assert!(
        resolve_version::shape(
            Fetched {
                data,
                fetched_at: 1
            },
            &VersionReq::STAR
        )
        .is_err()
    );
}
