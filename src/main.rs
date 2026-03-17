use clap::Parser;
use synoplayer::api::auth::AuthApi;
use synoplayer::api::client::SynoClient;
use synoplayer::cache::manager::CacheManager;
use synoplayer::cli;
use synoplayer::config::model::AppConfig;
use synoplayer::credentials::store::CredentialStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = cli::Cli::parse();
    let config = AppConfig::load()?;

    match cli.command {
        // --- Config ---
        cli::Commands::Config { action } => match action {
            cli::ConfigAction::Show => {
                println!("{}", toml::to_string_pretty(&config)?);
            }
            cli::ConfigAction::SetServer { host } => {
                let mut config = config;
                config.server.host = host;
                config.save()?;
                println!("Server host updated.");
            }
            cli::ConfigAction::SetPort { port } => {
                let mut config = config;
                config.server.port = port;
                config.save()?;
                println!("Server port updated.");
            }
        },

        // --- Login ---
        cli::Commands::Login { save } => {
            let username = prompt("Username: ")?;
            let password = prompt_password("Password: ")?;

            let mut client = SynoClient::new(&config.base_url());
            let mut auth = AuthApi::new(&mut client);

            auth.discover().await?;
            auth.login(&username, &password).await?;

            // Save session
            let sid = auth.client.sid().unwrap_or("").to_string();
            save_session(&sid)?;

            if save {
                let store = CredentialStore::from_config(&config.auth.credential_store);
                store.save(&username, &password)?;
                println!("Logged in. Credentials saved.");
            } else {
                println!("Logged in (session only, credentials not saved).");
            }
        }

        // --- Logout ---
        cli::Commands::Logout => {
            let mut client = SynoClient::new(&config.base_url());
            if let Some(sid) = load_session() {
                client.set_sid(sid);
                let mut auth = AuthApi::new(&mut client);
                auth.logout().await?;
            }
            clear_session()?;
            println!("Logged out.");
        }

        // --- Credentials ---
        cli::Commands::Credentials { action } => match action {
            cli::CredentialAction::Clear => {
                let store = CredentialStore::from_config(&config.auth.credential_store);
                store.clear()?;
                println!("Credentials cleared.");
            }
        },

        // --- Cache ---
        cli::Commands::Cache { action } => match action {
            cli::CacheAction::Status => {
                let cache = CacheManager::new(config.cache.clone());
                let status = cache.status()?;
                println!(
                    "Cache: {}",
                    if status.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
                println!("Path: {}", status.path.display());
                println!("Files: {}", status.file_count);
                println!(
                    "Size: {:.1} MB / {:.0} MB",
                    status.total_size_bytes as f64 / 1_048_576.0,
                    status.max_size_bytes as f64 / 1_048_576.0
                );
            }
            cli::CacheAction::Clear { older: _ } => {
                let cache = CacheManager::new(config.cache.clone());
                cache.clear()?;
                println!("Cache cleared.");
            }
            _ => {
                eprintln!("Not yet implemented.");
            }
        },

        // --- Not yet implemented ---
        _ => {
            eprintln!("Not yet implemented. Run `synoplayer --help` for usage.");
        }
    }

    Ok(())
}

fn prompt(msg: &str) -> anyhow::Result<String> {
    eprint!("{msg}");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_password(msg: &str) -> anyhow::Result<String> {
    // Simple password prompt (no echo hiding for now)
    prompt(msg)
}

fn session_path() -> std::path::PathBuf {
    AppConfig::session_path()
}

fn save_session(sid: &str) -> anyhow::Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::json!({
        "sid": sid,
        "created_at": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(path, serde_json::to_string(&data)?)?;
    Ok(())
}

fn load_session() -> Option<String> {
    let path = session_path();
    let content = std::fs::read_to_string(path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    data["sid"].as_str().map(|s| s.to_string())
}

fn clear_session() -> anyhow::Result<()> {
    let path = session_path();
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
