//! HNSW (Hierarchical Navigable Small World) vector index for similarity search.
//!
//! This module implements approximate nearest neighbor search using the HNSW algorithm,
//! storing the graph structure in sled for persistence.
//!
//! ## Algorithm Overview
//!
//! HNSW builds a multi-layer graph where:
//! - Higher layers contain fewer nodes (exponentially decreasing)
//! - Search starts from top layer and descends to layer 0
//! - Each layer uses greedy search to find entry point for next layer
//! - Layer 0 contains all nodes and returns final results
//!
//! ## Storage Layout
//!
//! ```text
//! Tree: "index:vector:{entity_type}:{column_name}"
//!   "meta" -> HnswMeta (serialized)
//!   "node:{entity_id}" -> HnswNode (serialized)
//! ```

use std::collections::{BinaryHeap, HashSet};
use std::cmp::Ordering;

use dashmap::DashMap;
use rand::Rng;
use rkyv::{Archive, Deserialize, Serialize};
use sled::{Db, Tree};

use crate::error::Error;

/// Tree name prefix for vector indexes.
pub const VECTOR_INDEX_TREE_PREFIX: &str = "index:vector:";

/// Distance metric for vector similarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum DistanceMetric {
    /// Cosine similarity (1 - cosine_sim, so lower is more similar).
    Cosine,
    /// Euclidean (L2) distance.
    Euclidean,
    /// Negative dot product (so lower is more similar for max inner product search).
    DotProduct,
}

impl Default for DistanceMetric {
    fn default() -> Self {
        DistanceMetric::Cosine
    }
}

/// Configuration for HNSW index.
#[derive(Debug, Clone)]
pub struct HnswConfig {
    /// Maximum number of connections per node per layer.
    pub m: usize,
    /// Maximum connections on layer 0 (typically 2*m).
    pub m0: usize,
    /// Size of dynamic candidate list during construction.
    pub ef_construction: usize,
    /// Size of dynamic candidate list during search.
    pub ef_search: usize,
    /// Level multiplier for random layer assignment.
    pub ml: f64,
    /// Distance metric to use.
    pub metric: DistanceMetric,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            m0: 32,
            ef_construction: 100,
            ef_search: 100,
            ml: 1.0 / (16.0_f64).ln(), // 1/ln(M)
            metric: DistanceMetric::Cosine,
        }
    }
}

impl HnswConfig {
    /// Create config optimized for high recall.
    pub fn high_recall() -> Self {
        Self {
            m: 32,
            m0: 64,
            ef_construction: 200,
            ef_search: 200,
            ml: 1.0 / (32.0_f64).ln(),
            metric: DistanceMetric::Cosine,
        }
    }

    /// Create config optimized for speed.
    pub fn fast() -> Self {
        Self {
            m: 8,
            m0: 16,
            ef_construction: 50,
            ef_search: 50,
            ml: 1.0 / (8.0_f64).ln(),
            metric: DistanceMetric::Cosine,
        }
    }

    /// Set the distance metric.
    pub fn with_metric(mut self, metric: DistanceMetric) -> Self {
        self.metric = metric;
        self
    }
}

/// Metadata for HNSW index stored persistently.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct HnswMeta {
    /// Number of dimensions for vectors.
    dimensions: u32,
    /// Distance metric.
    metric: DistanceMetric,
    /// Entry point entity ID (if any).
    entry_point: Option<[u8; 16]>,
    /// Maximum layer in the graph.
    max_layer: u8,
    /// Total number of vectors in the index.
    count: u64,
    /// Config parameters (for reconstruction).
    m: u16,
    m0: u16,
}

/// A node in the HNSW graph.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct HnswNode {
    /// The vector data.
    vector: Vec<f32>,
    /// The layer this node was assigned to.
    layer: u8,
    /// Neighbors for each layer (layer 0 first).
    /// Each layer has a list of neighbor entity IDs.
    neighbors: Vec<Vec<[u8; 16]>>,
}

