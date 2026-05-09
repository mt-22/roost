/// Real fuzzy matching engine with character-skipping and scoring.
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub index: usize,
    pub score: i32,
    /// Character positions in the candidate that matched the query.
    pub positions: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct FuzzyEngine {
    query: String,
    matches: Vec<FuzzyMatch>,
    cursor: usize,
}

impl Default for FuzzyEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzyEngine {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            matches: Vec::new(),
            cursor: 0,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }

    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn backspace(&mut self) {
        self.query.pop();
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.matches.clear();
        self.cursor = 0;
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.matches.is_empty() && self.cursor < self.matches.len() - 1 {
            self.cursor += 1;
        }
    }

    pub fn matches(&self) -> &[FuzzyMatch] {
        &self.matches
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Returns the original index of the currently selected match.
    pub fn selected_index(&self) -> Option<usize> {
        self.matches.get(self.cursor).map(|m| m.index)
    }

    /// Filter `items` using real fuzzy character-skipping matching.
    /// Results are stored sorted by score descending.
    pub fn filter<T: AsRef<str>>(&mut self, items: &[T]) {
        let query_chars: Vec<char> = self.query.to_lowercase().chars().collect();
        if query_chars.is_empty() {
            self.matches = items
                .iter()
                .enumerate()
                .map(|(i, _)| FuzzyMatch {
                    index: i,
                    score: 0,
                    positions: Vec::new(),
                })
                .collect();
            self.cursor = 0;
            return;
        }

        let mut results = Vec::new();

        for (i, item) in items.iter().enumerate() {
            let candidate: Vec<char> = item.as_ref().to_lowercase().chars().collect();
            if let Some((score, positions)) = fuzzy_match(&query_chars, &candidate) {
                results.push(FuzzyMatch {
                    index: i,
                    score,
                    positions,
                });
            }
        }

        // Sort by score descending; stable sort preserves original order for ties
        results.sort_by(|a, b| b.score.cmp(&a.score));

        self.matches = results;
        if self.cursor >= self.matches.len() && !self.matches.is_empty() {
            self.cursor = self.matches.len() - 1;
        } else if self.matches.is_empty() {
            self.cursor = 0;
        }
    }
}

/// Try to match all query chars in order within candidate.
/// Returns (score, matched_positions) on success, None if no match.
fn fuzzy_match(query: &[char], candidate: &[char]) -> Option<(i32, Vec<usize>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }
    if candidate.len() < query.len() {
        return None;
    }

    let mut positions = Vec::with_capacity(query.len());
    let mut candidate_idx = 0;

    for &qc in query {
        let mut found = false;
        while candidate_idx < candidate.len() {
            if candidate[candidate_idx] == qc {
                positions.push(candidate_idx);
                candidate_idx += 1;
                found = true;
                break;
            }
            candidate_idx += 1;
        }
        if !found {
            return None;
        }
    }

    let score = compute_score(query, candidate, &positions);
    Some((score, positions))
}

fn compute_score(_query: &[char], candidate: &[char], positions: &[usize]) -> i32 {
    let mut score = 0i32;

    for (i, &pos) in positions.iter().enumerate() {
        // +10 for each matched char
        score += 10;

        // +5 bonus if at word boundary
        if is_word_boundary(candidate, pos) {
            score += 5;
        }

        // +3 bonus for consecutive matches
        if i > 0 && pos == positions[i - 1] + 1 {
            score += 3;
        }
    }

    // Penalize distance from start
    if let Some(&first) = positions.first() {
        score -= first as i32;
    }

    score
}

