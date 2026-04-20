//! R-tree geo index for spatial queries.
//!
//! This module implements an R-tree for efficient spatial indexing of geographic points,
//! supporting radius queries, bounding box queries, polygon containment, and nearest neighbor search.
//!
//! ## Algorithm Overview
//!
//! R-tree is a balanced tree where:
//! - Each node has a minimum bounding rectangle (MBR) that contains all its children
//! - Leaf nodes contain actual data entries (entity ID + point)
//! - Internal nodes contain child node references
//! - Queries prune branches by checking MBR intersections
//!
//! ## Storage Layout
//!
//! ```text
//! Tree: "index:geo:{entity_type}:{column_name}"
//!   "meta" -> GeoMeta (serialized)
//!   "node:{id}" -> RTreeNode (serialized)
//!   "entry:{entity_id}" -> GeoEntry (serialized, for reverse lookup)
//! ```

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use dashmap::DashMap;
use rkyv::{Archive, Deserialize, Serialize};
use sled::{Db, Tree};

use crate::error::Error;

/// Tree name prefix for geo indexes.
pub const GEO_INDEX_TREE_PREFIX: &str = "index:geo:";

/// Earth's radius in kilometers.
const EARTH_RADIUS_KM: f64 = 6371.0;

/// Configuration for R-tree index.
#[derive(Debug, Clone)]
pub struct RTreeConfig {
    /// Maximum entries per node (branching factor).
    pub max_entries: usize,
    /// Minimum entries per node (except root).
    pub min_entries: usize,
}

impl Default for RTreeConfig {
    fn default() -> Self {
        Self {
            max_entries: 50,
            min_entries: 20,
        }
    }
}

impl RTreeConfig {
    /// Create config for small datasets.
    pub fn small() -> Self {
        Self {
            max_entries: 10,
            min_entries: 4,
        }
    }

    /// Create config for large datasets.
    pub fn large() -> Self {
        Self {
            max_entries: 100,
            min_entries: 40,
        }
    }
}

/// A geographic point (latitude, longitude).
#[derive(Debug, Clone, Copy, PartialEq, Archive, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
}

impl GeoPoint {
    pub fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }
}

/// Minimum bounding rectangle for geographic regions.
#[derive(Debug, Clone, Copy, PartialEq, Archive, Serialize, Deserialize)]
pub struct MBR {
    pub min_lat: f64,
    pub max_lat: f64,
    pub min_lon: f64,
    pub max_lon: f64,
}

impl MBR {
    /// Create an MBR from a single point.
    pub fn from_point(point: GeoPoint) -> Self {
        Self {
            min_lat: point.lat,
            max_lat: point.lat,
            min_lon: point.lon,
            max_lon: point.lon,
        }
    }

    /// Create an MBR from two corner points.
    pub fn from_corners(p1: GeoPoint, p2: GeoPoint) -> Self {
        Self {
            min_lat: p1.lat.min(p2.lat),
            max_lat: p1.lat.max(p2.lat),
            min_lon: p1.lon.min(p2.lon),
            max_lon: p1.lon.max(p2.lon),
        }
    }

    /// Expand this MBR to include a point.
    pub fn expand_to_include_point(&mut self, point: GeoPoint) {
        self.min_lat = self.min_lat.min(point.lat);
        self.max_lat = self.max_lat.max(point.lat);
        self.min_lon = self.min_lon.min(point.lon);
        self.max_lon = self.max_lon.max(point.lon);
    }

    /// Expand this MBR to include another MBR.
    pub fn expand_to_include(&mut self, other: &MBR) {
        self.min_lat = self.min_lat.min(other.min_lat);
        self.max_lat = self.max_lat.max(other.max_lat);
        self.min_lon = self.min_lon.min(other.min_lon);
        self.max_lon = self.max_lon.max(other.max_lon);
    }

    /// Check if this MBR contains a point.
    pub fn contains_point(&self, point: GeoPoint) -> bool {
        point.lat >= self.min_lat
            && point.lat <= self.max_lat
            && point.lon >= self.min_lon
            && point.lon <= self.max_lon
    }

    /// Check if this MBR intersects another MBR.
    pub fn intersects(&self, other: &MBR) -> bool {
        self.min_lat <= other.max_lat
            && self.max_lat >= other.min_lat
            && self.min_lon <= other.max_lon
            && self.max_lon >= other.min_lon
    }

    /// Calculate the area of this MBR (in degrees squared, for comparison).
    pub fn area(&self) -> f64 {
        (self.max_lat - self.min_lat) * (self.max_lon - self.min_lon)
    }

    /// Calculate the enlargement needed to include a point.
    pub fn enlargement_for_point(&self, point: GeoPoint) -> f64 {
        let mut expanded = *self;
        expanded.expand_to_include_point(point);
        expanded.area() - self.area()
    }

    /// Calculate the enlargement needed to include another MBR.
    pub fn enlargement_for(&self, other: &MBR) -> f64 {
        let mut expanded = *self;
        expanded.expand_to_include(other);
        expanded.area() - self.area()
    }

