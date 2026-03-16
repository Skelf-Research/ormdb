# Technology Stack

This document specifies the implementation technology choices for ORMDB, mapping each core dependency to its role in the system architecture.

## Implementation Language

**Rust** (Edition 2021)

Rationale:
- Memory safety without garbage collection overhead
- Zero-cost abstractions for high-performance data structures
- Strong async ecosystem for concurrent workloads
- Excellent tooling for embedded database development

## Core Dependencies

| Crate | Version | Role |
|-------|---------|------|
| sled | 0.34 | Embedded storage engine |
| rkyv | 0.8 | Zero-copy serialization |
| mimalloc | 0.1 | Global memory allocator |
| async-nng | 0.2 | Async messaging transport |

## Storage Layer: sled

sled provides the foundation for ORMDB's storage engine.

**Mapping to architecture:**
- Row-oriented storage via sled's ordered key-value model
- Built-in crash recovery and durability guarantees
- Transaction support for ACID semantics
- Built-in WAL for write durability

**MVCC strategy:**
- Version keys encoded as `(entity_id, version_ts)` tuples
- Garbage collection of old versions via background compaction
- Read snapshots use consistent version timestamps

**Configuration considerations:**
- Page cache sizing for working set
- Flush policy tuning for durability vs throughput tradeoff
- Compression settings for storage efficiency

## Serialization Layer: rkyv

rkyv provides zero-copy deserialization for the protocol layer.

**Mapping to architecture:**
- Typed query IR serialized with rkyv for wire protocol
- Entity blocks and edge blocks as rkyv-serializable structures
- Zero-copy reads eliminate deserialization overhead for large result sets

**Schema design:**
- All protocol types derive `Archive`, `Serialize`, `Deserialize`
- Alignment requirements considered for cross-platform compatibility
- Versioned schemas for protocol evolution

**Benefits:**
- Sub-microsecond deserialization for cached query results
- Memory-mapped result buffers without parsing
- Reduced allocation pressure on hot paths

## Networking Layer: async-nng

async-nng provides the messaging transport for client-server communication.

**Mapping to architecture:**
- Request-reply pattern for graph fetch queries
- Pub-sub pattern for change streams
- Pipeline pattern for bulk ingestion

**Message types:**
- Query request: serialized query IR + parameters
- Query response: entity blocks + edge blocks + metadata
- Change event: entity/relation deltas with version tokens

**Transport patterns:**
- TCP for remote clients
- IPC for local high-performance connections
- Connection pooling and multiplexing

## Memory Management: mimalloc

mimalloc replaces the default allocator for performance-critical workloads.

**Rationale:**
- Improved performance for allocation-heavy workloads
- Better scalability under concurrent access
- Reduced memory fragmentation

**Setup:**
```rust
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

**Tuning:**
- Arena configuration for large allocations
- Thread-local heap optimization
- Memory statistics for observability

## Secondary Dependencies

| Crate | Purpose |
|-------|---------|
| tokio | Async runtime for I/O and scheduling |
| thiserror | Error type definitions |
| tracing | Structured logging and diagnostics |
| bytes | Efficient byte buffer management |

## Build Requirements

- Rust toolchain (stable, edition 2021)
- C compiler for mimalloc native build
- nng library (system or bundled)

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `mimalloc` | on | Use mimalloc allocator |
| `compression` | on | Enable storage compression |
| `tls` | off | TLS support for nng transport |
| `analytics` | off | Enable relational query mode |

## Dependency Rationale Summary

| Choice | Alternatives Considered | Rationale |
|--------|------------------------|-----------|
| sled | rocksdb, sqlite | Pure Rust, embeddable, good async story |
| rkyv | bincode, postcard, serde | Zero-copy critical for large result sets |
| mimalloc | jemalloc, tcmalloc | Best general performance, easy integration |
| async-nng | tokio channels, tonic/grpc | Lightweight, flexible patterns, IPC support |
