# Real-time Dashboard Example

An analytics dashboard demonstrating Change Data Capture, aggregations, and performance optimization.

---

## Overview

This example builds a real-time analytics dashboard with:
- Live data updates via CDC (Change Data Capture)
- Efficient aggregation queries
- Time-series data handling
- Columnar storage optimization
- WebSocket streaming

---

## Schema

```ormdb
// schema.ormdb

// Events table - high volume append-only
entity Event {
    id: uuid @id @default(uuid())
    org_id: uuid
    event_type: string
    event_name: string
    properties: json?
    user_id: string?
    session_id: string?
    timestamp: timestamp @default(now())

    // Denormalized for fast queries
    date: string  // YYYY-MM-DD
    hour: int32   // 0-23

    @index(org_id)
    @index(event_type)
    @index(timestamp)
    @index([org_id, date])
    @index([org_id, event_type, date])
}

// Pre-aggregated hourly stats
entity HourlyStats {
    id: uuid @id @default(uuid())
    org_id: uuid
    date: string
    hour: int32
    event_type: string
    event_count: int64
    unique_users: int64
    unique_sessions: int64

    @unique([org_id, date, hour, event_type])
    @index(org_id)
    @index([org_id, date])
}

// Pre-aggregated daily stats
entity DailyStats {
    id: uuid @id @default(uuid())
    org_id: uuid
    date: string
    event_type: string
    event_count: int64
    unique_users: int64
    unique_sessions: int64

    @unique([org_id, date, event_type])
    @index(org_id)
    @index([org_id, date])
}

// Real-time counters (updated every few seconds)
entity RealtimeCounter {
    id: uuid @id @default(uuid())
    org_id: uuid
    counter_name: string
    value: int64
    window_start: timestamp
    window_end: timestamp
    updated_at: timestamp

    @unique([org_id, counter_name])
    @index(org_id)
}

// Dashboard configuration
entity Dashboard {
    id: uuid @id @default(uuid())
    org_id: uuid
    name: string
    widgets: json
    created_at: timestamp @default(now())
    updated_at: timestamp @default(now())

    @index(org_id)
}
```

---

## Event Ingestion

### High-Throughput Event Writer

```rust
// src/ingestion.rs
use std::sync::Arc;
use tokio::sync::mpsc;
use ormdb_core::Database;
use chrono::{Utc, Datelike, Timelike};

pub struct EventIngester {
    db: Arc<Database>,
    buffer: mpsc::Sender<Event>,
}

impl EventIngester {
    pub fn new(db: Arc<Database>) -> Self {
        let (tx, rx) = mpsc::channel(10_000);

        // Spawn background batch writer
        let db_clone = db.clone();
        tokio::spawn(async move {
            batch_writer(db_clone, rx).await;
        });

        Self { db, buffer: tx }
    }

    pub async fn ingest(&self, event: IngestEvent) -> Result<(), Error> {
        let now = Utc::now();

        let event = Event {
            id: Uuid::new_v4(),
            org_id: event.org_id,
            event_type: event.event_type,
            event_name: event.event_name,
            properties: event.properties,
            user_id: event.user_id,
            session_id: event.session_id,
            timestamp: now.timestamp_micros(),
            date: now.format("%Y-%m-%d").to_string(),
            hour: now.hour() as i32,
        };

        self.buffer.send(event).await?;
        Ok(())
    }
}

async fn batch_writer(db: Arc<Database>, mut rx: mpsc::Receiver<Event>) {
    let mut batch = Vec::with_capacity(1000);
    let mut interval = tokio::time::interval(Duration::from_millis(100));

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                batch.push(event);

                // Flush when batch is full
                if batch.len() >= 1000 {
                    flush_batch(&db, &mut batch).await;
                }
            }
            _ = interval.tick() => {
                // Flush periodically even if batch not full
                if !batch.is_empty() {
                    flush_batch(&db, &mut batch).await;
                }
            }
        }
    }
}

async fn flush_batch(db: &Database, batch: &mut Vec<Event>) {
    let mutations: Vec<_> = batch.drain(..)
        .map(|e| {
            Mutation::create("Event")
                .set("id", Value::Uuid(e.id.into_bytes()))
                .set("org_id", Value::Uuid(e.org_id.into_bytes()))
                .set("event_type", Value::String(e.event_type))
                .set("event_name", Value::String(e.event_name))
                .set_opt("properties", e.properties.map(|p| Value::Json(p.to_string())))
                .set_opt("user_id", e.user_id.map(Value::String))
                .set_opt("session_id", e.session_id.map(Value::String))
                .set("timestamp", Value::Timestamp(e.timestamp))
                .set("date", Value::String(e.date))
                .set("hour", Value::Int32(e.hour))
        })
        .collect();

    if let Err(e) = db.mutate_batch(mutations).await {
        eprintln!("Failed to write event batch: {}", e);
    }
}
```

