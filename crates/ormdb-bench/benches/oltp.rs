//! OLTP (Online Transaction Processing) benchmarks for embedded ORMDB.
//!
//! These benchmarks simulate realistic mixed read/write workloads typical of
//! e-commerce, social media, and other transactional applications.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ormdb_bench::oltp::{OltpConfig, OltpWorkload};
use ormdb_core::storage::{
    FullTextConfig, FullTextIndex, GeoIndex, GeoPoint, HnswConfig, RTreeConfig, StorageConfig,
    VectorIndex,
};
use ormdb_core::StorageEngine;
use rand::Rng;
use std::sync::Arc;
use std::time::Duration;

/// Create a test storage engine.
fn create_storage() -> (Arc<StorageEngine>, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().unwrap();
    let config = StorageConfig::new(dir.path());
    let storage = StorageEngine::open(config).unwrap();
    (Arc::new(storage), dir)
}

/// Benchmark mixed OLTP throughput at different scales.
fn bench_oltp_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("oltp/throughput");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    for (name, config) in [("small", OltpConfig::small()), ("medium", OltpConfig::medium())] {
        let (storage, _dir) = create_storage();
        let mut workload = OltpWorkload::new(storage, config.clone());

        group.throughput(Throughput::Elements(config.ops_per_iteration as u64));

        group.bench_function(BenchmarkId::new("mixed", name), |b| {
            b.iter(|| workload.run_mixed_ops(config.ops_per_iteration))
        });
    }

    group.finish();
}

/// Benchmark individual operation latencies.
fn bench_oltp_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("oltp/latency");
    group.sample_size(100);

    // Create a medium-sized database for latency tests
    let (storage, _dir) = create_storage();
    let config = OltpConfig::medium();
    let mut workload = OltpWorkload::new(storage, config);

    group.bench_function("point_read", |b| {
        b.iter(|| workload.point_read())
    });

    group.bench_function("range_scan", |b| {
        b.iter(|| workload.range_scan())
    });

    group.bench_function("insert", |b| {
        b.iter(|| workload.insert_order())
    });

    group.bench_function("update", |b| {
        b.iter(|| workload.update_inventory())
    });

    // Note: delete can deplete orders, so we re-populate periodically
    group.bench_function("delete", |b| {
        b.iter(|| workload.delete_order())
    });

    group.finish();
}

/// Benchmark different read/write mixes.
fn bench_oltp_workload_mix(c: &mut Criterion) {
    let mut group = c.benchmark_group("oltp/mix");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(5));

    // Different workload mixes
    let mixes = [
        ("read_heavy_95", 0.95),
        ("balanced_80", 0.80),
        ("write_heavy_50", 0.50),
    ];

    for (name, read_pct) in mixes {
        let (storage, _dir) = create_storage();
        let config = OltpConfig {
            num_customers: 500,
            num_products: 100,
            num_orders: 2000,
            read_pct,
            ops_per_iteration: 100,
        };
        let mut workload = OltpWorkload::new(storage, config.clone());

        group.throughput(Throughput::Elements(config.ops_per_iteration as u64));

        group.bench_function(BenchmarkId::new("ops", name), |b| {
            b.iter(|| workload.run_mixed_ops(config.ops_per_iteration))
        });
    }

    group.finish();
}

/// Benchmark with increasing data size.
fn bench_oltp_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("oltp/scaling");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(5));

    // Different data sizes
    let scales = [
        ("100_entities", 100, 50, 200),
        ("1k_entities", 1000, 200, 2000),
        ("10k_entities", 10000, 500, 10000),
    ];

    for (name, customers, products, orders) in scales {
        let (storage, _dir) = create_storage();
        let config = OltpConfig {
            num_customers: customers,
            num_products: products,
            num_orders: orders,
            read_pct: 0.80,
            ops_per_iteration: 100,
        };
        let mut workload = OltpWorkload::new(storage, config.clone());

        group.throughput(Throughput::Elements(config.ops_per_iteration as u64));

        group.bench_function(BenchmarkId::new("mixed", name), |b| {
            b.iter(|| workload.run_mixed_ops(config.ops_per_iteration))
        });
    }

    group.finish();
}

/// Benchmark concurrent access patterns.
fn bench_oltp_concurrent(c: &mut Criterion) {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

    let mut group = c.benchmark_group("oltp/concurrent");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    for num_threads in [1, 2, 4] {
        let (storage, _dir) = create_storage();

        // Populate initial data
        let config = OltpConfig::medium();
        let _ = OltpWorkload::new(storage.clone(), config.clone());

        let ops_per_thread = 25;
        let total_ops = ops_per_thread * num_threads;

        group.throughput(Throughput::Elements(total_ops as u64));

        group.bench_function(BenchmarkId::new("threads", num_threads), |b| {
            b.iter(|| {
                let done = Arc::new(AtomicBool::new(false));
                let mut handles = Vec::new();

                for _ in 0..num_threads {
                    let storage_clone = storage.clone();
                    let config_clone = config.clone();
                    let done_clone = done.clone();

                    let handle = thread::spawn(move || {
                        let mut workload = OltpWorkload::new(storage_clone, config_clone);
                        let stats = workload.run_mixed_ops(ops_per_thread);
                        done_clone.store(true, Ordering::SeqCst);
                        stats
                    });
                    handles.push(handle);
                }

                let mut total_ops = 0;
                for handle in handles {
                    let stats = handle.join().unwrap();
                    total_ops += stats.ops_completed;
                }
                total_ops
            })
        });
    }

    group.finish();
}

