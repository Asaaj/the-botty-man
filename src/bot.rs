use std::sync::OnceLock;

use serenity::{
    all::{Command, Context, EventHandler, GuildId, Interaction, Message, Ready, UserId},
    async_trait,
};

use crate::{Config, log::discord_log, skills::SkillRegistry};

static ME: OnceLock<UserId> = OnceLock::new();

pub struct Handler {
    guild_id: Option<GuildId>,
    skills: OnceLock<SkillRegistry>,
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
            skills: OnceLock::new(),
        }
    }

    fn skills(&self) -> &SkillRegistry {
        self.skills.get().expect("skills not initialized")
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        ME.set(ready.user.id).ok();
        self.skills.set(SkillRegistry::new(&ctx).await).ok();
        let skills = self.skills();

        let result = match self.guild_id {
            Some(guild_id) => guild_id
                .set_commands(&ctx.http, skills.all_commands(&ctx))
                .await
                .map(|_| ()),
            None => Command::set_global_commands(&ctx.http, skills.all_commands(&ctx))
                .await
                .map(|_| ()),
        };
        if let Err(e) = result {
            eprintln!("Failed to register commands: {e:?}");
        }
        discord_log!(ctx, "{} is connected!", ready.user.name).await;
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if ME.get().copied() != Some(msg.author.id) {
            discord_log!(ctx, "{msg:?}").await;
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Err(e) = self.skills().dispatch(&ctx, &interaction).await {
            discord_log!(ctx, "Failed to create interaction: {e}").await;
        }
    }
}