    /// Get the center point of this MBR.
    pub fn center(&self) -> GeoPoint {
        GeoPoint {
            lat: (self.min_lat + self.max_lat) / 2.0,
            lon: (self.min_lon + self.max_lon) / 2.0,
        }
    }

    /// Calculate minimum distance from this MBR to a point (in km).
    pub fn min_distance_to_point(&self, point: GeoPoint) -> f64 {
        // Find the closest point on the MBR to the query point
        let closest_lat = point.lat.clamp(self.min_lat, self.max_lat);
        let closest_lon = point.lon.clamp(self.min_lon, self.max_lon);
        haversine_distance(point.lat, point.lon, closest_lat, closest_lon)
    }
}

/// Entry in an R-tree leaf node.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct RTreeEntry {
    entity_id: [u8; 16],
    point: GeoPoint,
}

/// R-tree node (can be leaf or internal).
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct RTreeNode {
    /// Node ID.
    id: u64,
    /// Whether this is a leaf node.
    is_leaf: bool,
    /// Minimum bounding rectangle for this node.
    mbr: MBR,
    /// Child node IDs (for internal nodes).
    children: Vec<u64>,
    /// Child MBRs (for internal nodes, parallel to children).
    child_mbrs: Vec<MBR>,
    /// Entries (for leaf nodes).
    entries: Vec<RTreeEntry>,
}

impl RTreeNode {
    fn new_leaf(id: u64) -> Self {
        Self {
            id,
            is_leaf: true,
            mbr: MBR {
                min_lat: f64::MAX,
                max_lat: f64::MIN,
                min_lon: f64::MAX,
                max_lon: f64::MIN,
            },
            children: Vec::new(),
            child_mbrs: Vec::new(),
            entries: Vec::new(),
        }
    }

    fn new_internal(id: u64) -> Self {
        Self {
            id,
            is_leaf: false,
            mbr: MBR {
                min_lat: f64::MAX,
                max_lat: f64::MIN,
                min_lon: f64::MAX,
                max_lon: f64::MIN,
            },
            children: Vec::new(),
            child_mbrs: Vec::new(),
            entries: Vec::new(),
        }
    }

    fn recalculate_mbr(&mut self) {
        if self.is_leaf {
            if self.entries.is_empty() {
                return;
            }
            self.mbr = MBR::from_point(self.entries[0].point);
            for entry in &self.entries[1..] {
                self.mbr.expand_to_include_point(entry.point);
            }
        } else {
            if self.child_mbrs.is_empty() {
                return;
            }
            self.mbr = self.child_mbrs[0];
            for mbr in &self.child_mbrs[1..] {
                self.mbr.expand_to_include(mbr);
            }
        }
    }

    fn len(&self) -> usize {
        if self.is_leaf {
            self.entries.len()
        } else {
            self.children.len()
        }
    }
}

/// Metadata for geo index.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct GeoMeta {
    /// Root node ID.
    root_id: Option<u64>,
    /// Next node ID to allocate.
    next_node_id: u64,
    /// Total number of entries.
    count: u64,
    /// Tree height.
    height: u32,
}

/// Candidate for nearest neighbor search.
struct NNCandidate {
    node_id: u64,
    min_distance: f64,
    is_leaf: bool,
}

impl PartialEq for NNCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.min_distance == other.min_distance
    }
}

impl Eq for NNCandidate {}

