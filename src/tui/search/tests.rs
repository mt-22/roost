use super::*;

fn test_items() -> Vec<(&'static str, usize)> {
    vec![("nvim", 0), ("neovim", 1), ("ghostty", 2)]
}

#[test]
fn test_fuzzy_match_exact() {
    assert!(fuzzy_match("nvim", "nvim"));
}

#[test]
fn test_fuzzy_match_prefix() {
    assert!(fuzzy_match("nvim", "nv"));
}

#[test]
fn test_fuzzy_match_subsequence() {
    assert!(fuzzy_match("nvim", "nvm"));
}

#[test]
fn test_fuzzy_match_out_of_order_fails() {
    assert!(!fuzzy_match("ivm", "nvim"));
}

#[test]
fn test_fuzzy_match_empty_query() {
    assert!(fuzzy_match("anything", ""));
}

#[test]
fn test_fuzzy_match_query_longer_than_text_fails() {
    assert!(!fuzzy_match("short", "verylongquery"));
}

#[test]
fn test_fuzzy_match_case_insensitive_in_rebuild() {
    let items: Vec<(&str, usize)> = vec![("NVIM", 0), ("nvim", 1)];
    let mut state = SearchState::new();
    state.query = "nvim".to_string();
    state.rebuild(&items);
    assert_eq!(state.result_count(), 2);
}

#[test]
fn test_search_state_rebuild_filters() {
    let items = test_items();
    let mut state = SearchState::new();
    state.query = "nv".to_string();
    state.rebuild(&items);
    assert!(state.result_count() > 0);
    assert!(state.result_count() < items.len());
}

#[test]
fn test_search_state_push_extends_query() {
    let items = test_items();
    let mut state = SearchState::new();
    state.push('n', &items);
    assert_eq!(state.query, "n");
    state.push('v', &items);
    assert_eq!(state.query, "nv");
}

#[test]
fn test_search_state_pop_removes_last() {
    let items = test_items();
    let mut state = SearchState::new();
    state.query = "nv".to_string();
    let still_active = state.pop(&items);
    assert_eq!(state.query, "n");
    assert!(still_active);
    let still_active = state.pop(&items);
    assert_eq!(state.query, "");
    assert!(!still_active);
}

#[test]
fn test_search_state_move_up_down() {
    let items = test_items();
    let mut state = SearchState::new();
    state.rebuild(&items);
    state.cursor = 1;
    state.move_up();
    assert_eq!(state.cursor, 0);
    state.move_down();
    assert!(state.cursor >= 1);
}

#[test]
fn test_search_state_new_is_empty() {
    let state = SearchState::new();
    assert_eq!(state.result_count(), 0);
    assert!(state.query.is_empty());
    assert_eq!(state.cursor, 0);
}