fn is_word_boundary(candidate: &[char], pos: usize) -> bool {
    if pos == 0 {
        return true;
    }
    let prev = candidate[pos - 1];
    matches!(prev, '/' | '.' | '_' | '-' | ' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_empty_query_returns_all() {
        let mut engine = FuzzyEngine::new();
        let items = vec!["nvim", "vim", "alacritty"];
        engine.filter(&items);
        assert_eq!(engine.match_count(), 3);
        assert_eq!(engine.selected_index(), Some(0));
    }

    #[test]
    fn filter_finds_fuzzy_matches() {
        let mut engine = FuzzyEngine::new();
        let items = vec!["nvim", "vim", "alacritty", "zsh"];
        engine.push_char('v');
        engine.push_char('i');
        engine.filter(&items);
        // "vim" and "nvim" both match, "vim" should score higher (starts at boundary)
        assert!(engine.match_count() >= 2);
        let indices: Vec<usize> = engine.matches().iter().map(|m| m.index).collect();
        assert!(indices.contains(&0)); // nvim
        assert!(indices.contains(&1)); // vim
    }

    #[test]
    fn filter_case_insensitive() {
        let mut engine = FuzzyEngine::new();
        let items = vec!["Nvim", "VIM", "Alacritty"];
        engine.push_char('v');
        engine.push_char('i');
        engine.filter(&items);
        assert_eq!(engine.match_count(), 2);
    }

    #[test]
    fn filter_skips_chars() {
        let mut engine = FuzzyEngine::new();
        let items = vec!["alacritty", "kitty", "wezterm"];
        engine.push_char('a');
        engine.push_char('t');
        engine.push_char('y');
        engine.filter(&items);
        // "alacritty" matches a..t..y (skips chars)
        assert!(engine.match_count() >= 1);
        assert_eq!(engine.matches()[0].index, 0); // alacritty
    }

    #[test]
    fn word_boundary_bonus_scores_higher() {
        let mut engine = FuzzyEngine::new();
        let items = vec!["nvim", "vim"];
        engine.push_char('v');
        engine.push_char('i');
        engine.filter(&items);
        // "vim" starts at boundary, should score higher than "nvim"
        assert_eq!(engine.match_count(), 2);
        assert_eq!(engine.matches()[0].index, 1); // vim first
    }

    #[test]
    fn consecutive_bonus_scores_higher() {
        let mut engine = FuzzyEngine::new();
        let items = vec!["vxxim", "vim"];
        engine.push_char('v');
        engine.push_char('i');
        engine.push_char('m');
        engine.filter(&items);
        // "vim" has consecutive matches, should score higher than "vxxim"
        assert_eq!(engine.match_count(), 2);
        assert_eq!(engine.matches()[0].index, 1); // vim first
    }

    #[test]
    fn cursor_bounds() {
        let mut engine = FuzzyEngine::new();
        let items = vec!["a", "b", "c", "d"];
        engine.filter(&items);
        assert_eq!(engine.cursor(), 0);

        engine.move_down();
        assert_eq!(engine.cursor(), 1);
        engine.move_down();
        engine.move_down();
        assert_eq!(engine.cursor(), 3);
        engine.move_down();
        assert_eq!(engine.cursor(), 3); // stays at max

        engine.move_up();
        engine.move_up();
        engine.move_up();
        engine.move_up();
        assert_eq!(engine.cursor(), 0); // stays at 0
    }

    #[test]
    fn no_matches_cursor_zero() {
        let mut engine = FuzzyEngine::new();
        let items = vec!["nvim", "vim"];
        engine.push_char('z');
        engine.push_char('z');
        engine.push_char('z');
        engine.filter(&items);
        assert!(engine.matches().is_empty());
        assert_eq!(engine.cursor(), 0);
        assert_eq!(engine.selected_index(), None);
    }

    #[test]
    fn backspace_updates_filter() {
        let mut engine = FuzzyEngine::new();
        let items = vec!["nvim", "vim", "alacritty"];
        engine.push_char('v');
        engine.push_char('i');
        engine.push_char('m');
        engine.filter(&items);
        assert_eq!(engine.match_count(), 2); // nvim, vim
        engine.backspace();
        engine.filter(&items);
        assert_eq!(engine.match_count(), 2); // nvim, vim
    }

    #[test]
    fn clear_resets_engine() {
        let mut engine = FuzzyEngine::new();
        let items = vec!["nvim", "vim"];
        engine.push_char('v');
        engine.filter(&items);
        assert_eq!(engine.match_count(), 2);
        engine.clear();
        assert!(!engine.is_active());
        assert_eq!(engine.match_count(), 0);
    }
}
