//! OLTP (Online Transaction Processing) workload simulation.
//!
//! Provides realistic mixed read/write workloads for benchmarking
//! the embedded ORMDB database.

use ormdb_core::storage::Record;
use ormdb_core::StorageEngine;
use rand::prelude::*;
use rkyv::{Archive, Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// OLTP workload configuration.
#[derive(Debug, Clone)]
pub struct OltpConfig {
    /// Number of customers in the database.
    pub num_customers: usize,
    /// Number of products in the database.
    pub num_products: usize,
    /// Number of initial orders.
    pub num_orders: usize,
    /// Percentage of read operations (0.0 - 1.0).
    pub read_pct: f64,
    /// Operations per benchmark iteration.
    pub ops_per_iteration: usize,
}

impl OltpConfig {
    /// Small workload for quick testing.
    pub fn small() -> Self {
        Self {
            num_customers: 100,
            num_products: 50,
            num_orders: 200,
            read_pct: 0.80,
            ops_per_iteration: 100,
        }
    }

    /// Medium workload for standard benchmarking.
    pub fn medium() -> Self {
        Self {
            num_customers: 1000,
            num_products: 200,
            num_orders: 5000,
            read_pct: 0.80,
            ops_per_iteration: 100,
        }
    }

    /// Large workload for stress testing.
    pub fn large() -> Self {
        Self {
            num_customers: 10000,
            num_products: 1000,
            num_orders: 50000,
            read_pct: 0.80,
            ops_per_iteration: 100,
        }
    }
}

/// Statistics from running a workload.
#[derive(Debug, Default, Clone)]
pub struct WorkloadStats {
    /// Total operations completed.
    pub ops_completed: usize,
    /// Total time spent.
    pub total_duration: Duration,
    /// Number of read operations.
    pub reads: usize,
    /// Number of write operations.
    pub writes: usize,
    /// Number of transaction conflicts.
    pub conflicts: usize,
}

impl WorkloadStats {
    /// Calculate operations per second.
    pub fn ops_per_sec(&self) -> f64 {
        if self.total_duration.as_secs_f64() > 0.0 {
            self.ops_completed as f64 / self.total_duration.as_secs_f64()
        } else {
            0.0
        }
    }
}

/// Customer entity for OLTP workload.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct Customer {
    pub name: String,
    pub email: String,
    pub balance: f64,
    pub status: String,
}

/// Product entity for OLTP workload.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct Product {
    pub name: String,
    pub price: f64,
    pub inventory: i32,
    pub category: String,
}

/// Order entity for OLTP workload.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct Order {
    pub customer_id: [u8; 16],
    pub total: f64,
    pub status: String,
}

/// OLTP workload runner for embedded ORMDB.
pub struct OltpWorkload {
    /// Storage engine.
    storage: Arc<StorageEngine>,
    /// Random number generator.
    rng: StdRng,
    /// Customer IDs for lookups.
    customer_ids: Vec<[u8; 16]>,
    /// Product IDs for lookups.
    product_ids: Vec<[u8; 16]>,
    /// Order IDs for lookups.
    order_ids: Vec<[u8; 16]>,
    /// Configuration.
    config: OltpConfig,
    /// Counter for generating unique IDs.
    id_counter: AtomicUsize,
}

impl OltpWorkload {
    /// Create a new OLTP workload with the given configuration.
    pub fn new(storage: Arc<StorageEngine>, config: OltpConfig) -> Self {
        let mut workload = Self {
            storage,
            rng: StdRng::seed_from_u64(12345),
            customer_ids: Vec::new(),
            product_ids: Vec::new(),
            order_ids: Vec::new(),
            config,
            id_counter: AtomicUsize::new(0),
        };
        workload.populate();
        workload
    }

    /// Generate a unique ID.
    fn generate_id(&self) -> [u8; 16] {
        let counter = self.id_counter.fetch_add(1, Ordering::SeqCst);
        let mut id = [0u8; 16];
        id[0..8].copy_from_slice(&counter.to_le_bytes());
        id[8..16].copy_from_slice(&0xDEADBEEF_u64.to_le_bytes());
        id
    }

    /// Serialize data to bytes using rkyv.
    fn serialize_customer(data: &Customer) -> Vec<u8> {
        rkyv::to_bytes::<rkyv::rancor::Error>(data)
            .map(|v| v.to_vec())
            .unwrap_or_default()
    }

