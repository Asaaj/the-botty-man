use std::sync::OnceLock;

use serenity::{
    all::{Context, EventHandler, Message, Ready, UserId},
    async_trait,
};

use crate::{Config, log::discord_log};

static ME: OnceLock<UserId> = OnceLock::new();

pub struct Handler;

impl Handler {
    pub fn from_config(_config: &Config) -> Self {
        Handler
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if ME.get().copied() != Some(msg.author.id) {
            discord_log!(ctx, "{msg:?}").await;
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        ME.set(ready.user.id).ok();
        discord_log!(ctx, "{} is connected!", ready.user.name).await;
    }
}
