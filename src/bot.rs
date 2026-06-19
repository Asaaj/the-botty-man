use std::sync::OnceLock;

use serenity::{
    all::{Command, Context, EventHandler, GuildId, Interaction, Message, Ready, UserId},
    async_trait,
};

use crate::{Config, log::discord_log, skills};

static ME: OnceLock<UserId> = OnceLock::new();

pub struct Handler {
    guild_id: Option<GuildId>,
}

impl Handler {
    pub fn from_config(config: &Config) -> Self {
        Handler {
            guild_id: config.guild_id.as_deref().map(|id| {
                GuildId::new(
                    id.parse()
                        .expect("guild_id must be a valid guild ID (snowflake)"),
                )
            }),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if ME.get().copied() != Some(msg.author.id) {
            discord_log!(ctx, "{msg:?}").await;
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        skills::dispatch(&ctx, &interaction).await;
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        ME.set(ready.user.id).ok();
        let result = match self.guild_id {
            Some(guild_id) => guild_id
                .set_commands(&ctx.http, skills::all_commands())
                .await
                .map(|_| ()),
            None => Command::set_global_commands(&ctx.http, skills::all_commands())
                .await
                .map(|_| ()),
        };
        if let Err(e) = result {
            eprintln!("Failed to register commands: {e:?}");
        }
        discord_log!(ctx, "{} is connected!", ready.user.name).await;
    }
}