    /// Serialize product to bytes using rkyv.
    fn serialize_product(data: &Product) -> Vec<u8> {
        rkyv::to_bytes::<rkyv::rancor::Error>(data)
            .map(|v| v.to_vec())
            .unwrap_or_default()
    }

    /// Serialize order to bytes using rkyv.
    fn serialize_order(data: &Order) -> Vec<u8> {
        rkyv::to_bytes::<rkyv::rancor::Error>(data)
            .map(|v| v.to_vec())
            .unwrap_or_default()
    }

    /// Populate the database with initial data.
    fn populate(&mut self) {
        // Populate customers
        for i in 0..self.config.num_customers {
            let id = self.generate_id();
            let customer = Customer {
                name: format!("Customer {}", i),
                email: format!("customer{}@example.com", i),
                balance: (i as f64) * 100.0,
                status: if i % 10 == 0 {
                    "inactive".to_string()
                } else {
                    "active".to_string()
                },
            };
            let data = Self::serialize_customer(&customer);
            let record = Record::new(data);
            let key = ormdb_core::storage::key::VersionedKey::new(id, 1);
            let _ = self.storage.put(key, record);
            self.customer_ids.push(id);
        }

        // Populate products
        for i in 0..self.config.num_products {
            let id = self.generate_id();
            let product = Product {
                name: format!("Product {}", i),
                price: ((i % 100) + 1) as f64 * 9.99,
                inventory: ((i % 500) + 10) as i32,
                category: ["Electronics", "Clothing", "Books", "Home", "Sports"][i % 5].to_string(),
            };
            let data = Self::serialize_product(&product);
            let record = Record::new(data);
            let key = ormdb_core::storage::key::VersionedKey::new(id, 1);
            let _ = self.storage.put(key, record);
            self.product_ids.push(id);
        }

        // Populate orders
        for i in 0..self.config.num_orders {
            let id = self.generate_id();
            let customer_idx = i % self.config.num_customers.max(1);
            let customer_id = self.customer_ids.get(customer_idx).copied().unwrap_or([0; 16]);
            let order = Order {
                customer_id,
                total: (i as f64) * 25.0 + 10.0,
                status: ["pending", "processing", "shipped", "delivered"][i % 4].to_string(),
            };
            let data = Self::serialize_order(&order);
            let record = Record::new(data);
            let key = ormdb_core::storage::key::VersionedKey::new(id, 1);
            let _ = self.storage.put(key, record);
            self.order_ids.push(id);
        }

        // Flush to ensure data is persisted
        let _ = self.storage.flush();
    }

    /// Perform a point read operation.
    pub fn point_read(&mut self) -> Duration {
        let start = Instant::now();

        // Random entity type and ID
        let choice = self.rng.gen_range(0..3);
        let id = match choice {
            0 => self.customer_ids[self.rng.gen_range(0..self.customer_ids.len())],
            1 => self.product_ids[self.rng.gen_range(0..self.product_ids.len())],
            _ => self.order_ids[self.rng.gen_range(0..self.order_ids.len())],
        };

        let _ = self.storage.get_latest(&id);
        start.elapsed()
    }

    /// Perform a range scan operation.
    pub fn range_scan(&mut self) -> Duration {
        let start = Instant::now();

        // Scan products (simulating a catalog browse)
        let limit = self.rng.gen_range(10..50);
        let mut count = 0;
        for id in self.product_ids.iter().take(limit) {
            let _ = self.storage.get_latest(id);
            count += 1;
        }
        let _ = count; // Use count to prevent optimization

        start.elapsed()
    }

    /// Perform an insert operation.
    pub fn insert_order(&mut self) -> Duration {
        let start = Instant::now();

        let id = self.generate_id();
        let index = self.order_ids.len();
        let customer_idx = index % self.config.num_customers.max(1);
        let customer_id = self.customer_ids.get(customer_idx).copied().unwrap_or([0; 16]);
        let order = Order {
            customer_id,
            total: (index as f64) * 25.0 + 10.0,
            status: ["pending", "processing", "shipped", "delivered"][index % 4].to_string(),
        };
        let data = Self::serialize_order(&order);
        let record = Record::new(data);
        let key = ormdb_core::storage::key::VersionedKey::new(id, 1);
        let _ = self.storage.put(key, record);
        self.order_ids.push(id);

        start.elapsed()
    }

