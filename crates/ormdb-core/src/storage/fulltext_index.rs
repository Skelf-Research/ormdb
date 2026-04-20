//! Full-text search index with BM25 scoring.
//!
//! This module implements an inverted index for full-text search with BM25 ranking,
//! supporting term matching, phrase search, and boolean queries.
//!
//! ## BM25 Scoring
//!
//! BM25 (Best Matching 25) is a ranking function used to score document relevance:
//!
//! ```text
//! score(D, Q) = sum_{i=1}^{n} IDF(q_i) * (f(q_i, D) * (k1 + 1)) / (f(q_i, D) + k1 * (1 - b + b * |D| / avgdl))
//! ```
//!
//! Where:
//! - `f(q_i, D)` is term frequency of query term i in document D
//! - `|D|` is the document length
//! - `avgdl` is the average document length
//! - `k1` and `b` are tuning parameters (default: k1=1.2, b=0.75)
//! - `IDF(q_i)` is the inverse document frequency
//!
//! ## Storage Layout
//!
//! ```text
//! Tree: "index:fulltext:{entity_type}:{column_name}"
//!   "meta" -> FullTextMeta
//!   "posting:{term}" -> PostingList (list of (entity_id, tf, positions))
//!   "doc:{entity_id}" -> DocInfo (term_count, field_length)
//! ```

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use dashmap::DashMap;
use rkyv::{Archive, Deserialize, Serialize};
use sled::{Db, Tree};

use super::stemmer::{analyze, tokenize, PorterStemmer};
use crate::error::Error;

/// Tree name prefix for full-text indexes.
pub const FULLTEXT_INDEX_TREE_PREFIX: &str = "index:fulltext:";

/// Configuration for full-text index.
#[derive(Debug, Clone)]
pub struct FullTextConfig {
    /// BM25 k1 parameter (term frequency saturation).
    pub k1: f64,
    /// BM25 b parameter (document length normalization).
    pub b: f64,
    /// Whether to use stemming.
    pub use_stemming: bool,
    /// Whether to remove stop words.
    pub remove_stop_words: bool,
    /// Minimum term length to index.
    pub min_term_length: usize,
}

impl Default for FullTextConfig {
    fn default() -> Self {
        Self {
            k1: 1.2,
            b: 0.75,
            use_stemming: true,
            remove_stop_words: true,
            min_term_length: 2,
        }
    }
}

impl FullTextConfig {
    /// Create config optimized for precision (exact matching).
    pub fn precise() -> Self {
        Self {
            k1: 1.5,
            b: 0.5,
            use_stemming: false,
            remove_stop_words: false,
            min_term_length: 1,
        }
    }

    /// Create config optimized for recall (find more matches).
    pub fn recall() -> Self {
        Self {
            k1: 1.0,
            b: 0.75,
            use_stemming: true,
            remove_stop_words: true,
            min_term_length: 2,
        }
    }
}

/// A posting in the inverted index.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct Posting {
    /// Entity ID.
    entity_id: [u8; 16],
    /// Term frequency in this document.
    term_frequency: u32,
    /// Positions of the term in the document (for phrase queries).
    positions: Vec<u32>,
}

/// A posting list for a term.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct PostingList {
    /// All postings for this term.
    postings: Vec<Posting>,
}

impl PostingList {
    fn new() -> Self {
        Self {
            postings: Vec::new(),
        }
    }
}

/// Document information.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct DocInfo {
    /// Total number of terms in the document.
    term_count: u32,
    /// Unique terms in the document (for quick lookup).
    terms: Vec<String>,
}

/// Metadata for full-text index.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct FullTextMeta {
    /// Total number of documents.
    doc_count: u64,
    /// Sum of all document lengths (for average calculation).
    total_doc_length: u64,
    /// Configuration stored as simple values.
    k1: f64,
    b: f64,
}

/// Search result with relevance score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Entity ID.
    pub entity_id: [u8; 16],
    /// BM25 relevance score.
    pub score: f64,
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl Eq for SearchResult {}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchResult {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher score = higher priority
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(Ordering::Equal)
    }
}

