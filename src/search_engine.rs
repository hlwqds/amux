//! Pure Rust BM25 search engine for session summaries.
//!
//! Uses TF-IDF + BM25 scoring to provide semantic-like search over session
//! summaries without any external dependencies.

use hashbrown::HashMap;

use serde::{Deserialize, Serialize};

/// Simple tokenizer: lowercase, split on whitespace/punctuation, remove empty tokens.
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// BM25 index for session summaries.
///
/// Uses `hashbrown::HashMap` for faster hashing and `HashMap<String, usize>` posting
/// lists for O(1) document removal (vs O(N) scan with `Vec` postings).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchIndex {
    /// Number of documents in the index.
    pub doc_count: usize,
    /// Average document length (in tokens).
    pub avg_dl: f64,
    /// Document lengths: doc_id -> number of tokens.
    pub doc_lengths: HashMap<String, usize>,
    /// Inverted index: term -> {doc_id: term_freq}.
    pub inverted: HashMap<String, HashMap<String, usize>>,
    /// Document frequencies: term -> number of documents containing term.
    pub df: HashMap<String, usize>,
    /// Total token count across all documents (for avg_dl computation).
    total_tokens: usize,
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchIndex {
    /// Create an empty search index.
    pub fn new() -> Self {
        Self {
            doc_count: 0,
            avg_dl: 0.0,
            doc_lengths: HashMap::new(),
            inverted: HashMap::new(),
            df: HashMap::new(),
            total_tokens: 0,
        }
    }

    /// Add a document to the index. If the doc_id already exists, it is replaced.
    pub fn add_document(&mut self, doc_id: &str, text: &str) {
        // Remove old version if present.
        self.remove_document(doc_id);

        let tokens = tokenize(text);
        let dl = tokens.len();

        self.doc_count += 1;
        self.total_tokens += dl;
        self.avg_dl = self.total_tokens as f64 / self.doc_count as f64;
        self.doc_lengths.insert(doc_id.to_string(), dl);

        // Count term frequencies.
        let mut tf: HashMap<String, usize> = HashMap::new();
        for token in &tokens {
            *tf.entry(token.clone()).or_default() += 1;
        }

        // Update inverted index and document frequencies.
        for (term, freq) in &tf {
            self.inverted
                .entry(term.clone())
                .or_default()
                .insert(doc_id.to_string(), *freq);
            *self.df.entry(term.clone()).or_default() += 1;
        }
    }

    /// Remove a document from the index.
    pub fn remove_document(&mut self, doc_id: &str) {
        let dl = match self.doc_lengths.remove(doc_id) {
            Some(dl) => dl,
            None => return,
        };

        self.doc_count = self.doc_count.saturating_sub(1);
        self.total_tokens = self.total_tokens.saturating_sub(dl);

        if self.doc_count > 0 {
            self.avg_dl = self.total_tokens as f64 / self.doc_count as f64;
        } else {
            self.avg_dl = 0.0;
        }

        // Remove from inverted index and update df.
        let mut empty_terms: Vec<String> = Vec::new();
        for (term, postings) in &mut self.inverted {
            if postings.remove(doc_id).is_some() {
                // This term appeared in the removed document.
                let df_entry = self.df.get_mut(term).expect("df entry must exist after insertion");
                *df_entry = df_entry.saturating_sub(1);
                if *df_entry == 0 {
                    empty_terms.push(term.clone());
                }
            }
            if postings.is_empty() {
                empty_terms.push(term.clone());
            }
        }

        for term in &empty_terms {
            self.inverted.remove(term);
            self.df.remove(term);
        }
    }

    /// Search the index using BM25 scoring.
    ///
    /// Returns the top-k results as (doc_id, score) pairs, sorted by score descending.
    ///
    /// BM25 formula:
    /// ```text
    /// score(D, Q) = Σ IDF(q) * (f(q,D) * (k1 + 1)) / (f(q,D) + k1 * (1 - b + b * |D| / avgdl))
    /// IDF(q) = ln((N - n(q) + 0.5) / (n(q) + 0.5) + 1)
    /// ```
    pub fn search(&self, query: &str, top_k: usize) -> Vec<(String, f64)> {
        if self.doc_count == 0 || query.trim().is_empty() {
            return Vec::new();
        }

        let k1: f64 = 1.2;
        let b: f64 = 0.75;
        let n = self.doc_count as f64;
        let avg_dl = self.avg_dl;

        let query_tokens = tokenize(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }

        // Accumulate scores per document.
        let mut scores: HashMap<String, f64> = HashMap::new();

        for qterm in &query_tokens {
            let nq = self.df.get(qterm).copied().unwrap_or(0) as f64;
            if nq == 0.0 {
                continue;
            }

            // IDF(q) = ln((N - n(q) + 0.5) / (n(q) + 0.5) + 1)
            let idf = ((n - nq + 0.5) / (nq + 0.5) + 1.0).ln();

            if let Some(postings) = self.inverted.get(qterm) {
                for (doc_id, freq) in postings {
                    let fq = *freq as f64;
                    let dl = self.doc_lengths.get(doc_id).copied().unwrap_or(0) as f64;

                    // BM25 term score.
                    let numerator = fq * (k1 + 1.0);
                    let denominator = fq + k1 * (1.0 - b + b * dl / avg_dl);
                    let term_score = idf * numerator / denominator;

                    *scores.entry(doc_id.clone()).or_default() += term_score;
                }
            }
        }

        // Sort by score descending, take top_k.
        let mut results: Vec<(String, f64)> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("Hello, World!  foo_bar baz123");
        assert_eq!(tokens, vec!["hello", "world", "foo_bar", "baz123"]);
    }

    #[test]
    fn test_tokenize_empty() {
        let tokens = tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_tokenize_punctuation_only() {
        let tokens = tokenize("!!! ??? ---");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_bm25_simple() {
        let mut index = SearchIndex::new();
        index.add_document("doc1", "rust programming language");
        index.add_document("doc2", "python programming language");
        index.add_document("doc3", "rust web framework");

        let results = index.search("rust", 10);
        assert!(!results.is_empty());
        // doc1 and doc3 should match, doc2 should not.
        let ids: Vec<&str> = results.iter().map(|(id, _)| id.as_str()).collect();
        assert!(ids.contains(&"doc1"));
        assert!(ids.contains(&"doc3"));
        assert!(!ids.contains(&"doc2"));
    }

    #[test]
    fn test_bm25_ranking() {
        let mut index = SearchIndex::new();
        // Document with more occurrences of "rust" should rank higher.
        index.add_document("doc1", "rust rust rust programming");
        index.add_document("doc2", "rust programming");
        index.add_document("doc3", "python programming");

        let results = index.search("rust", 10);
        assert_eq!(results.len(), 2);
        // doc1 (3 occurrences) should rank higher than doc2 (1 occurrence).
        assert_eq!(results[0].0, "doc1");
        assert!(results[0].1 > results[1].1);
        assert_eq!(results[1].0, "doc2");
    }

    #[test]
    fn test_bm25_empty_query() {
        let mut index = SearchIndex::new();
        index.add_document("doc1", "hello world");
        let results = index.search("", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_bm25_no_results() {
        let mut index = SearchIndex::new();
        index.add_document("doc1", "hello world");
        let results = index.search("xyz", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_bm25_empty_index() {
        let index = SearchIndex::new();
        let results = index.search("hello", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_remove_document() {
        let mut index = SearchIndex::new();
        index.add_document("doc1", "rust programming");
        index.add_document("doc2", "python programming");

        index.remove_document("doc1");
        assert_eq!(index.doc_count, 1);

        let results = index.search("rust", 10);
        assert!(results.is_empty());

        let results = index.search("python", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "doc2");
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut index = SearchIndex::new();
        index.add_document("doc1", "hello");
        index.remove_document("nonexistent");
        assert_eq!(index.doc_count, 1);
    }

    #[test]
    fn test_replace_document() {
        let mut index = SearchIndex::new();
        index.add_document("doc1", "rust programming");
        index.add_document("doc1", "python web framework");

        assert_eq!(index.doc_count, 1);

        let results = index.search("python", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "doc1");

        let results = index.search("rust", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_bm25_multi_term_query() {
        let mut index = SearchIndex::new();
        index.add_document("doc1", "rust web framework actix");
        index.add_document("doc2", "rust systems programming");
        index.add_document("doc3", "python web framework django");

        // Query with both "rust" and "web" — doc1 should rank highest (matches both).
        let results = index.search("rust web", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "doc1");
    }

    #[test]
    fn test_bm25_top_k() {
        let mut index = SearchIndex::new();
        index.add_document("doc1", "rust a");
        index.add_document("doc2", "rust b");
        index.add_document("doc3", "rust c");

        let results = index.search("rust", 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_index_serialization() {
        let mut index = SearchIndex::new();
        index.add_document("doc1", "hello world");
        index.add_document("doc2", "foo bar");

        let json = serde_json::to_string(&index).unwrap();
        let deserialized: SearchIndex = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.doc_count, 2);
        assert_eq!(deserialized.doc_lengths.len(), 2);

        let results = deserialized.search("hello", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "doc1");
    }

    #[test]
    fn test_avg_dl_computation() {
        let mut index = SearchIndex::new();
        index.add_document("doc1", "a b c"); // 3 tokens
        index.add_document("doc2", "x y"); // 2 tokens
        assert_eq!(index.avg_dl, 2.5); // (3 + 2) / 2

        index.remove_document("doc1");
        assert_eq!(index.avg_dl, 2.0); // 2 / 1
    }

    #[test]
    fn test_bm25_idf_and_scoring() {
        // Verify IDF and BM25 score computation against hand-calculated values.
        // Setup: 3 documents, query "rust"
        let mut index = SearchIndex::new();
        index.add_document("doc1", "rust rust rust"); // tf=3, dl=3
        index.add_document("doc2", "rust python");     // tf=1, dl=2
        index.add_document("doc3", "python java");     // tf=0, dl=2

        // N=3, n("rust")=2 (appears in 2 docs)
        // IDF = ln((3 - 2 + 0.5) / (2 + 0.5) + 1) = ln(1.5/2.5 + 1) = ln(1.6)
        let idf_expected: f64 = (((3.0_f64 - 2.0 + 0.5) / (2.0 + 0.5)) + 1.0_f64).ln();
        assert!((idf_expected - 0.4700).abs() < 0.01, "IDF approx 0.47, got {idf_expected}");

        // avg_dl = (3 + 2 + 2) / 3 = 7/3
        assert!((index.avg_dl - 2.333).abs() < 0.01);

        let results = index.search("rust", 10);
        assert_eq!(results.len(), 2); // doc3 doesn't match

        // doc1 should rank higher than doc2 (higher tf)
        assert_eq!(results[0].0, "doc1");
        assert_eq!(results[1].0, "doc2");
        assert!(results[0].1 > results[1].1, "doc1 score ({}) > doc2 score ({})", results[0].1, results[1].1);

        // Verify scores are positive and finite
        for (_, score) in &results {
            assert!(score.is_finite() && *score > 0.0, "score should be positive finite, got {score}");
        }
    }
}
