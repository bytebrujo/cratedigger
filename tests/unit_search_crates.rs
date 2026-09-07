#![forbid(unsafe_code)]
use mcp_crates::{
    crates_io::{Fetched, SearchResponse},
    tools::search_crates,
};
#[test]
fn shapes_recorded_search_and_enforces_cap() {
    let parse = || {
        serde_json::from_str::<SearchResponse>(include_str!("fixtures/search_itoa.json")).unwrap()
    };
    let response = search_crates::shape(
        Fetched {
            data: parse(),
            fetched_at: 123,
        },
        1,
    )
    .unwrap();
    assert_eq!(response.crates.len(), 1);
    assert_eq!(response.crates[0].name, "itoa");
    assert_eq!(response.crates[0].latest_version, "1.0.18");
    assert_eq!(response.fetched_at, 123);
    let mut data = parse();
    data.crates[0].max_stable_version = None;
    data.crates[0].max_version = "2.0.0-beta.1".into();
    assert_eq!(
        search_crates::shape(
            Fetched {
                data,
                fetched_at: 0
            },
            1
        )
        .unwrap()
        .crates[0]
            .latest_version,
        "2.0.0-beta.1"
    );
}