/// Full-text search index with BM25 scoring.
pub struct FullTextIndex {
    db: Db,
    /// Cache for posting lists.
    posting_cache: DashMap<String, PostingList>,
    /// Configuration.
    config: FullTextConfig,
    /// Stemmer instance.
    stemmer: PorterStemmer,
}

impl FullTextIndex {
    /// Open or create a full-text index.
    pub fn open(db: &Db, config: FullTextConfig) -> Result<Self, Error> {
        Ok(Self {
            db: db.clone(),
            posting_cache: DashMap::new(),
            config,
            stemmer: PorterStemmer::new(),
        })
    }

    /// Get the sled tree for a specific entity type and column.
    fn tree_for_index(&self, entity_type: &str, column_name: &str) -> Result<Tree, Error> {
        let tree_name = format!(
            "{}{}:{}",
            FULLTEXT_INDEX_TREE_PREFIX, entity_type, column_name
        );
        Ok(self.db.open_tree(tree_name)?)
    }

    /// Analyze text into indexable terms.
    fn analyze_text(&self, text: &str) -> Vec<String> {
        if self.config.use_stemming && self.config.remove_stop_words {
            analyze(text)
                .into_iter()
                .filter(|t| t.len() >= self.config.min_term_length)
                .collect()
        } else if self.config.use_stemming {
            tokenize(text)
                .into_iter()
                .filter(|t| t.len() >= self.config.min_term_length)
                .map(|w| self.stemmer.stem(&w))
                .collect()
        } else if self.config.remove_stop_words {
            tokenize(text)
                .into_iter()
                .filter(|t| t.len() >= self.config.min_term_length && !super::stemmer::is_stop_word(t))
                .collect()
        } else {
            tokenize(text)
                .into_iter()
                .filter(|t| t.len() >= self.config.min_term_length)
                .collect()
        }
    }

    /// Build the key for metadata.
    fn meta_key() -> Vec<u8> {
        b"meta".to_vec()
    }

    /// Build the key for a posting list.
    fn posting_key(term: &str) -> Vec<u8> {
        let mut key = Vec::with_capacity(8 + term.len());
        key.extend_from_slice(b"posting:");
        key.extend_from_slice(term.as_bytes());
        key
    }

    /// Build the key for document info.
    fn doc_key(entity_id: &[u8; 16]) -> Vec<u8> {
        let mut key = Vec::with_capacity(4 + 16);
        key.extend_from_slice(b"doc:");
        key.extend_from_slice(entity_id);
        key
    }

    /// Get metadata for an index.
    fn get_meta(&self, tree: &Tree) -> Result<Option<FullTextMeta>, Error> {
        match tree.get(Self::meta_key())? {
            Some(bytes) => {
                let archived = rkyv::access::<ArchivedFullTextMeta, rkyv::rancor::Error>(&bytes)
                    .map_err(|e| Error::Deserialization(e.to_string()))?;
                let meta: FullTextMeta =
                    rkyv::deserialize::<FullTextMeta, rkyv::rancor::Error>(archived)
                        .map_err(|e| Error::Deserialization(e.to_string()))?;
                Ok(Some(meta))
            }
            None => Ok(None),
        }
    }

    /// Save metadata for an index.
    fn save_meta(&self, tree: &Tree, meta: &FullTextMeta) -> Result<(), Error> {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(meta)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        tree.insert(Self::meta_key(), bytes.as_slice())?;
        Ok(())
    }

    /// Get a posting list for a term.
    fn get_posting_list(&self, tree: &Tree, term: &str) -> Result<Option<PostingList>, Error> {
        // Check cache first
        let cache_key = term.to_string();
        if let Some(pl) = self.posting_cache.get(&cache_key) {
            return Ok(Some(pl.clone()));
        }

        // Load from disk
        let key = Self::posting_key(term);
        match tree.get(&key)? {
            Some(bytes) => {
                let archived = rkyv::access::<ArchivedPostingList, rkyv::rancor::Error>(&bytes)
                    .map_err(|e| Error::Deserialization(e.to_string()))?;
                let pl: PostingList =
                    rkyv::deserialize::<PostingList, rkyv::rancor::Error>(archived)
                        .map_err(|e| Error::Deserialization(e.to_string()))?;

                // Cache it
                self.posting_cache.insert(cache_key, pl.clone());

                Ok(Some(pl))
            }
            None => Ok(None),
        }
    }

