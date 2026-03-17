use clap::Parser;
use synoplayer::cli;
use synoplayer::config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = cli::Cli::parse();

    // Command dispatch will be implemented in Etap 1
    match cli.command {
        cli::Commands::Config { action } => match action {
            cli::ConfigAction::Show => {
                let config = config::model::AppConfig::load()?;
                println!("{}", toml::to_string_pretty(&config)?);
            }
            _ => {
                eprintln!("Not yet implemented");
            }
        },
        _ => {
            eprintln!("Not yet implemented. Run `synoplayer --help` for usage.");
        }
    }

    Ok(())
}