/// Generate a random vector for benchmarking.
fn random_vector(dimensions: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..dimensions).map(|_| rng.gen::<f32>()).collect()
}

/// Benchmark vector search operations using HNSW index.
fn bench_vector_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("oltp/vector");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(5));

    let dir = tempfile::TempDir::new().unwrap();
    let _config = StorageConfig::new(dir.path());
    let db = sled::open(dir.path()).unwrap();
    let vi = VectorIndex::open(&db, HnswConfig::default()).unwrap();

    // Populate with 10k vectors (384 dimensions like typical embeddings)
    let dimensions = 384;
    let num_vectors = 10_000;

    for i in 0..num_vectors {
        let mut entity_id = [0u8; 16];
        entity_id[0..8].copy_from_slice(&(i as u64).to_le_bytes());
        let vector = random_vector(dimensions);
        vi.insert("Product", "embedding", entity_id, &vector).unwrap();
    }

    // Benchmark k-NN search with different k values
    for k in [10, 50, 100] {
        let query_vector = random_vector(dimensions);

        group.bench_function(BenchmarkId::new("knn", k), |b| {
            b.iter(|| vi.search("Product", "embedding", &query_vector, k))
        });
    }

    group.finish();
}

/// Benchmark geo search operations using R-tree index.
fn bench_geo_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("oltp/geo");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(5));

    let dir = tempfile::TempDir::new().unwrap();
    let db = sled::open(dir.path()).unwrap();
    let gi = GeoIndex::open(&db, RTreeConfig::default()).unwrap();

    // Populate with 10k geo points spread around a region
    let num_points = 10_000;
    let mut rng = rand::thread_rng();

    // San Francisco Bay Area region
    let base_lat = 37.5;
    let base_lon = -122.5;
    let range = 1.0; // ~100km

    for i in 0..num_points {
        let mut entity_id = [0u8; 16];
        entity_id[0..8].copy_from_slice(&(i as u64).to_le_bytes());
        let point = GeoPoint {
            lat: base_lat + rng.gen::<f64>() * range,
            lon: base_lon + rng.gen::<f64>() * range,
        };
        gi.insert("Restaurant", "location", entity_id, point).unwrap();
    }

    let center = GeoPoint { lat: 37.7749, lon: -122.4194 };

    // Benchmark radius search with different radii
    for radius_km in [1.0, 5.0, 10.0] {
        group.bench_function(BenchmarkId::new("radius", format!("{}km", radius_km)), |b| {
            b.iter(|| gi.within_radius("Restaurant", "location", center, radius_km))
        });
    }

    // Benchmark bounding box search
    group.bench_function("bounding_box", |b| {
        let bbox = ormdb_core::storage::MBR {
            min_lat: 37.7, min_lon: -122.5,
            max_lat: 37.85, max_lon: -122.35,
        };
        b.iter(|| gi.within_box("Restaurant", "location", bbox))
    });

    // Benchmark nearest neighbor search
    for k in [10, 50] {
        group.bench_function(BenchmarkId::new("nearest", k), |b| {
            b.iter(|| gi.nearest("Restaurant", "location", center, k))
        });
    }

    group.finish();
}

/// Benchmark full-text search operations using inverted index.
fn bench_fulltext_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("oltp/fulltext");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(5));

    let dir = tempfile::TempDir::new().unwrap();
    let db = sled::open(dir.path()).unwrap();
    let fti = FullTextIndex::open(&db, FullTextConfig::default()).unwrap();

    // Sample documents for indexing
    let documents = [
        "Rust is a systems programming language that runs blazingly fast",
        "The quick brown fox jumps over the lazy dog in the garden",
        "Machine learning algorithms process large datasets efficiently",
        "Database indexing improves query performance significantly",
        "Full-text search enables finding documents by content",
        "Vector embeddings capture semantic meaning of text",
        "Geographic information systems map spatial data",
        "Real-time analytics require low latency processing",
        "Distributed systems scale across multiple nodes",
        "Memory safety prevents common programming errors",
    ];

    // Populate with 10k documents (cycling through samples)
    let num_docs = 10_000;
    for i in 0..num_docs {
        let mut entity_id = [0u8; 16];
        entity_id[0..8].copy_from_slice(&(i as u64).to_le_bytes());
        let doc = documents[i % documents.len()];
        fti.insert("Article", "content", entity_id, doc).unwrap();
    }

    // Benchmark single-term search
    group.bench_function("single_term", |b| {
        b.iter(|| fti.search("Article", "content", "rust", 10))
    });

    // Benchmark multi-term search
    group.bench_function("multi_term", |b| {
        b.iter(|| fti.search("Article", "content", "database indexing query", 10))
    });

    // Benchmark phrase search
    group.bench_function("phrase", |b| {
        b.iter(|| fti.search_phrase("Article", "content", "quick brown fox", 10))
    });

    // Benchmark boolean search
    group.bench_function("boolean", |b| {
        let must = vec!["search".to_string()];
        let should = vec!["database".to_string(), "text".to_string()];
        let must_not = vec![];
        b.iter(|| fti.search_boolean("Article", "content", &must, &should, &must_not, 10))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_oltp_throughput,
    bench_oltp_latency,
    bench_oltp_workload_mix,
    bench_oltp_scaling,
    bench_oltp_concurrent,
    bench_vector_search,
    bench_geo_search,
    bench_fulltext_search
);

criterion_main!(benches);