---

## Change Data Capture

### CDC Subscriber for Aggregation

```rust
// src/cdc.rs
use ormdb_core::{Database, CdcSubscriber, ChangeEvent};

pub async fn start_aggregation_worker(db: Arc<Database>) {
    let subscriber = db.subscribe_changes("Event").await.unwrap();

    tokio::spawn(async move {
        process_changes(db, subscriber).await;
    });
}

async fn process_changes(db: Arc<Database>, mut subscriber: CdcSubscriber) {
    let mut hourly_buffer: HashMap<AggKey, AggValue> = HashMap::new();
    let mut flush_interval = tokio::time::interval(Duration::from_secs(5));

    loop {
        tokio::select! {
            Some(change) = subscriber.next() => {
                if let ChangeEvent::Insert { entity, .. } = change {
                    let key = AggKey {
                        org_id: entity.get("org_id").unwrap(),
                        date: entity.get("date").unwrap(),
                        hour: entity.get("hour").unwrap(),
                        event_type: entity.get("event_type").unwrap(),
                    };

                    let entry = hourly_buffer.entry(key).or_default();
                    entry.event_count += 1;

                    if let Some(user_id) = entity.get::<String>("user_id") {
                        entry.unique_users.insert(user_id);
                    }
                    if let Some(session_id) = entity.get::<String>("session_id") {
                        entry.unique_sessions.insert(session_id);
                    }
                }
            }
            _ = flush_interval.tick() => {
                flush_aggregations(&db, &mut hourly_buffer).await;
            }
        }
    }
}

async fn flush_aggregations(
    db: &Database,
    buffer: &mut HashMap<AggKey, AggValue>,
) {
    for (key, value) in buffer.drain() {
        // Upsert hourly stats
        let existing = db.query(
            GraphQuery::new("HourlyStats")
                .filter(FilterExpr::and(vec![
                    FilterExpr::eq("org_id", Value::Uuid(key.org_id.into_bytes())),
                    FilterExpr::eq("date", Value::String(key.date.clone())),
                    FilterExpr::eq("hour", Value::Int32(key.hour)),
                    FilterExpr::eq("event_type", Value::String(key.event_type.clone())),
                ]))
        ).await.ok().and_then(|r| r.entities().next());

        if let Some(existing) = existing {
            let id: Uuid = existing.get("id").unwrap();
            let current_count: i64 = existing.get("event_count").unwrap();
            let current_users: i64 = existing.get("unique_users").unwrap();
            let current_sessions: i64 = existing.get("unique_sessions").unwrap();

            db.mutate(
                Mutation::update("HourlyStats")
                    .filter(FilterExpr::eq("id", Value::Uuid(id.into_bytes())))
                    .set("event_count", Value::Int64(current_count + value.event_count))
                    .set("unique_users", Value::Int64(current_users + value.unique_users.len() as i64))
                    .set("unique_sessions", Value::Int64(current_sessions + value.unique_sessions.len() as i64))
            ).await.ok();
        } else {
            db.mutate(
                Mutation::create("HourlyStats")
                    .set("id", Value::Uuid(Uuid::new_v4().into_bytes()))
                    .set("org_id", Value::Uuid(key.org_id.into_bytes()))
                    .set("date", Value::String(key.date))
                    .set("hour", Value::Int32(key.hour))
                    .set("event_type", Value::String(key.event_type))
                    .set("event_count", Value::Int64(value.event_count))
                    .set("unique_users", Value::Int64(value.unique_users.len() as i64))
                    .set("unique_sessions", Value::Int64(value.unique_sessions.len() as i64))
            ).await.ok();
        }
    }
}

#[derive(Hash, Eq, PartialEq)]
struct AggKey {
    org_id: Uuid,
    date: String,
    hour: i32,
    event_type: String,
}

#[derive(Default)]
struct AggValue {
    event_count: i64,
    unique_users: HashSet<String>,
    unique_sessions: HashSet<String>,
}
```

---

## Aggregation Queries

### Time-Series Data