    /// Save a posting list for a term.
    fn save_posting_list(&self, tree: &Tree, term: &str, pl: &PostingList) -> Result<(), Error> {
        let key = Self::posting_key(term);
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(pl)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        tree.insert(key, bytes.as_slice())?;

        // Update cache
        self.posting_cache.insert(term.to_string(), pl.clone());

        Ok(())
    }

    /// Get document info.
    fn get_doc_info(&self, tree: &Tree, entity_id: &[u8; 16]) -> Result<Option<DocInfo>, Error> {
        let key = Self::doc_key(entity_id);
        match tree.get(&key)? {
            Some(bytes) => {
                let archived = rkyv::access::<ArchivedDocInfo, rkyv::rancor::Error>(&bytes)
                    .map_err(|e| Error::Deserialization(e.to_string()))?;
                let info: DocInfo = rkyv::deserialize::<DocInfo, rkyv::rancor::Error>(archived)
                    .map_err(|e| Error::Deserialization(e.to_string()))?;
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    /// Save document info.
    fn save_doc_info(&self, tree: &Tree, entity_id: &[u8; 16], info: &DocInfo) -> Result<(), Error> {
        let key = Self::doc_key(entity_id);
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(info)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        tree.insert(key, bytes.as_slice())?;
        Ok(())
    }

    /// Index a document.
    pub fn insert(
        &self,
        entity_type: &str,
        column_name: &str,
        entity_id: [u8; 16],
        text: &str,
    ) -> Result<(), Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        // Get or create metadata
        let mut meta = self.get_meta(&tree)?.unwrap_or(FullTextMeta {
            doc_count: 0,
            total_doc_length: 0,
            k1: self.config.k1,
            b: self.config.b,
        });

        // Check if document already exists (for update)
        let old_doc_info = self.get_doc_info(&tree, &entity_id)?;
        if let Some(ref old_info) = old_doc_info {
            // Remove old postings
            for term in &old_info.terms {
                if let Some(mut pl) = self.get_posting_list(&tree, term)? {
                    pl.postings.retain(|p| p.entity_id != entity_id);
                    if pl.postings.is_empty() {
                        tree.remove(Self::posting_key(term))?;
                        self.posting_cache.remove(term);
                    } else {
                        self.save_posting_list(&tree, term, &pl)?;
                    }
                }
            }
            meta.total_doc_length -= old_info.term_count as u64;
            meta.doc_count -= 1;
        }

        // Analyze text into terms with positions
        let terms = self.analyze_text(text);
        if terms.is_empty() {
            // Nothing to index, but save empty doc info
            let doc_info = DocInfo {
                term_count: 0,
                terms: Vec::new(),
            };
            self.save_doc_info(&tree, &entity_id, &doc_info)?;
            meta.doc_count += 1;
            self.save_meta(&tree, &meta)?;
            return Ok(());
        }

        // Build term frequency and position map
        let mut term_info: HashMap<String, (u32, Vec<u32>)> = HashMap::new();
        for (pos, term) in terms.iter().enumerate() {
            let entry = term_info.entry(term.clone()).or_insert((0, Vec::new()));
            entry.0 += 1;
            entry.1.push(pos as u32);
        }

        // Update posting lists
        let unique_terms: Vec<String> = term_info.keys().cloned().collect();
        for (term, (tf, positions)) in term_info {
            let mut pl = self.get_posting_list(&tree, &term)?.unwrap_or_else(PostingList::new);

            // Add new posting
            pl.postings.push(Posting {
                entity_id,
                term_frequency: tf,
                positions,
            });

            self.save_posting_list(&tree, &term, &pl)?;
        }

        // Save document info
        let doc_info = DocInfo {
            term_count: terms.len() as u32,
            terms: unique_terms,
        };
        self.save_doc_info(&tree, &entity_id, &doc_info)?;

        // Update metadata
        meta.doc_count += 1;
        meta.total_doc_length += terms.len() as u64;
        self.save_meta(&tree, &meta)?;

        Ok(())
    }

    /// Calculate IDF (Inverse Document Frequency) for a term.
    fn idf(&self, doc_count: u64, doc_freq: usize) -> f64 {
        if doc_freq == 0 {
            return 0.0;
        }
        let n = doc_count as f64;
        let df = doc_freq as f64;
        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }

    /// Calculate BM25 score for a document.
    fn bm25_score(
        &self,
        tf: u32,
        doc_length: u32,
        avg_doc_length: f64,
        idf: f64,
        k1: f64,
        b: f64,
    ) -> f64 {
        let tf = tf as f64;
        let dl = doc_length as f64;
        let numerator = tf * (k1 + 1.0);
        let denominator = tf + k1 * (1.0 - b + b * dl / avg_doc_length);
        idf * numerator / denominator
    }

    /// Search for documents matching a query.
    pub fn search(
        &self,
        entity_type: &str,
        column_name: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, Error> {
        self.search_with_min_score(entity_type, column_name, query, limit, None)
    }

    /// Search for documents matching a query with minimum score threshold.
    pub fn search_with_min_score(
        &self,
        entity_type: &str,
        column_name: &str,
        query: &str,
        limit: usize,
        min_score: Option<f64>,
    ) -> Result<Vec<SearchResult>, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        let meta = match self.get_meta(&tree)? {
            Some(m) => m,
            None => return Ok(vec![]),
        };

        if meta.doc_count == 0 {
            return Ok(vec![]);
        }

        let avg_doc_length = meta.total_doc_length as f64 / meta.doc_count as f64;
        let query_terms = self.analyze_text(query);

        if query_terms.is_empty() {
            return Ok(vec![]);
        }

        // Collect scores for all documents
        let mut doc_scores: HashMap<[u8; 16], f64> = HashMap::new();

        for term in &query_terms {
            if let Some(pl) = self.get_posting_list(&tree, term)? {
                let idf = self.idf(meta.doc_count, pl.postings.len());

                for posting in &pl.postings {
                    // Get document length
                    let doc_length = self
                        .get_doc_info(&tree, &posting.entity_id)?
                        .map_or(1, |d| d.term_count);

                    let score = self.bm25_score(
                        posting.term_frequency,
                        doc_length,
                        avg_doc_length,
                        idf,
                        meta.k1,
                        meta.b,
                    );

                    *doc_scores.entry(posting.entity_id).or_insert(0.0) += score;
                }
            }
        }

        // Sort by score and take top results
        let min = min_score.unwrap_or(0.0);
        let mut results: Vec<SearchResult> = doc_scores
            .into_iter()
            .filter(|(_, score)| *score >= min)
            .map(|(entity_id, score)| SearchResult { entity_id, score })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    /// Search for documents containing an exact phrase.
    pub fn search_phrase(
        &self,
        entity_type: &str,
        column_name: &str,
        phrase: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        let meta = match self.get_meta(&tree)? {
            Some(m) => m,
            None => return Ok(vec![]),
        };

        if meta.doc_count == 0 {
            return Ok(vec![]);
        }

        let phrase_terms = self.analyze_text(phrase);
        if phrase_terms.is_empty() {
            return Ok(vec![]);
        }

        // Get posting lists for all terms in the phrase
        let mut posting_lists: Vec<(String, PostingList)> = Vec::new();
        for term in &phrase_terms {
            match self.get_posting_list(&tree, term)? {
                Some(pl) => posting_lists.push((term.clone(), pl)),
                None => return Ok(vec![]), // Phrase can't match if any term is missing
            }
        }

        // Find documents containing all terms
        let mut candidate_docs: HashSet<[u8; 16]> = posting_lists[0]
            .1
            .postings
            .iter()
            .map(|p| p.entity_id)
            .collect();

        for (_, pl) in &posting_lists[1..] {
            let docs: HashSet<[u8; 16]> = pl.postings.iter().map(|p| p.entity_id).collect();
            candidate_docs = candidate_docs.intersection(&docs).copied().collect();
        }

        if candidate_docs.is_empty() {
            return Ok(vec![]);
        }

        // Check phrase positions in candidate documents
        let avg_doc_length = meta.total_doc_length as f64 / meta.doc_count as f64;
        let mut results = Vec::new();

        for doc_id in candidate_docs {
            // Get positions for each term in this document
            let mut term_positions: Vec<Vec<u32>> = Vec::new();
            for (_, pl) in &posting_lists {
                if let Some(posting) = pl.postings.iter().find(|p| p.entity_id == doc_id) {
                    term_positions.push(posting.positions.clone());
                } else {
                    break;
                }
            }

            if term_positions.len() != phrase_terms.len() {
                continue;
            }

            // Check if terms appear consecutively
            let mut phrase_matches = 0;
            for &start_pos in &term_positions[0] {
                let mut matches = true;
                for (offset, positions) in term_positions.iter().enumerate().skip(1) {
                    let expected_pos = start_pos + offset as u32;
                    if !positions.contains(&expected_pos) {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    phrase_matches += 1;
                }
            }

            if phrase_matches > 0 {
                // Calculate score based on phrase frequency
                let doc_length = self.get_doc_info(&tree, &doc_id)?.map_or(1, |d| d.term_count);

                // Use average IDF of phrase terms
                let avg_idf: f64 = posting_lists
                    .iter()
                    .map(|(_, pl)| self.idf(meta.doc_count, pl.postings.len()))
                    .sum::<f64>()
                    / posting_lists.len() as f64;

                let score = self.bm25_score(
                    phrase_matches,
                    doc_length,
                    avg_doc_length,
                    avg_idf * phrase_terms.len() as f64, // Boost for phrase match
                    meta.k1,
                    meta.b,
                );

                results.push(SearchResult {
                    entity_id: doc_id,
                    score,
                });
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    /// Boolean search with must, should, and must_not terms.
    pub fn search_boolean(
        &self,
        entity_type: &str,
        column_name: &str,
        must: &[String],
        should: &[String],
        must_not: &[String],
        limit: usize,
    ) -> Result<Vec<SearchResult>, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        let meta = match self.get_meta(&tree)? {
            Some(m) => m,
            None => return Ok(vec![]),
        };

        if meta.doc_count == 0 {
            return Ok(vec![]);
        }

        let avg_doc_length = meta.total_doc_length as f64 / meta.doc_count as f64;

        // Process must terms - documents must contain ALL of these
        let must_terms: Vec<String> = must.iter().flat_map(|t| self.analyze_text(t)).collect();
        let mut must_docs: Option<HashSet<[u8; 16]>> = None;

        for term in &must_terms {
            if let Some(pl) = self.get_posting_list(&tree, term)? {
                let docs: HashSet<[u8; 16]> = pl.postings.iter().map(|p| p.entity_id).collect();
                must_docs = Some(match must_docs {
                    Some(existing) => existing.intersection(&docs).copied().collect(),
                    None => docs,
                });
            } else {
                return Ok(vec![]); // Must term not found
            }
        }

        // Process must_not terms - documents must NOT contain ANY of these
        let must_not_terms: Vec<String> = must_not
            .iter()
            .flat_map(|t| self.analyze_text(t))
            .collect();
        let mut excluded_docs: HashSet<[u8; 16]> = HashSet::new();

        for term in &must_not_terms {
            if let Some(pl) = self.get_posting_list(&tree, term)? {
                for posting in &pl.postings {
                    excluded_docs.insert(posting.entity_id);
                }
            }
        }

        // Get candidate documents
        let candidates: HashSet<[u8; 16]> = match must_docs {
            Some(docs) => docs.difference(&excluded_docs).copied().collect(),
            None => {
                // No must terms - use should terms to find candidates
                let should_terms: Vec<String> =
                    should.iter().flat_map(|t| self.analyze_text(t)).collect();
                if should_terms.is_empty() {
                    return Ok(vec![]);
                }

                let mut docs = HashSet::new();
                for term in &should_terms {
                    if let Some(pl) = self.get_posting_list(&tree, term)? {
                        for posting in &pl.postings {
                            if !excluded_docs.contains(&posting.entity_id) {
                                docs.insert(posting.entity_id);
                            }
                        }
                    }
                }
                docs
            }
        };

        if candidates.is_empty() {
            return Ok(vec![]);
        }

        // Score candidates
        let should_terms: Vec<String> = should.iter().flat_map(|t| self.analyze_text(t)).collect();
        let all_scoring_terms: Vec<String> = must_terms
            .into_iter()
            .chain(should_terms.into_iter())
            .collect();

        let mut doc_scores: HashMap<[u8; 16], f64> = HashMap::new();

        for term in &all_scoring_terms {
            if let Some(pl) = self.get_posting_list(&tree, term)? {
                let idf = self.idf(meta.doc_count, pl.postings.len());

                for posting in &pl.postings {
                    if !candidates.contains(&posting.entity_id) {
                        continue;
                    }

                    let doc_length = self
                        .get_doc_info(&tree, &posting.entity_id)?
                        .map_or(1, |d| d.term_count);

                    let score = self.bm25_score(
                        posting.term_frequency,
                        doc_length,
                        avg_doc_length,
                        idf,
                        meta.k1,
                        meta.b,
                    );

                    *doc_scores.entry(posting.entity_id).or_insert(0.0) += score;
                }
            }
        }

        let mut results: Vec<SearchResult> = doc_scores
            .into_iter()
            .map(|(entity_id, score)| SearchResult { entity_id, score })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    /// Remove a document from the index.
    pub fn remove(
        &self,
        entity_type: &str,
        column_name: &str,
        entity_id: [u8; 16],
    ) -> Result<(), Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        // Get document info
        let doc_info = match self.get_doc_info(&tree, &entity_id)? {
            Some(info) => info,
            None => return Ok(()), // Already removed
        };

        // Remove from posting lists
        for term in &doc_info.terms {
            if let Some(mut pl) = self.get_posting_list(&tree, term)? {
                pl.postings.retain(|p| p.entity_id != entity_id);
                if pl.postings.is_empty() {
                    tree.remove(Self::posting_key(term))?;
                    self.posting_cache.remove(term);
                } else {
                    self.save_posting_list(&tree, term, &pl)?;
                }
            }
        }

        // Remove document info
        tree.remove(Self::doc_key(&entity_id))?;

        // Update metadata
        if let Some(mut meta) = self.get_meta(&tree)? {
            meta.doc_count = meta.doc_count.saturating_sub(1);
            meta.total_doc_length = meta.total_doc_length.saturating_sub(doc_info.term_count as u64);
            self.save_meta(&tree, &meta)?;
        }

        Ok(())
    }

    /// Check if a full-text index exists for a column.
    pub fn has_index(&self, entity_type: &str, column_name: &str) -> Result<bool, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;
        Ok(self.get_meta(&tree)?.is_some())
    }

    /// Get the count of documents in an index.
    pub fn count(&self, entity_type: &str, column_name: &str) -> Result<u64, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;
        Ok(self.get_meta(&tree)?.map_or(0, |m| m.doc_count))
    }

    /// Drop the index for an entity type and column.
    pub fn drop_index(&self, entity_type: &str, column_name: &str) -> Result<(), Error> {
        let tree_name = format!(
            "{}{}:{}",
            FULLTEXT_INDEX_TREE_PREFIX, entity_type, column_name
        );
        self.db.drop_tree(tree_name)?;
        self.posting_cache.clear();
        Ok(())
    }

    /// Flush the index to disk.
    pub fn flush(&self) -> Result<(), Error> {
        self.db.flush()?;
        Ok(())
    }

    /// Clear the posting cache.
    pub fn clear_cache(&self) {
        self.posting_cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_index() -> FullTextIndex {
        let db = sled::Config::new().temporary(true).open().unwrap();
        FullTextIndex::open(&db, FullTextConfig::default()).unwrap()
    }

    #[test]
    fn test_insert_and_search_single() {
        let index = test_index();
        let entity_id = [1u8; 16];
        let text = "The quick brown fox jumps over the lazy dog";

        index
            .insert("Article", "content", entity_id, text)
            .unwrap();

        let results = index.search("Article", "content", "quick fox", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entity_id, entity_id);
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_search_multiple_documents() {
        let index = test_index();

        let docs = vec![
            ([1u8; 16], "Rust programming language is fast and memory safe"),
            ([2u8; 16], "Python programming is easy to learn"),
            ([3u8; 16], "JavaScript runs in the browser"),
            ([4u8; 16], "Rust and Python are both popular programming languages"),
        ];

        for (id, text) in &docs {
            let mut eid = [0u8; 16];
            eid[0] = id[0];
            index.insert("Article", "content", eid, text).unwrap();
        }

        // Search for "programming"
        let results = index
            .search("Article", "content", "programming", 10)
            .unwrap();
        assert_eq!(results.len(), 3); // Docs 1, 2, 4 contain "programming"

        // Search for "Rust"
        let results = index.search("Article", "content", "Rust", 10).unwrap();
        assert_eq!(results.len(), 2); // Docs 1, 4 contain "Rust"

        // Search for "Rust programming"
        let results = index
            .search("Article", "content", "Rust programming", 10)
            .unwrap();
        assert!(results.len() >= 2);
        // Doc 1 should rank highest (contains both terms prominently)
    }

    #[test]
    fn test_phrase_search() {
        let index = test_index();

        let docs = vec![
            ([1u8; 16], "The quick brown fox jumps over the lazy dog"),
            ([2u8; 16], "A brown fox is quick and clever"),
            ([3u8; 16], "Quick thinking helps in a brown world with a fox"),
        ];

        for (id, text) in &docs {
            let mut eid = [0u8; 16];
            eid[0] = id[0];
            index.insert("Article", "content", eid, text).unwrap();
        }

        // Search for exact phrase "quick brown fox"
        let results = index
            .search_phrase("Article", "content", "quick brown fox", 10)
            .unwrap();
        assert_eq!(results.len(), 1); // Only doc 1 has this exact phrase
        assert_eq!(results[0].entity_id[0], 1);
    }

    #[test]
    fn test_boolean_search() {
        let index = test_index();

        let docs = vec![
            ([1u8; 16], "Rust programming language"),
            ([2u8; 16], "Python programming language"),
            ([3u8; 16], "Rust systems programming"),
            ([4u8; 16], "JavaScript web programming"),
        ];

        for (id, text) in &docs {
            let mut eid = [0u8; 16];
            eid[0] = id[0];
            index.insert("Article", "content", eid, text).unwrap();
        }

        // Must have "programming", must not have "Python"
        let results = index
            .search_boolean(
                "Article",
                "content",
                &["programming".to_string()],
                &[],
                &["Python".to_string()],
                10,
            )
            .unwrap();
        assert_eq!(results.len(), 3); // Docs 1, 3, 4

        // Must have "Rust" AND "programming"
        let results = index
            .search_boolean(
                "Article",
                "content",
                &["Rust".to_string(), "programming".to_string()],
                &[],
                &[],
                10,
            )
            .unwrap();
        assert_eq!(results.len(), 2); // Docs 1, 3
    }

    #[test]
    fn test_remove_document() {
        let index = test_index();

        let id1 = [1u8; 16];
        let id2 = [2u8; 16];

        index
            .insert("Article", "content", id1, "Hello world")
            .unwrap();
        index
            .insert("Article", "content", id2, "World peace")
            .unwrap();

        assert_eq!(index.count("Article", "content").unwrap(), 2);

        // Remove first document
        index.remove("Article", "content", id1).unwrap();

        assert_eq!(index.count("Article", "content").unwrap(), 1);

        // Search should only find second document
        let results = index.search("Article", "content", "world", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entity_id, id2);
    }

    #[test]
    fn test_update_document() {
        let index = test_index();
        let entity_id = [1u8; 16];

        // Insert initial content
        index
            .insert("Article", "content", entity_id, "Hello world")
            .unwrap();

        let results = index.search("Article", "content", "hello", 10).unwrap();
        assert_eq!(results.len(), 1);

        // Update content (re-insert with same ID)
        index
            .insert("Article", "content", entity_id, "Goodbye moon")
            .unwrap();

        // Old term should not match
        let results = index.search("Article", "content", "hello", 10).unwrap();
        assert_eq!(results.len(), 0);

        // New term should match
        let results = index.search("Article", "content", "goodbye", 10).unwrap();
        assert_eq!(results.len(), 1);

        // Document count should still be 1
        assert_eq!(index.count("Article", "content").unwrap(), 1);
    }

    #[test]
    fn test_empty_index() {
        let index = test_index();

        let results = index.search("Article", "content", "test", 10).unwrap();
        assert!(results.is_empty());

        let results = index
            .search_phrase("Article", "content", "test phrase", 10)
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_has_index() {
        let index = test_index();

        assert!(!index.has_index("Article", "content").unwrap());

        index
            .insert("Article", "content", [1u8; 16], "test")
            .unwrap();

        assert!(index.has_index("Article", "content").unwrap());
        assert!(!index.has_index("Article", "other").unwrap());
    }

    #[test]
    fn test_drop_index() {
        let index = test_index();

        index
            .insert("Article", "content", [1u8; 16], "test")
            .unwrap();
        assert!(index.has_index("Article", "content").unwrap());

        index.drop_index("Article", "content").unwrap();
        assert!(!index.has_index("Article", "content").unwrap());
    }

    #[test]
    fn test_min_score_filter() {
        let index = test_index();

        let docs = vec![
            (
                [1u8; 16],
                "Rust Rust Rust programming language", // High frequency of "Rust"
            ),
            ([2u8; 16], "Rust is a language"),  // Lower frequency
            ([3u8; 16], "Programming in Rust"), // Lower frequency
        ];

        for (id, text) in &docs {
            let mut eid = [0u8; 16];
            eid[0] = id[0];
            index.insert("Article", "content", eid, text).unwrap();
        }

        // Search with high minimum score
        let results = index
            .search_with_min_score("Article", "content", "Rust", 10, Some(1.0))
            .unwrap();

        // Should filter out low-scoring documents
        assert!(results.len() <= 3);
        for r in &results {
            assert!(r.score >= 1.0);
        }
    }

    #[test]
    fn test_bm25_ranking() {
        let index = test_index();

        // Document 1 has higher term frequency for "rust"
        let doc1 = "Rust is amazing. Rust is fast. Rust is memory safe.";
        // Document 2 has lower term frequency
        let doc2 = "Rust is a programming language.";
        // Document 3 doesn't contain the search term
        let doc3 = "Python is also a great language.";

        index
            .insert("Article", "content", [1u8; 16], doc1)
            .unwrap();
        index
            .insert("Article", "content", [2u8; 16], doc2)
            .unwrap();
        index
            .insert("Article", "content", [3u8; 16], doc3)
            .unwrap();

        let results = index.search("Article", "content", "rust", 10).unwrap();

        // Only docs 1 and 2 should match
        assert_eq!(results.len(), 2);

        // Doc 1 should rank higher due to higher term frequency
        assert_eq!(results[0].entity_id[0], 1);
        assert_eq!(results[1].entity_id[0], 2);
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_large_dataset() {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let index = FullTextIndex::open(&db, FullTextConfig::default()).unwrap();

        // Insert 100 documents
        for i in 0..100 {
            let mut id = [0u8; 16];
            id[0..2].copy_from_slice(&(i as u16).to_le_bytes());
            let text = format!(
                "Document number {} about topic {} with content {}",
                i,
                i % 10,
                i % 5
            );
            index.insert("Article", "content", id, &text).unwrap();
        }

        assert_eq!(index.count("Article", "content").unwrap(), 100);

        // Search should work efficiently
        let results = index
            .search("Article", "content", "document topic", 10)
            .unwrap();
        assert_eq!(results.len(), 10);

        // All results should have positive scores
        for r in &results {
            assert!(r.score > 0.0);
        }
    }
}
