use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(name = "ormdb-studio")]
#[command(about = "ORMDB Studio - Web-based database management")]
#[command(version)]
pub struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "3000")]
    pub port: u16,

    /// Address to bind to (localhost only for security)
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Directory for session databases (default: system temp)
    #[arg(long)]
    pub data_dir: Option<PathBuf>,

    /// Session timeout in minutes
    #[arg(long, default_value_t = 60)]
    pub session_timeout: u64,

    /// Maximum concurrent sessions
    #[arg(long, default_value_t = 10)]
    pub max_sessions: usize,

    /// Don't open browser automatically
    #[arg(long, default_value_t = false)]
    pub no_open: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,
}

#[derive(Debug, Clone)]
pub struct StudioConfig {
    pub host: String,
    pub port: u16,
    pub data_dir: Option<PathBuf>,
    pub session_timeout: Duration,
    pub max_sessions: usize,
    pub open_browser: bool,
}

impl From<Args> for StudioConfig {
    fn from(args: Args) -> Self {
        Self {
            host: args.host,
            port: args.port,
            data_dir: args.data_dir,
            session_timeout: Duration::from_secs(args.session_timeout * 60),
            max_sessions: args.max_sessions,
            open_browser: !args.no_open,
        }
    }
}

impl StudioConfig {
    pub fn listen_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}
