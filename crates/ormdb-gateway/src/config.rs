//! Gateway configuration.

use clap::Parser;

/// ORMDB HTTP/JSON Gateway command line arguments.
#[derive(Debug, Parser)]
#[command(name = "ormdb-gateway")]
#[command(about = "HTTP/JSON Gateway for ORMDB")]
pub struct Args {
    /// Address to listen on for HTTP requests.
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    pub listen: String,

    /// Address of the ORMDB server (NNG address).
    #[arg(short, long, default_value = "tcp://127.0.0.1:9000")]
    pub ormdb: String,
}

/// Gateway configuration.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Address to listen on for HTTP requests.
    pub listen_addr: String,
    /// Address of the ORMDB server.
    pub ormdb_addr: String,
}

impl From<&Args> for GatewayConfig {
    fn from(args: &Args) -> Self {
        Self {
            listen_addr: args.listen.clone(),
            ormdb_addr: args.ormdb.clone(),
        }
    }
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:8080".to_string(),
            ormdb_addr: "tcp://127.0.0.1:9000".to_string(),
        }
    }
}