```rust
// src/queries/analytics.rs

/// Get event counts over time
pub async fn get_event_timeseries(
    db: &Database,
    org_id: Uuid,
    event_type: Option<&str>,
    start_date: &str,
    end_date: &str,
    granularity: Granularity,
) -> Vec<TimeseriesPoint> {
    match granularity {
        Granularity::Hour => {
            let query = GraphQuery::new("HourlyStats")
                .filter(FilterExpr::and(vec![
                    FilterExpr::eq("org_id", Value::Uuid(org_id.into_bytes())),
                    FilterExpr::gte("date", Value::String(start_date.into())),
                    FilterExpr::lte("date", Value::String(end_date.into())),
                ]))
                .order_by(OrderSpec::asc("date"))
                .order_by(OrderSpec::asc("hour"));

            let query = if let Some(et) = event_type {
                query.filter(FilterExpr::eq("event_type", Value::String(et.into())))
            } else {
                query
            };

            let result = db.query(query).await.unwrap();

            result.entities()
                .map(|e| TimeseriesPoint {
                    timestamp: format!("{} {:02}:00", e.get::<String>("date").unwrap(), e.get::<i32>("hour").unwrap()),
                    value: e.get("event_count").unwrap(),
                })
                .collect()
        }
        Granularity::Day => {
            let query = GraphQuery::new("DailyStats")
                .filter(FilterExpr::and(vec![
                    FilterExpr::eq("org_id", Value::Uuid(org_id.into_bytes())),
                    FilterExpr::gte("date", Value::String(start_date.into())),
                    FilterExpr::lte("date", Value::String(end_date.into())),
                ]))
                .order_by(OrderSpec::asc("date"));

            // ... similar processing
        }
    }
}

/// Get top events by count
pub async fn get_top_events(
    db: &Database,
    org_id: Uuid,
    date: &str,
    limit: usize,
) -> Vec<TopEvent> {
    // Use columnar aggregation for efficiency
    let result = db.aggregate(
        AggregateQuery::new("Event")
            .filter(FilterExpr::and(vec![
                FilterExpr::eq("org_id", Value::Uuid(org_id.into_bytes())),
                FilterExpr::eq("date", Value::String(date.into())),
            ]))
            .group_by("event_name")
            .aggregate("count", AggregateFunction::Count)
            .order_by(OrderSpec::desc("count"))
            .limit(limit)
    ).await.unwrap();

    result.rows()
        .map(|r| TopEvent {
            event_name: r.get("event_name").unwrap(),
            count: r.get("count").unwrap(),
        })
        .collect()
}

/// Get unique user count
pub async fn get_unique_users(
    db: &Database,
    org_id: Uuid,
    start_date: &str,
    end_date: &str,
) -> i64 {
    let result = db.aggregate(
        AggregateQuery::new("Event")
            .filter(FilterExpr::and(vec![
                FilterExpr::eq("org_id", Value::Uuid(org_id.into_bytes())),
                FilterExpr::gte("date", Value::String(start_date.into())),
                FilterExpr::lte("date", Value::String(end_date.into())),
                FilterExpr::is_not_null("user_id"),
            ]))
            .aggregate("unique_users", AggregateFunction::CountDistinct("user_id"))
    ).await.unwrap();

    result.get("unique_users").unwrap_or(0)
}
```

---

## Real-time Counters

### Counter Update Worker

