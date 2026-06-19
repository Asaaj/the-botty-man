use std::path::PathBuf;

use clap::Parser;
use serenity::{
    Client,
    all::{ChannelId, GatewayIntents},
};

mod bot;
mod cache;
mod log;
mod skills;
use bot::Handler;

#[derive(Parser, Debug)]
pub struct Config {
    /// Token for the Discord bot (Bot page in the Applications section of the developer portal).
    #[arg(long, env = "DISCORD_TOKEN")]
    pub bot_token: String,

    /// Channel ID (snowflake) to send log messages to.
    #[arg(long, env = "LOGGING_CHANNEL_ID")]
    pub logging_channel: String,

    /// Guild ID (snowflake) to register commands on. Omit to register globally (up to 1 hour propagation).
    #[arg(long, env = "GUILD_ID")]
    pub guild_id: Option<String>,

    /// Directory used for caching data.
    #[arg(long, env = "CACHE_LOC", default_value = ".cache")]
    pub cache_loc: PathBuf,
}

#[tokio::main]
async fn main() {
    let config = Config::parse();
    log::init(ChannelId::new(
        config
            .logging_channel
            .parse()
            .expect("logging_channel must be a valid channel ID (snowflake)"),
    ));
    cache::Cache::install(&config.cache_loc).unwrap();

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&config.bot_token, intents)
        .event_handler(Handler::from_config(&config))
        .await
        .expect("Error creating client");

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        eprintln!("\nGoodbye!");
        shard_manager.shutdown_all().await;
    });

    if let Err(e) = client.start().await {
        eprintln!("Client error: {e:?}");
    }
}