/// A candidate during search (ordered by distance, smaller = better).
#[derive(Clone)]
struct Candidate {
    entity_id: [u8; 16],
    distance: f32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for max-heap (we want smallest distance at top)
        other.distance.partial_cmp(&self.distance).unwrap_or(Ordering::Equal)
    }
}

/// Reverse candidate for max-heap operations.
struct ReversedCandidate(Candidate);

impl PartialEq for ReversedCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.0.distance == other.0.distance
    }
}

impl Eq for ReversedCandidate {}

impl PartialOrd for ReversedCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ReversedCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Normal order (largest distance at top)
        self.0.distance.partial_cmp(&other.0.distance).unwrap_or(Ordering::Equal)
    }
}

/// HNSW vector index for similarity search.
pub struct VectorIndex {
    db: Db,
    /// Cache for frequently accessed nodes.
    node_cache: DashMap<[u8; 16], HnswNode>,
    /// Configuration.
    config: HnswConfig,
}

impl VectorIndex {
    /// Open or create a vector index.
    pub fn open(db: &Db, config: HnswConfig) -> Result<Self, Error> {
        Ok(Self {
            db: db.clone(),
            node_cache: DashMap::new(),
            config,
        })
    }

    /// Get the sled tree for a specific entity type and column.
    fn tree_for_index(&self, entity_type: &str, column_name: &str) -> Result<Tree, Error> {
        let tree_name = format!("{}{}:{}", VECTOR_INDEX_TREE_PREFIX, entity_type, column_name);
        Ok(self.db.open_tree(tree_name)?)
    }

    /// Get metadata for an index.
    fn get_meta(&self, tree: &Tree) -> Result<Option<HnswMeta>, Error> {
        match tree.get("meta")? {
            Some(bytes) => {
                let archived = rkyv::access::<ArchivedHnswMeta, rkyv::rancor::Error>(&bytes)
                    .map_err(|e| Error::Deserialization(e.to_string()))?;
                let meta: HnswMeta = rkyv::deserialize::<HnswMeta, rkyv::rancor::Error>(archived)
                    .map_err(|e| Error::Deserialization(e.to_string()))?;
                Ok(Some(meta))
            }
            None => Ok(None),
        }
    }

    /// Save metadata for an index.
    fn save_meta(&self, tree: &Tree, meta: &HnswMeta) -> Result<(), Error> {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(meta)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        tree.insert("meta", bytes.as_slice())?;
        Ok(())
    }

    /// Build the key for a node.
    fn node_key(entity_id: &[u8; 16]) -> Vec<u8> {
        let mut key = Vec::with_capacity(5 + 16);
        key.extend_from_slice(b"node:");
        key.extend_from_slice(entity_id);
        key
    }

    /// Get a node from the index.
    fn get_node(&self, tree: &Tree, entity_id: &[u8; 16]) -> Result<Option<HnswNode>, Error> {
        // Check cache first
        if let Some(node) = self.node_cache.get(entity_id) {
            return Ok(Some(node.clone()));
        }

        // Load from disk
        let key = Self::node_key(entity_id);
        match tree.get(&key)? {
            Some(bytes) => {
                let archived = rkyv::access::<ArchivedHnswNode, rkyv::rancor::Error>(&bytes)
                    .map_err(|e| Error::Deserialization(e.to_string()))?;
                let node: HnswNode = rkyv::deserialize::<HnswNode, rkyv::rancor::Error>(archived)
                    .map_err(|e| Error::Deserialization(e.to_string()))?;

                // Cache it
                self.node_cache.insert(*entity_id, node.clone());

                Ok(Some(node))
            }
            None => Ok(None),
        }
    }

    /// Save a node to the index.
    fn save_node(&self, tree: &Tree, entity_id: &[u8; 16], node: &HnswNode) -> Result<(), Error> {
        let key = Self::node_key(entity_id);
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(node)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        tree.insert(key, bytes.as_slice())?;

        // Update cache
        self.node_cache.insert(*entity_id, node.clone());

        Ok(())
    }