impl PartialOrd for NNCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NNCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for min-heap (smaller distance = higher priority)
        other
            .min_distance
            .partial_cmp(&self.min_distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// R-tree geo index for spatial queries.
pub struct GeoIndex {
    db: Db,
    /// Cache for frequently accessed nodes.
    node_cache: DashMap<u64, RTreeNode>,
    /// Configuration.
    config: RTreeConfig,
}

impl GeoIndex {
    /// Open or create a geo index.
    pub fn open(db: &Db, config: RTreeConfig) -> Result<Self, Error> {
        Ok(Self {
            db: db.clone(),
            node_cache: DashMap::new(),
            config,
        })
    }

    /// Get the sled tree for a specific entity type and column.
    fn tree_for_index(&self, entity_type: &str, column_name: &str) -> Result<Tree, Error> {
        let tree_name = format!("{}{}:{}", GEO_INDEX_TREE_PREFIX, entity_type, column_name);
        Ok(self.db.open_tree(tree_name)?)
    }

    /// Get metadata for an index.
    fn get_meta(&self, tree: &Tree) -> Result<Option<GeoMeta>, Error> {
        match tree.get("meta")? {
            Some(bytes) => {
                let archived = rkyv::access::<ArchivedGeoMeta, rkyv::rancor::Error>(&bytes)
                    .map_err(|e| Error::Deserialization(e.to_string()))?;
                let meta: GeoMeta = rkyv::deserialize::<GeoMeta, rkyv::rancor::Error>(archived)
                    .map_err(|e| Error::Deserialization(e.to_string()))?;
                Ok(Some(meta))
            }
            None => Ok(None),
        }
    }

    /// Save metadata for an index.
    fn save_meta(&self, tree: &Tree, meta: &GeoMeta) -> Result<(), Error> {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(meta)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        tree.insert("meta", bytes.as_slice())?;
        Ok(())
    }

    /// Build the key for a node.
    fn node_key(node_id: u64) -> Vec<u8> {
        let mut key = Vec::with_capacity(5 + 8);
        key.extend_from_slice(b"node:");
        key.extend_from_slice(&node_id.to_le_bytes());
        key
    }

    /// Build the key for an entry (reverse lookup).
    fn entry_key(entity_id: &[u8; 16]) -> Vec<u8> {
        let mut key = Vec::with_capacity(6 + 16);
        key.extend_from_slice(b"entry:");
        key.extend_from_slice(entity_id);
        key
    }

    /// Get a node from the index.
    fn get_node(&self, tree: &Tree, node_id: u64) -> Result<Option<RTreeNode>, Error> {
        // Check cache first
        if let Some(node) = self.node_cache.get(&node_id) {
            return Ok(Some(node.clone()));
        }

        // Load from disk
        let key = Self::node_key(node_id);
        match tree.get(&key)? {
            Some(bytes) => {
                let archived = rkyv::access::<ArchivedRTreeNode, rkyv::rancor::Error>(&bytes)
                    .map_err(|e| Error::Deserialization(e.to_string()))?;
                let node: RTreeNode =
                    rkyv::deserialize::<RTreeNode, rkyv::rancor::Error>(archived)
                        .map_err(|e| Error::Deserialization(e.to_string()))?;

                // Cache it
                self.node_cache.insert(node_id, node.clone());

                Ok(Some(node))
            }
            None => Ok(None),
        }
    }

    /// Save a node to the index.
    fn save_node(&self, tree: &Tree, node: &RTreeNode) -> Result<(), Error> {
        let key = Self::node_key(node.id);
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(node)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        tree.insert(key, bytes.as_slice())?;

        // Update cache
        self.node_cache.insert(node.id, node.clone());

        Ok(())
    }

    /// Delete a node.
    fn delete_node(&self, tree: &Tree, node_id: u64) -> Result<(), Error> {
        let key = Self::node_key(node_id);
        tree.remove(&key)?;
        self.node_cache.remove(&node_id);
        Ok(())
    }

    /// Insert a geo point into the index.
    pub fn insert(
        &self,
        entity_type: &str,
        column_name: &str,
        entity_id: [u8; 16],
        point: GeoPoint,
    ) -> Result<(), Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        // Get or create metadata
        let mut meta = self.get_meta(&tree)?.unwrap_or(GeoMeta {
            root_id: None,
            next_node_id: 0,
            count: 0,
            height: 0,
        });

        let entry = RTreeEntry { entity_id, point };

        // If tree is empty, create root
        if meta.root_id.is_none() {
            let mut root = RTreeNode::new_leaf(meta.next_node_id);
            meta.next_node_id += 1;
            root.entries.push(entry);
            root.recalculate_mbr();
            meta.root_id = Some(root.id);
            meta.height = 1;
            meta.count = 1;
            self.save_node(&tree, &root)?;
            self.save_meta(&tree, &meta)?;

            // Save reverse lookup
            tree.insert(Self::entry_key(&entity_id), &root.id.to_le_bytes())?;
            return Ok(());
        }

        // Find leaf node to insert into
        let root_id = meta.root_id.unwrap();
        let leaf_id = self.choose_leaf(&tree, root_id, point)?;

        // Insert into leaf
        let mut leaf = self
            .get_node(&tree, leaf_id)?
            .ok_or(Error::InvalidData("Leaf node not found".into()))?;
        leaf.entries.push(entry);
        leaf.recalculate_mbr();

        // Check if split is needed
        if leaf.len() > self.config.max_entries {
            // Split the leaf
            let (mut node1, mut node2) = self.split_leaf(&mut leaf, &mut meta);
            node1.recalculate_mbr();
            node2.recalculate_mbr();

            // Save reverse lookup for new entry
            let entry_leaf_id = if node1
                .entries
                .iter()
                .any(|e| e.entity_id == entity_id)
            {
                node1.id
            } else {
                node2.id
            };
            tree.insert(Self::entry_key(&entity_id), &entry_leaf_id.to_le_bytes())?;

            // Propagate split up the tree
            self.save_node(&tree, &node1)?;
            self.save_node(&tree, &node2)?;
            self.adjust_tree(&tree, &mut meta, node1.id, Some((node2.id, node2.mbr)))?;
        } else {
            self.save_node(&tree, &leaf)?;
            // Save reverse lookup
            tree.insert(Self::entry_key(&entity_id), &leaf.id.to_le_bytes())?;
            self.adjust_tree(&tree, &mut meta, leaf.id, None)?;
        }

        meta.count += 1;
        self.save_meta(&tree, &meta)?;

        Ok(())
    }

    /// Choose the best leaf node for inserting a point.
    fn choose_leaf(&self, tree: &Tree, root_id: u64, point: GeoPoint) -> Result<u64, Error> {
        let mut current_id = root_id;

        loop {
            let node = self
                .get_node(tree, current_id)?
                .ok_or(Error::InvalidData("Node not found".into()))?;

            if node.is_leaf {
                return Ok(current_id);
            }

            // Choose child with minimum enlargement
            let mut best_idx = 0;
            let mut best_enlargement = f64::MAX;
            let mut best_area = f64::MAX;

            for (i, mbr) in node.child_mbrs.iter().enumerate() {
                let enlargement = mbr.enlargement_for_point(point);
                let area = mbr.area();

                if enlargement < best_enlargement
                    || (enlargement == best_enlargement && area < best_area)
                {
                    best_idx = i;
                    best_enlargement = enlargement;
                    best_area = area;
                }
            }

            current_id = node.children[best_idx];
        }
    }

    /// Split a leaf node.
    fn split_leaf(&self, node: &mut RTreeNode, meta: &mut GeoMeta) -> (RTreeNode, RTreeNode) {
        // Use quadratic split: pick two seeds that are farthest apart
        let entries = std::mem::take(&mut node.entries);
        let (seed1, seed2) = self.pick_seeds_entries(&entries);

        let mut node1 = RTreeNode::new_leaf(node.id);
        let mut node2 = RTreeNode::new_leaf(meta.next_node_id);
        meta.next_node_id += 1;

        node1.entries.push(entries[seed1].clone());
        node2.entries.push(entries[seed2].clone());
        node1.recalculate_mbr();
        node2.recalculate_mbr();

        // Distribute remaining entries
        for (i, entry) in entries.into_iter().enumerate() {
            if i == seed1 || i == seed2 {
                continue;
            }

            // Check if one node needs all remaining entries
            let remaining = node1.entries.len() + node2.entries.len();
            if node1.entries.len() >= self.config.max_entries - remaining + self.config.min_entries
            {
                node2.entries.push(entry);
                continue;
            }
            if node2.entries.len() >= self.config.max_entries - remaining + self.config.min_entries
            {
                node1.entries.push(entry);
                continue;
            }

            // Choose node with minimum enlargement
            let enlarge1 = node1.mbr.enlargement_for_point(entry.point);
            let enlarge2 = node2.mbr.enlargement_for_point(entry.point);

            if enlarge1 < enlarge2 {
                node1.entries.push(entry);
                node1.recalculate_mbr();
            } else if enlarge2 < enlarge1 {
                node2.entries.push(entry);
                node2.recalculate_mbr();
            } else {
                // Tie: use smaller area
                if node1.mbr.area() <= node2.mbr.area() {
                    node1.entries.push(entry);
                    node1.recalculate_mbr();
                } else {
                    node2.entries.push(entry);
                    node2.recalculate_mbr();
                }
            }
        }

        (node1, node2)
    }

    /// Pick two seed entries for quadratic split.
    fn pick_seeds_entries(&self, entries: &[RTreeEntry]) -> (usize, usize) {
        let mut worst_waste = f64::MIN;
        let mut seed1 = 0;
        let mut seed2 = 1;

        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                let mbr1 = MBR::from_point(entries[i].point);
                let mbr2 = MBR::from_point(entries[j].point);
                let mut combined = mbr1;
                combined.expand_to_include(&mbr2);
                let waste = combined.area() - mbr1.area() - mbr2.area();

                if waste > worst_waste {
                    worst_waste = waste;
                    seed1 = i;
                    seed2 = j;
                }
            }
        }

        (seed1, seed2)
    }

    /// Adjust the tree after insertion, propagating MBR changes and splits.
    fn adjust_tree(
        &self,
        tree: &Tree,
        meta: &mut GeoMeta,
        node_id: u64,
        split_sibling: Option<(u64, MBR)>,
    ) -> Result<(), Error> {
        let node = self
            .get_node(tree, node_id)?
            .ok_or(Error::InvalidData("Node not found".into()))?;

        // Find parent
        let parent_id = self.find_parent(tree, meta, node_id)?;

        match parent_id {
            None => {
                // We're at root
                if let Some((sibling_id, sibling_mbr)) = split_sibling {
                    // Create new root
                    let mut new_root = RTreeNode::new_internal(meta.next_node_id);
                    meta.next_node_id += 1;
                    new_root.children.push(node_id);
                    new_root.child_mbrs.push(node.mbr);
                    new_root.children.push(sibling_id);
                    new_root.child_mbrs.push(sibling_mbr);
                    new_root.recalculate_mbr();
                    meta.root_id = Some(new_root.id);
                    meta.height += 1;
                    self.save_node(tree, &new_root)?;
                }
                Ok(())
            }
            Some(pid) => {
                let mut parent = self
                    .get_node(tree, pid)?
                    .ok_or(Error::InvalidData("Parent not found".into()))?;

                // Update MBR of child
                if let Some(idx) = parent.children.iter().position(|&id| id == node_id) {
                    parent.child_mbrs[idx] = node.mbr;
                }

                // Handle split sibling
                if let Some((sibling_id, sibling_mbr)) = split_sibling {
                    parent.children.push(sibling_id);
                    parent.child_mbrs.push(sibling_mbr);
                }

                parent.recalculate_mbr();

                // Check if parent needs split
                if parent.len() > self.config.max_entries {
                    let (mut p1, mut p2) = self.split_internal(&mut parent, meta);
                    p1.recalculate_mbr();
                    p2.recalculate_mbr();
                    self.save_node(tree, &p1)?;
                    self.save_node(tree, &p2)?;
                    self.adjust_tree(tree, meta, p1.id, Some((p2.id, p2.mbr)))?;
                } else {
                    self.save_node(tree, &parent)?;
                    self.adjust_tree(tree, meta, parent.id, None)?;
                }

                Ok(())
            }
        }
    }

    /// Split an internal node.
    fn split_internal(
        &self,
        node: &mut RTreeNode,
        meta: &mut GeoMeta,
    ) -> (RTreeNode, RTreeNode) {
        let children = std::mem::take(&mut node.children);
        let child_mbrs = std::mem::take(&mut node.child_mbrs);

        let (seed1, seed2) = self.pick_seeds_mbrs(&child_mbrs);

        let mut node1 = RTreeNode::new_internal(node.id);
        let mut node2 = RTreeNode::new_internal(meta.next_node_id);
        meta.next_node_id += 1;

        node1.children.push(children[seed1]);
        node1.child_mbrs.push(child_mbrs[seed1]);
        node2.children.push(children[seed2]);
        node2.child_mbrs.push(child_mbrs[seed2]);
        node1.recalculate_mbr();
        node2.recalculate_mbr();

        for (i, (child, mbr)) in children.into_iter().zip(child_mbrs).enumerate() {
            if i == seed1 || i == seed2 {
                continue;
            }

            let enlarge1 = node1.mbr.enlargement_for(&mbr);
            let enlarge2 = node2.mbr.enlargement_for(&mbr);

            if enlarge1 < enlarge2 {
                node1.children.push(child);
                node1.child_mbrs.push(mbr);
                node1.recalculate_mbr();
            } else {
                node2.children.push(child);
                node2.child_mbrs.push(mbr);
                node2.recalculate_mbr();
            }
        }

        (node1, node2)
    }

    /// Pick two seed MBRs for quadratic split.
    fn pick_seeds_mbrs(&self, mbrs: &[MBR]) -> (usize, usize) {
        let mut worst_waste = f64::MIN;
        let mut seed1 = 0;
        let mut seed2 = 1;

        for i in 0..mbrs.len() {
            for j in (i + 1)..mbrs.len() {
                let mut combined = mbrs[i];
                combined.expand_to_include(&mbrs[j]);
                let waste = combined.area() - mbrs[i].area() - mbrs[j].area();

                if waste > worst_waste {
                    worst_waste = waste;
                    seed1 = i;
                    seed2 = j;
                }
            }
        }

        (seed1, seed2)
    }

    /// Find the parent of a node.
    fn find_parent(
        &self,
        tree: &Tree,
        meta: &GeoMeta,
        node_id: u64,
    ) -> Result<Option<u64>, Error> {
        let root_id = match meta.root_id {
            Some(id) => id,
            None => return Ok(None),
        };

        if node_id == root_id {
            return Ok(None);
        }

        // BFS to find parent
        let mut queue = vec![root_id];
        while let Some(current_id) = queue.pop() {
            let node = match self.get_node(tree, current_id)? {
                Some(n) => n,
                None => continue,
            };

            if !node.is_leaf {
                if node.children.contains(&node_id) {
                    return Ok(Some(current_id));
                }
                queue.extend(&node.children);
            }
        }

        Ok(None)
    }

    /// Find all points within a given radius of a center point.
    pub fn within_radius(
        &self,
        entity_type: &str,
        column_name: &str,
        center: GeoPoint,
        radius_km: f64,
    ) -> Result<Vec<([u8; 16], f64)>, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        let meta = match self.get_meta(&tree)? {
            Some(m) => m,
            None => return Ok(vec![]),
        };

        let root_id = match meta.root_id {
            Some(id) => id,
            None => return Ok(vec![]),
        };

        let mut results = Vec::new();
        let mut stack = vec![root_id];

        while let Some(node_id) = stack.pop() {
            let node = match self.get_node(&tree, node_id)? {
                Some(n) => n,
                None => continue,
            };

            // Check if MBR could contain points within radius
            if node.mbr.min_distance_to_point(center) > radius_km {
                continue;
            }

            if node.is_leaf {
                for entry in &node.entries {
                    let dist = haversine_distance(
                        center.lat,
                        center.lon,
                        entry.point.lat,
                        entry.point.lon,
                    );
                    if dist <= radius_km {
                        results.push((entry.entity_id, dist));
                    }
                }
            } else {
                stack.extend(&node.children);
            }
        }

        // Sort by distance
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        Ok(results)
    }

    /// Find all points within a bounding box.
    pub fn within_box(
        &self,
        entity_type: &str,
        column_name: &str,
        bbox: MBR,
    ) -> Result<Vec<[u8; 16]>, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        let meta = match self.get_meta(&tree)? {
            Some(m) => m,
            None => return Ok(vec![]),
        };

        let root_id = match meta.root_id {
            Some(id) => id,
            None => return Ok(vec![]),
        };

        let mut results = Vec::new();
        let mut stack = vec![root_id];

        while let Some(node_id) = stack.pop() {
            let node = match self.get_node(&tree, node_id)? {
                Some(n) => n,
                None => continue,
            };

            // Check if MBR intersects query box
            if !node.mbr.intersects(&bbox) {
                continue;
            }

            if node.is_leaf {
                for entry in &node.entries {
                    if bbox.contains_point(entry.point) {
                        results.push(entry.entity_id);
                    }
                }
            } else {
                stack.extend(&node.children);
            }
        }

        Ok(results)
    }

    /// Find all points within a polygon.
    pub fn within_polygon(
        &self,
        entity_type: &str,
        column_name: &str,
        vertices: &[(f64, f64)],
    ) -> Result<Vec<[u8; 16]>, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        let meta = match self.get_meta(&tree)? {
            Some(m) => m,
            None => return Ok(vec![]),
        };

        let root_id = match meta.root_id {
            Some(id) => id,
            None => return Ok(vec![]),
        };

        // Compute polygon bounding box for quick pruning
        let poly_mbr = polygon_mbr(vertices);

        let mut results = Vec::new();
        let mut stack = vec![root_id];

        while let Some(node_id) = stack.pop() {
            let node = match self.get_node(&tree, node_id)? {
                Some(n) => n,
                None => continue,
            };

            // Check if MBR intersects polygon bounding box
            if !node.mbr.intersects(&poly_mbr) {
                continue;
            }

            if node.is_leaf {
                for entry in &node.entries {
                    if point_in_polygon(entry.point.lat, entry.point.lon, vertices) {
                        results.push(entry.entity_id);
                    }
                }
            } else {
                stack.extend(&node.children);
            }
        }

        Ok(results)
    }

    /// Find k nearest neighbors to a point.
    pub fn nearest(
        &self,
        entity_type: &str,
        column_name: &str,
        query: GeoPoint,
        k: usize,
    ) -> Result<Vec<([u8; 16], f64)>, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        let meta = match self.get_meta(&tree)? {
            Some(m) => m,
            None => return Ok(vec![]),
        };

        let root_id = match meta.root_id {
            Some(id) => id,
            None => return Ok(vec![]),
        };

        // Priority queue for branch-and-bound search
        let mut heap = BinaryHeap::new();
        let root = self
            .get_node(&tree, root_id)?
            .ok_or(Error::InvalidData("Root not found".into()))?;

        heap.push(NNCandidate {
            node_id: root_id,
            min_distance: root.mbr.min_distance_to_point(query),
            is_leaf: root.is_leaf,
        });

        let mut results: Vec<([u8; 16], f64)> = Vec::new();
        let mut worst_distance = f64::MAX;

        while let Some(candidate) = heap.pop() {
            // Prune if this node can't have closer points
            if candidate.min_distance > worst_distance && results.len() >= k {
                continue;
            }

            let node = match self.get_node(&tree, candidate.node_id)? {
                Some(n) => n,
                None => continue,
            };

            if node.is_leaf {
                for entry in &node.entries {
                    let dist = haversine_distance(
                        query.lat,
                        query.lon,
                        entry.point.lat,
                        entry.point.lon,
                    );

                    if results.len() < k || dist < worst_distance {
                        results.push((entry.entity_id, dist));
                        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
                        if results.len() > k {
                            results.pop();
                        }
                        if results.len() == k {
                            worst_distance = results[k - 1].1;
                        }
                    }
                }
            } else {
                // Add children to heap
                for (i, &child_id) in node.children.iter().enumerate() {
                    let child_mbr = &node.child_mbrs[i];
                    let min_dist = child_mbr.min_distance_to_point(query);

                    if min_dist <= worst_distance || results.len() < k {
                        let child = self.get_node(&tree, child_id)?;
                        heap.push(NNCandidate {
                            node_id: child_id,
                            min_distance: min_dist,
                            is_leaf: child.map_or(true, |c| c.is_leaf),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Remove a point from the index.
    pub fn remove(
        &self,
        entity_type: &str,
        column_name: &str,
        entity_id: [u8; 16],
    ) -> Result<(), Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;

        // Find the leaf containing this entry via reverse lookup
        let entry_key = Self::entry_key(&entity_id);
        let leaf_id = match tree.get(&entry_key)? {
            Some(bytes) => {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes);
                u64::from_le_bytes(arr)
            }
            None => return Ok(()), // Already removed
        };

        // Remove from leaf
        let mut leaf = match self.get_node(&tree, leaf_id)? {
            Some(n) => n,
            None => return Ok(()),
        };

        leaf.entries.retain(|e| e.entity_id != entity_id);
        leaf.recalculate_mbr();
        self.save_node(&tree, &leaf)?;

        // Remove reverse lookup
        tree.remove(&entry_key)?;

        // Update metadata
        if let Some(mut meta) = self.get_meta(&tree)? {
            meta.count = meta.count.saturating_sub(1);
            self.save_meta(&tree, &meta)?;
        }

        Ok(())
    }

    /// Check if a geo index exists for a column.
    pub fn has_index(&self, entity_type: &str, column_name: &str) -> Result<bool, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;
        Ok(self.get_meta(&tree)?.is_some())
    }

    /// Get the count of points in an index.
    pub fn count(&self, entity_type: &str, column_name: &str) -> Result<u64, Error> {
        let tree = self.tree_for_index(entity_type, column_name)?;
        Ok(self.get_meta(&tree)?.map_or(0, |m| m.count))
    }

    /// Drop the index for an entity type and column.
    pub fn drop_index(&self, entity_type: &str, column_name: &str) -> Result<(), Error> {
        let tree_name = format!("{}{}:{}", GEO_INDEX_TREE_PREFIX, entity_type, column_name);
        self.db.drop_tree(tree_name)?;
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

/// Calculate the haversine distance between two points in kilometers.
pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();

    EARTH_RADIUS_KM * c
}

/// Compute the MBR of a polygon.
fn polygon_mbr(vertices: &[(f64, f64)]) -> MBR {
    let mut mbr = MBR {
        min_lat: f64::MAX,
        max_lat: f64::MIN,
        min_lon: f64::MAX,
        max_lon: f64::MIN,
    };

    for &(lat, lon) in vertices {
        mbr.min_lat = mbr.min_lat.min(lat);
        mbr.max_lat = mbr.max_lat.max(lat);
        mbr.min_lon = mbr.min_lon.min(lon);
        mbr.max_lon = mbr.max_lon.max(lon);
    }

    mbr
}

/// Check if a point is inside a polygon using ray casting.
pub fn point_in_polygon(lat: f64, lon: f64, vertices: &[(f64, f64)]) -> bool {
    let n = vertices.len();
    if n < 3 {
        return false;
    }

    let mut inside = false;
    let mut j = n - 1;

    for i in 0..n {
        let (yi, xi) = vertices[i];
        let (yj, xj) = vertices[j];

        if ((yi > lat) != (yj > lat)) && (lon < (xj - xi) * (lat - yi) / (yj - yi) + xi) {
            inside = !inside;
        }

        j = i;
    }

    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_index() -> GeoIndex {
        let db = sled::Config::new().temporary(true).open().unwrap();
        GeoIndex::open(&db, RTreeConfig::small()).unwrap()
    }

    #[test]
    fn test_insert_and_search_single() {
        let index = test_index();
        let entity_id = [1u8; 16];
        let point = GeoPoint::new(37.7749, -122.4194); // San Francisco

        index
            .insert("Restaurant", "location", entity_id, point)
            .unwrap();

        let results = index
            .within_radius("Restaurant", "location", point, 1.0)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, entity_id);
        assert!(results[0].1 < 0.001); // Should be very close
    }

    #[test]
    fn test_insert_multiple_and_radius_search() {
        let index = test_index();

        // Insert several points in San Francisco area
        let points = vec![
            ([1u8; 16], GeoPoint::new(37.7749, -122.4194)), // SF downtown
            ([2u8; 16], GeoPoint::new(37.7849, -122.4094)), // ~1.5km away
            ([3u8; 16], GeoPoint::new(37.8049, -122.4294)), // ~3.5km away
            ([4u8; 16], GeoPoint::new(37.8749, -122.2594)), // ~20km away (Oakland)
        ];

        for (id, point) in &points {
            let mut eid = [0u8; 16];
            eid[0] = id[0];
            index.insert("Restaurant", "location", eid, *point).unwrap();
        }

        // Search within 5km of SF downtown
        let center = GeoPoint::new(37.7749, -122.4194);
        let results = index
            .within_radius("Restaurant", "location", center, 5.0)
            .unwrap();

        assert_eq!(results.len(), 3); // Should find first 3 points
        assert!(results.iter().all(|(_, dist)| *dist <= 5.0));

        // Oakland should not be included
        let oakland_id = [4u8; 16];
        assert!(!results.iter().any(|(id, _)| *id == oakland_id));
    }

    #[test]
    fn test_bounding_box_search() {
        let index = test_index();

        // Insert points
        let points = vec![
            ([1u8; 16], GeoPoint::new(37.75, -122.42)),
            ([2u8; 16], GeoPoint::new(37.78, -122.40)),
            ([3u8; 16], GeoPoint::new(37.80, -122.45)), // Outside box
            ([4u8; 16], GeoPoint::new(37.70, -122.50)), // Outside box
        ];

        for (id, point) in &points {
            let mut eid = [0u8; 16];
            eid[0] = id[0];
            index.insert("Restaurant", "location", eid, *point).unwrap();
        }

        let bbox = MBR::from_corners(
            GeoPoint::new(37.74, -122.43),
            GeoPoint::new(37.79, -122.39),
        );

        let results = index.within_box("Restaurant", "location", bbox).unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_polygon_containment() {
        let index = test_index();

        // Insert points
        let points = vec![
            ([1u8; 16], GeoPoint::new(37.76, -122.41)), // Inside triangle
            ([2u8; 16], GeoPoint::new(37.77, -122.42)), // Inside triangle
            ([3u8; 16], GeoPoint::new(37.90, -122.50)), // Outside triangle
        ];

        for (id, point) in &points {
            let mut eid = [0u8; 16];
            eid[0] = id[0];
            index.insert("Restaurant", "location", eid, *point).unwrap();
        }

        // Triangle around part of SF
        let triangle = vec![
            (37.75, -122.45),
            (37.80, -122.40),
            (37.75, -122.35),
        ];

        let results = index
            .within_polygon("Restaurant", "location", &triangle)
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_nearest_neighbors() {
        let index = test_index();

        // Insert points at increasing distances
        let points = vec![
            ([1u8; 16], GeoPoint::new(37.7749, -122.4194)), // 0km
            ([2u8; 16], GeoPoint::new(37.7849, -122.4194)), // ~1.1km
            ([3u8; 16], GeoPoint::new(37.7949, -122.4194)), // ~2.2km
            ([4u8; 16], GeoPoint::new(37.8049, -122.4194)), // ~3.3km
            ([5u8; 16], GeoPoint::new(37.8149, -122.4194)), // ~4.4km
        ];

        for (id, point) in &points {
            let mut eid = [0u8; 16];
            eid[0] = id[0];
            index.insert("Restaurant", "location", eid, *point).unwrap();
        }

        let query = GeoPoint::new(37.7749, -122.4194);
        let results = index.nearest("Restaurant", "location", query, 3).unwrap();

        assert_eq!(results.len(), 3);
        // Results should be sorted by distance
        for i in 1..results.len() {
            assert!(results[i].1 >= results[i - 1].1);
        }
        // First result should be the query point itself (or very close)
        assert!(results[0].1 < 0.001);
    }

    #[test]
    fn test_remove() {
        let index = test_index();
        let id1 = [1u8; 16];
        let id2 = [2u8; 16];
        let point1 = GeoPoint::new(37.7749, -122.4194);
        let point2 = GeoPoint::new(37.7849, -122.4094);

        index.insert("Restaurant", "location", id1, point1).unwrap();
        index.insert("Restaurant", "location", id2, point2).unwrap();

        assert_eq!(index.count("Restaurant", "location").unwrap(), 2);

        index.remove("Restaurant", "location", id1).unwrap();

        assert_eq!(index.count("Restaurant", "location").unwrap(), 1);

        let results = index
            .within_radius("Restaurant", "location", point1, 10.0)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id2);
    }

    #[test]
    fn test_has_index() {
        let index = test_index();

        assert!(!index.has_index("Restaurant", "location").unwrap());

        let id = [1u8; 16];
        let point = GeoPoint::new(37.7749, -122.4194);
        index.insert("Restaurant", "location", id, point).unwrap();

        assert!(index.has_index("Restaurant", "location").unwrap());
        assert!(!index.has_index("Restaurant", "other").unwrap());
    }

    #[test]
    fn test_drop_index() {
        let index = test_index();
        let id = [1u8; 16];
        let point = GeoPoint::new(37.7749, -122.4194);

        index.insert("Restaurant", "location", id, point).unwrap();
        assert!(index.has_index("Restaurant", "location").unwrap());

        index.drop_index("Restaurant", "location").unwrap();
        assert!(!index.has_index("Restaurant", "location").unwrap());
    }

    #[test]
    fn test_empty_index_search() {
        let index = test_index();
        let query = GeoPoint::new(37.7749, -122.4194);

        let results = index
            .within_radius("Restaurant", "location", query, 10.0)
            .unwrap();
        assert!(results.is_empty());

        let results = index.nearest("Restaurant", "location", query, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_haversine_distance() {
        // SF to LA is about 559km
        let sf = GeoPoint::new(37.7749, -122.4194);
        let la = GeoPoint::new(34.0522, -118.2437);

        let dist = haversine_distance(sf.lat, sf.lon, la.lat, la.lon);
        assert!((dist - 559.0).abs() < 5.0); // Within 5km tolerance
    }

    #[test]
    fn test_point_in_polygon() {
        // Square from (0,0) to (10,10)
        let square = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];

        assert!(point_in_polygon(5.0, 5.0, &square)); // Center
        assert!(point_in_polygon(1.0, 1.0, &square)); // Inside
        assert!(!point_in_polygon(15.0, 15.0, &square)); // Outside
        assert!(!point_in_polygon(-1.0, 5.0, &square)); // Outside
    }

    #[test]
    fn test_large_dataset() {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let index = GeoIndex::open(&db, RTreeConfig::default()).unwrap();

        // Insert 500 random points in SF bay area
        let base_lat = 37.5;
        let base_lon = -122.5;

        for i in 0..500 {
            let mut id = [0u8; 16];
            id[0..2].copy_from_slice(&(i as u16).to_le_bytes());
            let lat = base_lat + (i as f64 % 50.0) * 0.01;
            let lon = base_lon + (i as f64 / 50.0).floor() * 0.01;
            let point = GeoPoint::new(lat, lon);
            index.insert("Place", "coord", id, point).unwrap();
        }

        assert_eq!(index.count("Place", "coord").unwrap(), 500);

        // Search near center
        let center = GeoPoint::new(37.75, -122.25);
        let results = index.within_radius("Place", "coord", center, 50.0).unwrap();

        // Should find some points
        assert!(!results.is_empty());

        // Nearest neighbor should work
        let nn_results = index.nearest("Place", "coord", center, 10).unwrap();
        assert_eq!(nn_results.len(), 10);
    }
}
