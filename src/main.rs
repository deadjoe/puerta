use clap::{Parser, Subcommand};
use log::info;
use puerta::config::{Config, ConfigError};
use puerta::{ProxyMode, Puerta, PuertaConfig};
use std::path::PathBuf;

// Pingora framework imports
use pingora_core::server::configuration::Opt;

#[derive(Parser)]
#[command(name = "puerta")]
#[command(
    about = "A high-performance load balancer for MongoDB Sharded Clusters and Redis Clusters"
)]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(author = "Puerta Team")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the puerta load balancer
    Run {
        /// Path to configuration file
        #[arg(short, long, default_value = "config/dev.toml")]
        config: PathBuf,
    },
    /// Generate example configuration files
    Config {
        /// Configuration mode (mongodb or redis)
        #[arg(short, long)]
        mode: String,
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Validate configuration file
    Validate {
        /// Path to configuration file to validate
        #[arg(short, long)]
        config: PathBuf,
    },
    /// Show version information
    Version,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { config } => {
            run_puerta(config).await?;
        }
        Commands::Config { mode, output } => {
            generate_config(mode, output)?;
        }
        Commands::Validate { config } => {
            validate_config(config)?;
        }
        Commands::Version => {
            show_version();
        }
    }

    Ok(())
}

async fn run_puerta(config_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Config::load_from_file(&config_path)
        .map_err(|e| format!("Failed to load config from {:?}: {}", config_path, e))?;

    // Initialize logging
    init_logging(&config)?;

    info!(
        "Starting puerta v{} with Pingora framework",
        env!("CARGO_PKG_VERSION")
    );
    info!("Configuration loaded from: {:?}", config_path);
    info!("Proxy mode: {:?}", config.proxy);
    info!("Listening on: {}", config.server.listen_addr);

    // Create puerta configuration
    let puerta_config = PuertaConfig {
        listen_addr: config.server.listen_addr.clone(),
        proxy_mode: match config.proxy {
            puerta::config::ProxyConfig::MongoDB {
                mongos_endpoints,
                session_affinity,
                ..
            } => ProxyMode::MongoDB {
                mongos_endpoints,
                session_affinity_enabled: session_affinity,
            },
            puerta::config::ProxyConfig::Redis {
                cluster_nodes,
                slot_refresh_interval_sec,
                ..
            } => ProxyMode::Redis {
                cluster_nodes,
                slot_refresh_interval_ms: slot_refresh_interval_sec * 1000,
            },
        },
        health_check_interval_ms: config.health.interval_sec * 1000,
        max_connections: config.server.max_connections,
    };

    // Create and initialize Puerta with Pingora
    let mut puerta = Puerta::new(puerta_config);

    // Initialize Pingora server with default options
    let pingora_opt = Opt::default();
    puerta
        .initialize(Some(pingora_opt))
        .map_err(|e| format!("Failed to initialize Puerta: {}", e))?;

    info!("Puerta initialized with Pingora framework, starting server...");
    if let Err(e) = puerta.run().await {
        return Err(format!("Failed to run puerta: {}", e).into());
    }

    Ok(())
}

fn generate_config(mode: String, output: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    println!("Generating {} configuration file: {:?}", mode, output);

    Config::create_example_config(&output, &mode)
        .map_err(|e| format!("Failed to generate config: {}", e))?;

    println!("Configuration file generated successfully!");
    println!("Edit the file to match your environment and run:");
    println!("  puerta run --config {:?}", output);

    Ok(())
}

fn validate_config(config_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    println!("Validating configuration file: {:?}", config_path);

    match Config::load_from_file(&config_path) {
        Ok(config) => {
            println!("✓ Configuration file is valid");
            println!("  Proxy mode: {:?}", config.proxy);
            println!("  Listen address: {}", config.server.listen_addr);
            println!("  Max connections: {}", config.server.max_connections);

            match config.proxy {
                puerta::config::ProxyConfig::MongoDB {
                    mongos_endpoints, ..
                } => {
                    println!(
                        "  MongoDB mongos endpoints: {} instances",
                        mongos_endpoints.len()
                    );
                    for (i, endpoint) in mongos_endpoints.iter().enumerate() {
                        println!("    {}: {}", i + 1, endpoint);
                    }
                }
                puerta::config::ProxyConfig::Redis { cluster_nodes, .. } => {
                    println!("  Redis cluster nodes: {} instances", cluster_nodes.len());
                    for (i, node) in cluster_nodes.iter().enumerate() {
                        println!("    {}: {}", i + 1, node);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Configuration file validation failed:");
            match &e {
                ConfigError::IoError(msg) => eprintln!("  File error: {}", msg),
                ConfigError::ParseError(msg) => eprintln!("  Parse error: {}", msg),
                ConfigError::ValidationError(msg) => eprintln!("  Validation error: {}", msg),
                ConfigError::SerializeError(msg) => eprintln!("  Serialization error: {}", msg),
            }
            return Err(Box::new(e));
        }
    }

    Ok(())
}

fn show_version() {
    println!("puerta v{}", env!("CARGO_PKG_VERSION"));
    println!("A high-performance load balancer for MongoDB Sharded Clusters and Redis Clusters");
    println!();
    println!(
        "Built with Rust {}",
        option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown")
    );
    println!("Target: {}", std::env::consts::ARCH);
    println!();
    println!("Features:");
    println!("  • MongoDB Sharded Cluster load balancing with session affinity");
    println!("  • Redis Cluster protocol-aware proxy with MOVED/ASK handling");
    println!("  • High-performance async I/O with Tokio");
    println!("  • Comprehensive health checking");
    println!("  • Zero-copy data forwarding");
}

fn init_logging(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let log_level = match config.logging.level.as_str() {
        "debug" => log::LevelFilter::Debug,
        "info" => log::LevelFilter::Info,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    };

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .init();

    info!("Logging initialized at level: {:?}", log_level);
    Ok(())
}