    /// Calculate distance between two vectors.
    fn distance(&self, a: &[f32], b: &[f32], metric: DistanceMetric) -> f32 {
        match metric {
            DistanceMetric::Cosine => {
                let mut dot = 0.0;
                let mut norm_a = 0.0;
                let mut norm_b = 0.0;
                for i in 0..a.len().min(b.len()) {
                    dot += a[i] * b[i];
                    norm_a += a[i] * a[i];
                    norm_b += b[i] * b[i];
                }
                let denom = (norm_a * norm_b).sqrt();
                if denom == 0.0 {
                    1.0
                } else {
                    1.0 - (dot / denom)
                }
            }
            DistanceMetric::Euclidean => {
                let mut sum = 0.0;
                for i in 0..a.len().min(b.len()) {
                    let diff = a[i] - b[i];
                    sum += diff * diff;
                }
                sum.sqrt()
            }
            DistanceMetric::DotProduct => {
                let mut dot = 0.0;
                for i in 0..a.len().min(b.len()) {
                    dot += a[i] * b[i];
                }
                -dot // Negative so smaller = more similar
            }
        }
    }

    /// Generate a random layer for a new node.
    fn random_layer(&self) -> u8 {
        let mut rng = rand::thread_rng();
        let r: f64 = rng.gen();
        let layer = (-r.ln() * self.config.ml).floor() as u8;
        layer.min(255) // Cap at max u8
    }

    /// Insert a vector into the index.
    pub fn insert(
        &self,
        entity_type: &str,
        column_name: &str,
        entity_id: [u8; 16],
        vector: &[f32],
    ) -> Result<(), Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        // Get or create metadata
        let mut meta = match self.get_meta(&tree)? {
            Some(m) => {
                // Validate dimensions
                if m.dimensions != vector.len() as u32 {
                    return Err(Error::InvalidData(format!(
                        "Vector dimension mismatch: expected {}, got {}",
                        m.dimensions,
                        vector.len()
                    )));
                }
                m
            }
            None => {
                // First vector - initialize metadata
                HnswMeta {
                    dimensions: vector.len() as u32,
                    metric: self.config.metric,
                    entry_point: None,
                    max_layer: 0,
                    count: 0,
                    m: self.config.m as u16,
                    m0: self.config.m0 as u16,
                }
            }
        };

        // Generate random layer for this node
        let node_layer = self.random_layer();

        // Create the new node
        let mut new_node = HnswNode {
            vector: vector.to_vec(),
            layer: node_layer,
            neighbors: vec![Vec::new(); node_layer as usize + 1],
        };

        // If this is the first node, just insert it
        if meta.entry_point.is_none() {
            meta.entry_point = Some(entity_id);
            meta.max_layer = node_layer;
            meta.count = 1;
            self.save_node(&tree, &entity_id, &new_node)?;
            self.save_meta(&tree, &meta)?;
            return Ok(());
        }

        let entry_point = meta.entry_point.unwrap();
        let mut current_entry = entry_point;

        // Search from top layer down to node_layer + 1 (greedy search)
        // Use small ef to find a good entry point quickly
        for layer in (node_layer as usize + 1..=meta.max_layer as usize).rev() {
            let candidates = self.search_layer(
                &tree,
                vector,
                &[current_entry],
                10, // Use ef=10 for better entry point discovery
                layer,
                meta.metric,
            )?;

            if let Some(best) = candidates.first() {
                current_entry = best.entity_id;
            }
        }