```rust
// src/realtime.rs

pub async fn start_realtime_counter_worker(db: Arc<Database>) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));

    loop {
        interval.tick().await;
        update_realtime_counters(&db).await;
    }
}

async fn update_realtime_counters(db: &Database) {
    let now = Utc::now();
    let window_start = now - Duration::from_secs(300);  // Last 5 minutes

    // Get all organizations
    let orgs = db.query(GraphQuery::new("Organization")).await.unwrap();

    for org in orgs.entities() {
        let org_id: Uuid = org.get("id").unwrap();

        // Count events in window
        let event_count = db.aggregate(
            AggregateQuery::new("Event")
                .filter(FilterExpr::and(vec![
                    FilterExpr::eq("org_id", Value::Uuid(org_id.into_bytes())),
                    FilterExpr::gte("timestamp", Value::Timestamp(window_start.timestamp_micros())),
                ]))
                .aggregate("count", AggregateFunction::Count)
        ).await.unwrap().get::<i64>("count").unwrap_or(0);

        // Count active users
        let active_users = db.aggregate(
            AggregateQuery::new("Event")
                .filter(FilterExpr::and(vec![
                    FilterExpr::eq("org_id", Value::Uuid(org_id.into_bytes())),
                    FilterExpr::gte("timestamp", Value::Timestamp(window_start.timestamp_micros())),
                    FilterExpr::is_not_null("user_id"),
                ]))
                .aggregate("count", AggregateFunction::CountDistinct("user_id"))
        ).await.unwrap().get::<i64>("count").unwrap_or(0);

        // Upsert counters
        upsert_counter(db, org_id, "events_5m", event_count, window_start, now).await;
        upsert_counter(db, org_id, "active_users_5m", active_users, window_start, now).await;
    }
}

async fn upsert_counter(
    db: &Database,
    org_id: Uuid,
    name: &str,
    value: i64,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) {
    let existing = db.query(
        GraphQuery::new("RealtimeCounter")
            .filter(FilterExpr::and(vec![
                FilterExpr::eq("org_id", Value::Uuid(org_id.into_bytes())),
                FilterExpr::eq("counter_name", Value::String(name.into())),
            ]))
    ).await.ok().and_then(|r| r.entities().next());

    if let Some(existing) = existing {
        let id: Uuid = existing.get("id").unwrap();
        db.mutate(
            Mutation::update("RealtimeCounter")
                .filter(FilterExpr::eq("id", Value::Uuid(id.into_bytes())))
                .set("value", Value::Int64(value))
                .set("window_start", Value::Timestamp(window_start.timestamp_micros()))
                .set("window_end", Value::Timestamp(window_end.timestamp_micros()))
                .set("updated_at", Value::Timestamp(window_end.timestamp_micros()))
        ).await.ok();
    } else {
        db.mutate(
            Mutation::create("RealtimeCounter")
                .set("id", Value::Uuid(Uuid::new_v4().into_bytes()))
                .set("org_id", Value::Uuid(org_id.into_bytes()))
                .set("counter_name", Value::String(name.into()))
                .set("value", Value::Int64(value))
                .set("window_start", Value::Timestamp(window_start.timestamp_micros()))
                .set("window_end", Value::Timestamp(window_end.timestamp_micros()))
                .set("updated_at", Value::Timestamp(window_end.timestamp_micros()))
        ).await.ok();
    }
}
```

---

## WebSocket Streaming

### Real-time Updates

```rust
// src/websocket.rs
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use futures::{SinkExt, StreamExt};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(db): State<Db>,
    auth: AuthContext,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, db, auth))
}

async fn handle_socket(socket: WebSocket, db: Db, auth: AuthContext) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to CDC for this org's events
    let org_id = auth.current_org_id.expect("Org required");
    let mut cdc = db.subscribe_changes("Event").await.unwrap();

    // Task to send updates
    let send_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut event_buffer = Vec::new();

        loop {
            tokio::select! {
                Some(change) = cdc.next() => {
                    if let ChangeEvent::Insert { entity, .. } = change {
                        // Filter to this org
                        if entity.get::<Uuid>("org_id") == Some(org_id) {
                            event_buffer.push(entity);
                        }
                    }
                }
                _ = interval.tick() => {
                    if !event_buffer.is_empty() {
                        let update = DashboardUpdate {
                            events: event_buffer.drain(..).collect(),
                            counters: get_realtime_counters(&db, org_id).await,
                        };

                        let msg = serde_json::to_string(&update).unwrap();
                        if sender.send(Message::Text(msg)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    // Task to receive commands
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                // Handle client commands (e.g., change time range)
                if let Ok(cmd) = serde_json::from_str::<ClientCommand>(&text) {
                    // Process command
                }
            }
        }
    });

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }
}

async fn get_realtime_counters(db: &Database, org_id: Uuid) -> HashMap<String, i64> {
    let result = db.query(
        GraphQuery::new("RealtimeCounter")
            .filter(FilterExpr::eq("org_id", Value::Uuid(org_id.into_bytes())))
    ).await.unwrap();

    result.entities()
        .map(|e| (e.get::<String>("counter_name").unwrap(), e.get::<i64>("value").unwrap()))
        .collect()
}
```

---

## Frontend Dashboard

### React Dashboard Component

