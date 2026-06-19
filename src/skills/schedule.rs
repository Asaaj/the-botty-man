use anyhow::Result;
use chrono::{Duration, Utc};
use serenity::{
    all::{
        ActionRowComponent, CommandInteraction, Context, CreateActionRow, CreateCommand,
        CreateInputText, CreateInteractionResponse, CreateInteractionResponseMessage, CreateModal,
        InputTextStyle, ModalInteraction,
    },
    async_trait,
};

use super::Skill;
use crate::cache::{Cache, CacheContainer};

const FIELD_NAME: &str = "name";
const FIELD_START: &str = "start_date";
const FIELD_END: &str = "end_date";

#[derive(Debug)]
pub struct Schedule {
    cache: CacheContainer,
}

impl Schedule {
    pub async fn new(ctx: &Context) -> Self {
        let cache = Cache::entry(ctx, "schedule").await;
        Self { cache }
    }
}

#[async_trait]
impl Skill for Schedule {
    fn name(&self) -> String {
        "schedule".into()
    }

    fn register(&self, _ctx: &Context) -> CreateCommand {
        CreateCommand::new(self.name()).description("Create a new schedule")
    }

    async fn command(&self, ctx: &Context, interaction: &CommandInteraction) -> Result<()> {
        let today = Utc::now().date_naive();
        let start = today + Duration::weeks(1);
        let end = today + Duration::weeks(3);

        let modal = CreateInteractionResponse::Modal(
            CreateModal::new(self.name(), "New Schedule").components(vec![
                CreateActionRow::InputText(
                    CreateInputText::new(InputTextStyle::Short, "Schedule Name", FIELD_NAME)
                        .placeholder("e.g. Next D&D session"),
                ),
                CreateActionRow::InputText(
                    CreateInputText::new(InputTextStyle::Short, "Start Date", FIELD_START)
                        .value(start.format("%Y-%m-%d").to_string()),
                ),
                CreateActionRow::InputText(
                    CreateInputText::new(InputTextStyle::Short, "End Date", FIELD_END)
                        .value(end.format("%Y-%m-%d").to_string()),
                ),
            ]),
        );

        interaction.create_response(ctx, modal).await?;
        Ok(())
    }

    async fn modal(&self, ctx: &Context, interaction: &ModalInteraction) -> Result<()> {
        let mut name = String::new();
        let mut start = String::new();
        let mut end = String::new();

        for row in &interaction.data.components {
            for component in &row.components {
                if let ActionRowComponent::InputText(input) = component {
                    match input.custom_id.as_str() {
                        FIELD_NAME => name = input.value.clone().unwrap_or_default(),
                        FIELD_START => start = input.value.clone().unwrap_or_default(),
                        FIELD_END => end = input.value.clone().unwrap_or_default(),
                        _ => {}
                    }
                }
            }
        }

        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(format!("Scheduled **{name}** from {start} to {end}"))
                .ephemeral(true),
        );
        interaction.create_response(ctx, response).await?;
        Ok(())
    }
}