        // Search and connect from node_layer down to layer 0
        for layer in (0..=node_layer as usize).rev() {
            let ef = if layer == 0 {
                self.config.ef_construction.max(self.config.m0)
            } else {
                self.config.ef_construction.max(self.config.m)
            };

            let candidates = self.search_layer(
                &tree,
                vector,
                &[current_entry],
                ef,
                layer,
                meta.metric,
            )?;

            // Select neighbors using simple heuristic
            let max_neighbors = if layer == 0 { self.config.m0 } else { self.config.m };
            let neighbors: Vec<[u8; 16]> = candidates
                .iter()
                .take(max_neighbors)
                .map(|c| c.entity_id)
                .collect();

            new_node.neighbors[layer] = neighbors.clone();

            // Add bidirectional connections
            for neighbor_id in &neighbors {
                if let Some(mut neighbor_node) = self.get_node(&tree, neighbor_id)? {
                    // Ensure neighbor has enough layers
                    while neighbor_node.neighbors.len() <= layer {
                        neighbor_node.neighbors.push(Vec::new());
                    }

                    // Add connection if not already present
                    if !neighbor_node.neighbors[layer].contains(&entity_id) {
                        neighbor_node.neighbors[layer].push(entity_id);

                        // Prune if too many neighbors, BUT always keep the new connection
                        // to maintain graph connectivity
                        if neighbor_node.neighbors[layer].len() > max_neighbors + 1 {
                            // Pruning: keep closest neighbors, but ALWAYS keep the new node
                            let neighbor_vec = &neighbor_node.vector;
                            let mut scored: Vec<_> = neighbor_node.neighbors[layer]
                                .iter()
                                .filter(|id| **id != entity_id) // Exclude new node from scoring
                                .filter_map(|id| {
                                    self.get_node(&tree, id).ok().flatten().map(|n| {
                                        let dist = self.distance(neighbor_vec, &n.vector, meta.metric);
                                        (*id, dist)
                                    })
                                })
                                .collect();
                            scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

                            // Keep the new node plus the closest (max_neighbors - 1) others
                            let mut new_neighbors: Vec<[u8; 16]> = vec![entity_id];
                            new_neighbors.extend(
                                scored
                                    .into_iter()
                                    .take(max_neighbors - 1)
                                    .map(|(id, _)| id),
                            );
                            neighbor_node.neighbors[layer] = new_neighbors;
                        }

                        self.save_node(&tree, neighbor_id, &neighbor_node)?;
                    }
                }
            }

            // Update entry point for next layer
            if !candidates.is_empty() {
                current_entry = candidates[0].entity_id;
            }
        }

        // Save the new node
        self.save_node(&tree, &entity_id, &new_node)?;

        // Update metadata
        if node_layer > meta.max_layer {
            meta.entry_point = Some(entity_id);
            meta.max_layer = node_layer;
        }
        meta.count += 1;
        self.save_meta(&tree, &meta)?;