```tsx
// frontend/src/components/Dashboard.tsx
import { useEffect, useState } from 'react';
import { useWebSocket } from '../hooks/useWebSocket';
import { LineChart, BarChart, StatCard } from './charts';

export function Dashboard({ orgId }: { orgId: string }) {
  const [timeRange, setTimeRange] = useState('24h');
  const [data, setData] = useState<DashboardData | null>(null);

  // WebSocket for real-time updates
  const { lastMessage, sendMessage } = useWebSocket(`/ws/dashboard`);

  useEffect(() => {
    if (lastMessage) {
      const update = JSON.parse(lastMessage.data);
      setData(prev => ({
        ...prev,
        counters: update.counters,
        recentEvents: [...update.events, ...(prev?.recentEvents || [])].slice(0, 100),
      }));
    }
  }, [lastMessage]);

  // Initial data load
  useEffect(() => {
    loadDashboardData(orgId, timeRange).then(setData);
  }, [orgId, timeRange]);

  if (!data) return <Loading />;

  return (
    <div className="dashboard">
      <TimeRangeSelector value={timeRange} onChange={setTimeRange} />

      <div className="stats-row">
        <StatCard
          title="Events (5m)"
          value={data.counters.events_5m}
          trend={data.eventsTrend}
        />
        <StatCard
          title="Active Users"
          value={data.counters.active_users_5m}
          trend={data.usersTrend}
        />
        <StatCard
          title="Total Events Today"
          value={data.totalEventsToday}
        />
        <StatCard
          title="Unique Users Today"
          value={data.uniqueUsersToday}
        />
      </div>

      <div className="charts-row">
        <LineChart
          title="Events Over Time"
          data={data.eventTimeseries}
          xKey="timestamp"
          yKey="value"
        />
        <BarChart
          title="Top Events"
          data={data.topEvents}
          xKey="event_name"
          yKey="count"
        />
      </div>

      <div className="recent-events">
        <h3>Recent Events</h3>
        <EventList events={data.recentEvents} />
      </div>
    </div>
  );
}
```

### Custom Hook for WebSocket

```tsx
// frontend/src/hooks/useWebSocket.ts
import { useEffect, useRef, useState, useCallback } from 'react';

export function useWebSocket(url: string) {
  const ws = useRef<WebSocket | null>(null);
  const [lastMessage, setLastMessage] = useState<MessageEvent | null>(null);
  const [isConnected, setIsConnected] = useState(false);

  useEffect(() => {
    const connect = () => {
      ws.current = new WebSocket(`${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}${url}`);

      ws.current.onopen = () => setIsConnected(true);
      ws.current.onclose = () => {
        setIsConnected(false);
        // Reconnect after delay
        setTimeout(connect, 3000);
      };
      ws.current.onmessage = setLastMessage;
    };

    connect();

    return () => {
      ws.current?.close();
    };
  }, [url]);

  const sendMessage = useCallback((data: any) => {
    if (ws.current?.readyState === WebSocket.OPEN) {
      ws.current.send(JSON.stringify(data));
    }
  }, []);

  return { lastMessage, sendMessage, isConnected };
}
```

---

## Performance Optimization

### Columnar Storage for Analytics

ORMDB automatically uses columnar storage for aggregation queries:

```rust
// This query uses columnar storage internally
let result = db.aggregate(
    AggregateQuery::new("Event")
        .filter(FilterExpr::eq("org_id", Value::Uuid(org_id.into_bytes())))
        .aggregate("total", AggregateFunction::Count)
        .aggregate("unique_users", AggregateFunction::CountDistinct("user_id"))
).await?;

// Columnar operations:
// 1. Scan only org_id and user_id columns (not full rows)
// 2. Use dictionary encoding for string columns
// 3. SIMD-optimized aggregation functions
```

### Index Strategy

```ormdb
// Composite indexes for common query patterns
@index([org_id, date])           // Daily queries
@index([org_id, event_type, date])  // Event type breakdown
@index([org_id, timestamp])      // Real-time range queries
```

### Pre-aggregation Strategy

| Query Type | Data Source | Latency |
|------------|-------------|---------|
| Last 5 min | RealtimeCounter | ~5ms |
| Today | HourlyStats | ~20ms |
| Last 7 days | DailyStats | ~30ms |
| Last 30 days | DailyStats | ~50ms |
| Custom range | Raw Events | ~200ms+ |

---

## Key Takeaways

1. **CDC enables real-time** - Subscribe to changes for live updates
2. **Pre-aggregate for speed** - Compute hourly/daily stats in background
3. **Columnar for analytics** - ORMDB optimizes aggregation queries
4. **Batch high-volume writes** - Buffer events before writing
5. **WebSocket for live UI** - Stream updates to dashboards

---

## Next Steps

- Add custom dashboards with [Tutorials](../tutorials/index.md)
- Implement alerting based on thresholds
- Export data with [Operations Guide](../operations/index.md)

