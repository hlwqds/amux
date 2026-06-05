//! Integration tests with insta snapshots for key types.

use amux::search_engine::SearchIndex;
use amux::stats::aggregate_daily;
use amux::types::Config;

#[test]
fn test_config_default_snapshot() {
    let config = Config::default();
    insta::assert_debug_snapshot!("config_default", config);
}

#[test]
fn test_search_index_empty_snapshot() {
    let index = SearchIndex::new();
    insta::assert_debug_snapshot!("search_index_empty", index);
}

#[test]
fn test_search_index_with_docs_snapshot() {
    let mut index = SearchIndex::new();
    index.add_document("session-1", "Fix login bug in auth module");
    index.add_document("session-2", "Add dark mode theme to TUI");
    index.add_document("session-3", "Refactor database connection pool");

    let results = index.search("auth login", 3);
    insta::assert_debug_snapshot!("search_index_auth_results", results);
}

#[test]
fn test_daily_stats_aggregation() {
    // Empty sessions -> empty stats
    let stats = aggregate_daily(&[]);
    assert!(stats.is_empty());
    insta::assert_debug_snapshot!("daily_stats_empty", stats);
}