        Ok(())
    }

    /// Search a single layer for nearest neighbors.
    fn search_layer(
        &self,
        tree: &Tree,
        query: &[f32],
        entry_points: &[[u8; 16]],
        ef: usize,
        layer: usize,
        metric: DistanceMetric,
    ) -> Result<Vec<Candidate>, Error> {
        let mut visited: HashSet<[u8; 16]> = HashSet::new();
        let mut candidates: BinaryHeap<Candidate> = BinaryHeap::new();
        let mut results: BinaryHeap<ReversedCandidate> = BinaryHeap::new();

        // Initialize with entry points
        for ep in entry_points {
            if let Some(node) = self.get_node(tree, ep)? {
                let dist = self.distance(query, &node.vector, metric);
                visited.insert(*ep);
                candidates.push(Candidate { entity_id: *ep, distance: dist });
                results.push(ReversedCandidate(Candidate { entity_id: *ep, distance: dist }));
            }
        }

        // Greedy search
        while let Some(current) = candidates.pop() {
            // Check stopping condition
            if let Some(worst) = results.peek() {
                if current.distance > worst.0.distance && results.len() >= ef {
                    break;
                }
            }

            // Expand neighbors
            if let Some(node) = self.get_node(tree, &current.entity_id)? {
                if layer < node.neighbors.len() {
                    for neighbor_id in &node.neighbors[layer] {
                        if visited.insert(*neighbor_id) {
                            if let Some(neighbor_node) = self.get_node(tree, neighbor_id)? {
                                let dist = self.distance(query, &neighbor_node.vector, metric);

                                let should_add = results.len() < ef || {
                                    if let Some(worst) = results.peek() {
                                        dist < worst.0.distance
                                    } else {
                                        true
                                    }
                                };

                                if should_add {
                                    candidates.push(Candidate { entity_id: *neighbor_id, distance: dist });
                                    results.push(ReversedCandidate(Candidate { entity_id: *neighbor_id, distance: dist }));

                                    // Keep results bounded
                                    while results.len() > ef {
                                        results.pop();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Convert results to sorted vec (smallest distance first)
        let mut result_vec: Vec<Candidate> = results.into_iter().map(|r| r.0).collect();
        result_vec.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));

        Ok(result_vec)
    }

    /// Search for k nearest neighbors.
    ///
    /// Returns a list of (entity_id, distance) pairs, sorted by distance (ascending).
    pub fn search(
        &self,
        entity_type: &str,
        column_name: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<([u8; 16], f32)>, Error> {
        self.search_with_threshold(entity_type, column_name, query, k, None)
    }

    /// Search for k nearest neighbors with optional distance threshold.
    pub fn search_with_threshold(
        &self,
        entity_type: &str,
        column_name: &str,
        query: &[f32],
        k: usize,
        max_distance: Option<f32>,
    ) -> Result<Vec<([u8; 16], f32)>, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        let meta = match self.get_meta(&tree)? {
            Some(m) => m,
            None => return Ok(vec![]), // Empty index
        };

        // Validate dimensions
        if meta.dimensions != query.len() as u32 {
            return Err(Error::InvalidData(format!(
                "Query dimension mismatch: expected {}, got {}",
                meta.dimensions,
                query.len()
            )));
        }

        let entry_point = match meta.entry_point {
            Some(ep) => ep,
            None => return Ok(vec![]), // Empty index
        };

        let mut current_entry = entry_point;

        // Search from top layer down to layer 1 (greedy)
        for layer in (1..=meta.max_layer as usize).rev() {
            let candidates = self.search_layer(
                &tree,
                query,
                &[current_entry],
                1,
                layer,
                meta.metric,
            )?;

            if let Some(best) = candidates.first() {
                current_entry = best.entity_id;
            }
        }

        // Search layer 0 with ef_search
        let ef = self.config.ef_search.max(k);
        let candidates = self.search_layer(
            &tree,
            query,
            &[current_entry],
            ef,
            0,
            meta.metric,
        )?;

        // Filter by distance threshold and take top k
        let results: Vec<([u8; 16], f32)> = candidates
            .into_iter()
            .filter(|c| max_distance.map_or(true, |max| c.distance <= max))
            .take(k)
            .map(|c| (c.entity_id, c.distance))
            .collect();

        Ok(results)
    }

    /// Remove a vector from the index.
    ///
    /// Note: This is a simple removal that may leave "holes" in the graph.
    /// For production use, consider periodic rebuilding.
    pub fn remove(
        &self,
        entity_type: &str,
        column_name: &str,
        entity_id: [u8; 16],
    ) -> Result<(), Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        // Get the node to remove
        let node = match self.get_node(&tree, &entity_id)? {
            Some(n) => n,
            None => return Ok(()), // Already removed
        };

        // Remove from neighbors' lists
        for (layer, neighbors) in node.neighbors.iter().enumerate() {
            for neighbor_id in neighbors {
                if let Some(mut neighbor_node) = self.get_node(&tree, neighbor_id)? {
                    if layer < neighbor_node.neighbors.len() {
                        neighbor_node.neighbors[layer].retain(|id| id != &entity_id);
                        self.save_node(&tree, neighbor_id, &neighbor_node)?;
                    }
                }
            }
        }

        // Remove the node itself
        let key = Self::node_key(&entity_id);
        tree.remove(&key)?;
        self.node_cache.remove(&entity_id);

        // Update metadata
        if let Some(mut meta) = self.get_meta(&tree)? {
            meta.count = meta.count.saturating_sub(1);

            // If this was the entry point, need to find a new one
            if meta.entry_point == Some(entity_id) {
                // Find any remaining node at the highest layer
                meta.entry_point = None;
                meta.max_layer = 0;

                for item in tree.scan_prefix(b"node:") {
                    if let Ok((key, value)) = item {
                        if key.len() == 21 { // "node:" + 16 bytes
                            let mut id = [0u8; 16];
                            id.copy_from_slice(&key[5..21]);

                            if let Ok(archived) = rkyv::access::<ArchivedHnswNode, rkyv::rancor::Error>(&value) {
                                if let Ok(n) = rkyv::deserialize::<HnswNode, rkyv::rancor::Error>(archived) {
                                    if n.layer >= meta.max_layer {
                                        meta.entry_point = Some(id);
                                        meta.max_layer = n.layer;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            self.save_meta(&tree, &meta)?;
        }

        Ok(())
    }

    /// Check if a vector index exists for a column.
    pub fn has_index(&self, entity_type: &str, column_name: &str) -> Result<bool, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;
        Ok(self.get_meta(&tree)?.is_some())
    }

    /// Get the count of vectors in an index.
    pub fn count(&self, entity_type: &str, column_name: &str) -> Result<u64, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;
        Ok(self.get_meta(&tree)?.map_or(0, |m| m.count))
    }

    /// Drop the index for an entity type and column.
    pub fn drop_index(&self, entity_type: &str, column_name: &str) -> Result<(), Error> {
        let tree_name = format!("{}{}:{}", VECTOR_INDEX_TREE_PREFIX, entity_type, column_name);
        self.db.drop_tree(tree_name)?;

        // Clear relevant cache entries
        // Note: This is a simplification - in production we'd track which IDs belong to which index
        self.node_cache.clear();

        Ok(())
    }

    /// Flush the index to disk.
    pub fn flush(&self) -> Result<(), Error> {
        self.db.flush()?;
        Ok(())
    }

    /// Clear the node cache.
    pub fn clear_cache(&self) {
        self.node_cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_index() -> VectorIndex {
        let db = sled::Config::new().temporary(true).open().unwrap();
        VectorIndex::open(&db, HnswConfig::default()).unwrap()
    }

    impl VectorIndex {
        /// Count reachable nodes from entry point (for debugging)
        fn count_reachable(&self, entity_type: &str, column_name: &str) -> Result<usize, Error> {
            let tree = self.tree_for_index(entity_type, column_name)?;
            let meta = match self.get_meta(&tree)? {
                Some(m) => m,
                None => return Ok(0),
            };
            let entry_point = match meta.entry_point {
                Some(ep) => ep,
                None => return Ok(0),
            };

            let mut visited = HashSet::new();
            let mut stack = vec![entry_point];

            while let Some(current) = stack.pop() {
                if !visited.insert(current) {
                    continue;
                }
                if let Some(node) = self.get_node(&tree, &current)? {
                    // Add all neighbors at layer 0
                    if !node.neighbors.is_empty() {
                        for neighbor_id in &node.neighbors[0] {
                            if !visited.contains(neighbor_id) {
                                stack.push(*neighbor_id);
                            }
                        }
                    }
                }
            }
            Ok(visited.len())
        }
    }

    fn random_vector(dim: usize) -> Vec<f32> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..dim).map(|_| rng.gen::<f32>()).collect()
    }

    fn normalize(v: &mut [f32]) {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }
    }

    #[test]
    fn test_insert_and_search_single() {
        let index = test_index();
        let entity_id = [1u8; 16];
        let vector = vec![1.0, 0.0, 0.0, 0.0];

        index.insert("Product", "embedding", entity_id, &vector).unwrap();

        let results = index.search("Product", "embedding", &vector, 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, entity_id);
        assert!(results[0].1 < 0.001); // Should be very close (same vector)
    }

    #[test]
    fn test_insert_and_search_multiple() {
        let index = test_index();
        let dim = 8;

        // Insert 100 vectors
        for i in 0..100 {
            let mut id = [0u8; 16];
            id[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            let mut v = random_vector(dim);
            normalize(&mut v);
            index.insert("Product", "embedding", id, &v).unwrap();
        }

        // Search with a random query
        let mut query = random_vector(dim);
        normalize(&mut query);

        let results = index.search("Product", "embedding", &query, 10).unwrap();
        assert_eq!(results.len(), 10);

        // Results should be sorted by distance
        for i in 1..results.len() {
            assert!(results[i].1 >= results[i - 1].1);
        }
    }

    #[test]
    fn test_distance_threshold() {
        let index = test_index();

        // Insert two very different vectors
        let id1 = [1u8; 16];
        let id2 = [2u8; 16];
        let v1 = vec![1.0, 0.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0, 0.0];

        index.insert("Product", "embedding", id1, &v1).unwrap();
        index.insert("Product", "embedding", id2, &v2).unwrap();

        // Search for v1 with low threshold
        let results = index
            .search_with_threshold("Product", "embedding", &v1, 10, Some(0.1))
            .unwrap();

        // Should only find v1 (distance 0)
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id1);
    }

    #[test]
    fn test_remove() {
        let index = test_index();
        let id1 = [1u8; 16];
        let id2 = [2u8; 16];
        let v1 = vec![1.0, 0.0, 0.0, 0.0];
        let v2 = vec![0.9, 0.1, 0.0, 0.0];

        index.insert("Product", "embedding", id1, &v1).unwrap();
        index.insert("Product", "embedding", id2, &v2).unwrap();

        assert_eq!(index.count("Product", "embedding").unwrap(), 2);

        // Remove first vector
        index.remove("Product", "embedding", id1).unwrap();

        assert_eq!(index.count("Product", "embedding").unwrap(), 1);

        // Search should only find v2
        let results = index.search("Product", "embedding", &v1, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id2);
    }

    #[test]
    fn test_dimension_validation() {
        let index = test_index();
        let id1 = [1u8; 16];
        let id2 = [2u8; 16];
        let v1 = vec![1.0, 0.0, 0.0, 0.0]; // 4 dimensions
        let v2 = vec![1.0, 0.0, 0.0]; // 3 dimensions

        index.insert("Product", "embedding", id1, &v1).unwrap();

        // Should fail due to dimension mismatch
        let result = index.insert("Product", "embedding", id2, &v2);
        assert!(result.is_err());
    }

    #[test]
    fn test_euclidean_distance() {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let config = HnswConfig::default().with_metric(DistanceMetric::Euclidean);
        let index = VectorIndex::open(&db, config).unwrap();

        let id1 = [1u8; 16];
        let id2 = [2u8; 16];
        let v1 = vec![0.0, 0.0, 0.0];
        let v2 = vec![3.0, 4.0, 0.0]; // Distance = 5

        index.insert("Product", "embedding", id1, &v1).unwrap();
        index.insert("Product", "embedding", id2, &v2).unwrap();

        let results = index.search("Product", "embedding", &v1, 10).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, id1);
        assert!(results[0].1 < 0.001); // v1 distance to itself
        assert!((results[1].1 - 5.0).abs() < 0.001); // v2 distance to v1
    }

    #[test]
    fn test_has_index() {
        let index = test_index();

        assert!(!index.has_index("Product", "embedding").unwrap());

        let id = [1u8; 16];
        let v = vec![1.0, 0.0, 0.0, 0.0];
        index.insert("Product", "embedding", id, &v).unwrap();

        assert!(index.has_index("Product", "embedding").unwrap());
        assert!(!index.has_index("Product", "other").unwrap());
    }

    #[test]
    fn test_drop_index() {
        let index = test_index();
        let id = [1u8; 16];
        let v = vec![1.0, 0.0, 0.0, 0.0];

        index.insert("Product", "embedding", id, &v).unwrap();
        assert!(index.has_index("Product", "embedding").unwrap());

        index.drop_index("Product", "embedding").unwrap();
        assert!(!index.has_index("Product", "embedding").unwrap());
    }

    #[test]
    fn test_empty_index_search() {
        let index = test_index();
        let query = vec![1.0, 0.0, 0.0, 0.0];

        let results = index.search("Product", "embedding", &query, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_graph_connectivity() {
        // Debug test to verify graph is connected
        let db = sled::Config::new().temporary(true).open().unwrap();
        let config = HnswConfig::default();
        let index = VectorIndex::open(&db, config).unwrap();

        let n = 10;
        let mut vectors: Vec<([u8; 16], Vec<f32>)> = Vec::new();

        // Insert a few normalized vectors (different directions)
        for i in 0..n {
            let mut id = [0u8; 16];
            id[0] = i as u8;
            // Create vectors that point in different directions
            let angle = (i as f32) * std::f32::consts::PI / (n as f32);
            let v = vec![angle.cos(), angle.sin(), 0.0, 0.0];
            index.insert("Test", "vec", id, &v).unwrap();
            vectors.push((id, v));
        }

        // Verify each vector can find itself
        for (id, v) in &vectors {
            let results = index.search("Test", "vec", v, 1).unwrap();
            assert!(!results.is_empty(), "Search returned no results for id {:?}", id);
            assert!(
                results[0].1 < 0.001,
                "Expected distance ~0 for same vector, got {} for id {:?}",
                results[0].1, id
            );
            assert_eq!(
                results[0].0, *id,
                "Expected id {:?}, got {:?} with distance {}",
                id, results[0].0, results[0].1
            );
        }
    }

    #[test]
    fn test_recall_quality() {
        // Test that HNSW achieves reasonable recall by querying with vectors IN the index
        let db = sled::Config::new().temporary(true).open().unwrap();
        let config = HnswConfig::default();
        let index = VectorIndex::open(&db, config).unwrap();

        let dim = 32;
        let n = 50;
        let mut vectors: Vec<([u8; 16], Vec<f32>)> = Vec::new();

        // Insert vectors and check connectivity after each insertion
        for i in 0..n {
            let mut id = [0u8; 16];
            id[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            let mut v = random_vector(dim);
            normalize(&mut v);
            index.insert("Product", "embedding", id, &v).unwrap();
            vectors.push((id.clone(), v.clone()));

            // Check connectivity - all nodes should be reachable
            let reachable = index.count_reachable("Product", "embedding").unwrap();
            let expected = i + 1;
            assert_eq!(
                reachable, expected,
                "After inserting {}, only {} of {} nodes are reachable",
                i, reachable, expected
            );
        }

        // Test recall with queries from the index
        let mut total_recall = 0.0;
        let num_queries = 10;
        let k = 10;

        for q in 0..num_queries {
            let (_, query) = &vectors[q * 4];

            // Compute exact top-k (brute force)
            let mut exact: Vec<_> = vectors
                .iter()
                .map(|(id, v)| {
                    let dist = index.distance(query, v, DistanceMetric::Cosine);
                    (*id, dist)
                })
                .collect();
            exact.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            let exact_topk: HashSet<[u8; 16]> = exact.iter().take(k).map(|(id, _)| *id).collect();

            // Get HNSW results
            let hnsw_results: HashSet<[u8; 16]> = index
                .search("Product", "embedding", query, k)
                .unwrap()
                .into_iter()
                .map(|(id, _)| id)
                .collect();

            let matches = hnsw_results.intersection(&exact_topk).count();
            total_recall += matches as f64 / k as f64;
        }

        let avg_recall = total_recall / num_queries as f64;
        assert!(avg_recall >= 0.5, "Recall too low: {:.2}", avg_recall);
    }
}