    /// Perform an update operation.
    pub fn update_inventory(&mut self) -> Duration {
        let start = Instant::now();

        if self.product_ids.is_empty() {
            return start.elapsed();
        }

        let idx = self.rng.gen_range(0..self.product_ids.len());
        let id = self.product_ids[idx];

        // Get current record and update
        if let Ok(Some((version, record))) = self.storage.get_latest(&id) {
            // Deserialize product
            if let Ok(archived) =
                rkyv::access::<ArchivedProduct, rkyv::rancor::Error>(&record.data)
            {
                if let Ok(mut product) =
                    rkyv::deserialize::<Product, rkyv::rancor::Error>(archived)
                {
                    // Update inventory
                    product.inventory = product.inventory.saturating_sub(1);

                    // Put with new version
                    let data = Self::serialize_product(&product);
                    let new_record = Record::new(data);
                    let key = ormdb_core::storage::key::VersionedKey::new(id, version + 1);
                    let _ = self.storage.put(key, new_record);
                }
            }
        }

        start.elapsed()
    }

    /// Perform a delete operation.
    pub fn delete_order(&mut self) -> Duration {
        let start = Instant::now();

        if self.order_ids.is_empty() {
            return start.elapsed();
        }

        // Remove a random order (from the end to avoid index issues)
        let idx = self.rng.gen_range(0..self.order_ids.len());
        let id = self.order_ids.swap_remove(idx);
        let _ = self.storage.delete(&id);

        start.elapsed()
    }

    /// Run a mixed workload.
    pub fn run_mixed_ops(&mut self, count: usize) -> WorkloadStats {
        let start = Instant::now();
        let mut stats = WorkloadStats::default();

        for _ in 0..count {
            let roll: f64 = self.rng.gen();

            if roll < self.config.read_pct {
                // Read operation
                if roll < self.config.read_pct * 0.75 {
                    // 75% of reads are point reads
                    self.point_read();
                } else {
                    // 25% of reads are range scans
                    self.range_scan();
                }
                stats.reads += 1;
            } else {
                // Write operation
                let write_roll: f64 = self.rng.gen();
                if write_roll < 0.5 {
                    // 50% of writes are inserts
                    self.insert_order();
                } else if write_roll < 0.9 {
                    // 40% of writes are updates
                    self.update_inventory();
                } else {
                    // 10% of writes are deletes
                    self.delete_order();
                }
                stats.writes += 1;
            }

            stats.ops_completed += 1;
        }

        stats.total_duration = start.elapsed();
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ormdb_core::storage::StorageConfig;
    use tempfile::TempDir;

    fn create_test_storage() -> (Arc<StorageEngine>, TempDir) {
        let dir = TempDir::new().unwrap();
        let config = StorageConfig::new(dir.path());
        let storage = StorageEngine::open(config).unwrap();
        (Arc::new(storage), dir)
    }

    #[test]
    fn test_oltp_small_workload() {
        let (storage, _dir) = create_test_storage();
        let config = OltpConfig::small();
        let mut workload = OltpWorkload::new(storage, config.clone());

        let stats = workload.run_mixed_ops(config.ops_per_iteration);

        assert_eq!(stats.ops_completed, config.ops_per_iteration);
        assert!(stats.reads > 0);
        assert!(stats.writes > 0);
        println!(
            "Small workload: {} ops in {:?} ({:.0} ops/sec)",
            stats.ops_completed,
            stats.total_duration,
            stats.ops_per_sec()
        );
    }

    #[test]
    fn test_point_read_latency() {
        let (storage, _dir) = create_test_storage();
        let config = OltpConfig::small();
        let mut workload = OltpWorkload::new(storage, config);

        // Warm up
        for _ in 0..10 {
            workload.point_read();
        }

        // Measure
        let mut total = Duration::ZERO;
        let iterations = 100;
        for _ in 0..iterations {
            total += workload.point_read();
        }

        let avg = total / iterations as u32;
        println!("Average point read latency: {:?}", avg);
        assert!(avg < Duration::from_millis(10)); // Should be fast
    }
}
