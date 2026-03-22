# Installation

This guide covers installing the ORMDB server and client libraries.

## Server Installation

### Using Docker (Recommended)

The easiest way to run ORMDB is with Docker:

```bash
# Pull and run ORMDB
docker run -d \
  --name ormdb \
  -p 9000:9000 \
  -p 8080:8080 \
  -v ormdb-data:/data \
  ormdb/ormdb:latest

# Check it's running
docker logs ormdb
```

Port **9000** is the native protocol port (used by Rust client).
Port **8080** is the HTTP gateway (used by TypeScript/Python clients).

### Using Cargo

If you have Rust installed, you can build from source:

```bash
# Install from crates.io
cargo install ormdb-server

# Or build from source
git clone https://github.com/Skelf-Research/ormdb.git
cd ormdb
cargo build --release

# Run the server
./target/release/ormdb-server --data-dir ./data
```

### Configuration

Create a config file at `~/.ormdb/config.toml`:

```toml
[server]
data_dir = "/var/lib/ormdb"
native_port = 9000
http_port = 8080

[storage]
cache_size = "1GB"
flush_interval = "1s"
compression = true

[security]
enable_auth = false  # Enable for production
```

See [Configuration Reference](../reference/configuration.md) for all options.

---

## Client Installation

### Rust

Add to your `Cargo.toml`:

```toml
[dependencies]
ormdb-client = "0.1"
ormdb-proto = "0.1"
tokio = { version = "1", features = ["full"] }
```

### TypeScript / JavaScript

Using npm:

```bash
npm install @ormdb/client
```

Using yarn:

```bash
yarn add @ormdb/client
```

Using pnpm:

```bash
pnpm add @ormdb/client
```

### Python

Using pip:

```bash
pip install ormdb
```

With SQLAlchemy support:

```bash
pip install ormdb[sqlalchemy]
```

With Django support:

```bash
pip install ormdb[django]
```

---

## Verify Installation

### Check Server

```bash
# Using curl
curl http://localhost:8080/health

# Expected response:
# {"status": "healthy", "version": "0.1.0"}
```

### Check Client Connection

=== "Rust"

    ```rust
    use ormdb_client::{Client, ClientConfig};

    #[tokio::main]
    async fn main() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::connect(ClientConfig::localhost()).await?;
        client.ping().await?;
        println!("Connected to ORMDB!");
        Ok(())
    }
    ```

=== "TypeScript"

    ```typescript
    import { OrmdbClient } from "@ormdb/client";

    const client = new OrmdbClient("http://localhost:8080");
    const health = await client.health();
    console.log("Connected to ORMDB!", health);
    ```

=== "Python"

    ```python
    from ormdb import OrmdbClient

    client = OrmdbClient("http://localhost:8080")
    health = client.health()
    print(f"Connected to ORMDB! {health}")
    ```

---

## Next Steps

Now that you have ORMDB installed, proceed to the [Quickstart](quickstart.md) to build your first app.
