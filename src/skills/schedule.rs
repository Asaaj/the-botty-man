use anyhow::{Result, anyhow};
use chrono::{Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serenity::{
    all::{
        ActionRowComponent, ButtonStyle, CommandInteraction, ComponentInteraction,
        ComponentInteractionDataKind, Context, CreateActionRow, CreateButton, CreateCommand,
        CreateInputText, CreateInteractionResponse, CreateInteractionResponseMessage, CreateModal,
        CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, InputTextStyle,
        ModalInteraction, UserId,
    },
    async_trait,
    futures::lock::Mutex,
};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use super::Skill;
use crate::{
    cache::{Cache, CacheContainer},
    log::discord_log,
};

const PROPOSE_BTN_ID: &str = "schedule_btn_propose";
const MODIFY_BTN_ID: &str = "schedule_btn_modify";
const SCHEDULE_SELECT_ID: &str = "schedule_select";
const PROPOSE_MODAL_ID: &str = "schedule_propose";
const MODIFY_MODAL_PREFIX: &str = "schedule_modify:";

const FIELD_NAME: &str = "name";
const FIELD_START: &str = "start_date";
const FIELD_END: &str = "end_date";

#[derive(Debug)]
pub struct Schedule {
    cache: CacheContainer,
    proposed: Mutex<AllProposedSchedules>,
}

impl Schedule {
    pub async fn new(ctx: &Context) -> Self {
        let cache = Cache::entry(ctx, "schedule").await;
        let proposed = match cache.load::<AllProposedSchedules>("proposed.json").await {
            Ok(proposed) => proposed,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AllProposedSchedules::default(),
            Err(e) => {
                discord_log!(ctx, "Failed to load proposed schedules: {e}").await;
                AllProposedSchedules::default()
            }
        };
        Self {
            cache,
            proposed: Mutex::new(proposed),
        }
    }
}

fn parse_modal_fields(interaction: &ModalInteraction) -> (String, String, String) {
    let (mut name, mut start, mut end) = (String::new(), String::new(), String::new());
    for row in &interaction.data.components {
        for component in &row.components {
            if let ActionRowComponent::InputText(input) = component {
                let value = input.value.clone().unwrap_or_default();
                match input.custom_id.as_str() {
                    FIELD_NAME => name = value,
                    FIELD_START => start = value,
                    FIELD_END => end = value,
                    _ => {}
                }
            }
        }
    }
    (name, start, end)
}

fn propose_modal() -> CreateInteractionResponse {
    let today = Utc::now().date_naive();
    let start = today + Duration::weeks(1);
    let end = today + Duration::weeks(3);
    CreateInteractionResponse::Modal(
        CreateModal::new(PROPOSE_MODAL_ID, "New Schedule").components(vec![
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
    )
}

fn modify_modal(schedule: &ProposedSchedule) -> CreateInteractionResponse {
    CreateInteractionResponse::Modal(
        CreateModal::new(
            format!("{}{}", MODIFY_MODAL_PREFIX, schedule.name),
            "Modify Schedule",
        )
        .components(vec![
            CreateActionRow::InputText(
                CreateInputText::new(InputTextStyle::Short, "Schedule Name", FIELD_NAME)
                    .value(schedule.name.clone()),
            ),
            CreateActionRow::InputText(
                CreateInputText::new(InputTextStyle::Short, "Start Date", FIELD_START)
                    .value(schedule.start_date.format("%Y-%m-%d").to_string()),
            ),
            CreateActionRow::InputText(
                CreateInputText::new(InputTextStyle::Short, "End Date", FIELD_END)
                    .value(schedule.end_date.format("%Y-%m-%d").to_string()),
            ),
        ]),
    )
}

#[async_trait]
impl Skill for Schedule {
    fn name(&self) -> String {
        "schedule".into()
    }

    fn register(&self, _ctx: &Context) -> CreateCommand {
        CreateCommand::new(self.name()).description("Create or modify a schedule proposal")
    }

    async fn command(&self, ctx: &Context, interaction: &CommandInteraction) -> Result<()> {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .ephemeral(true)
                .components(vec![CreateActionRow::Buttons(vec![
                    CreateButton::new(PROPOSE_BTN_ID)
                        .label("Propose a date range")
                        .style(ButtonStyle::Primary),
                    CreateButton::new(MODIFY_BTN_ID)
                        .label("Modify an existing proposal")
                        .style(ButtonStyle::Secondary),
                ])]),
        );
        interaction.create_response(ctx, response).await?;
        Ok(())
    }

    async fn component(&self, ctx: &Context, interaction: &ComponentInteraction) -> Result<()> {
        match interaction.data.custom_id.as_str() {
            PROPOSE_BTN_ID => interaction.create_response(ctx, propose_modal()).await?,
            MODIFY_BTN_ID => {
                let response = {
                    let proposed = self.proposed.lock().await;
                    if proposed.schedules.is_empty() {
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("No proposed schedules to modify.")
                                .ephemeral(true),
                        )
                    } else {
                        let options = proposed
                            .schedules
                            .iter()
                            .map(|s| CreateSelectMenuOption::new(s.name.clone(), s.name.clone()))
                            .collect();
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .ephemeral(true)
                                .components(vec![CreateActionRow::SelectMenu(
                                    CreateSelectMenu::new(
                                        SCHEDULE_SELECT_ID,
                                        CreateSelectMenuKind::String { options },
                                    ),
                                )]),
                        )
                    }
                };
                interaction.create_response(ctx, response).await?
            }
            SCHEDULE_SELECT_ID => {
                let ComponentInteractionDataKind::StringSelect { values } = &interaction.data.kind
                else {
                    return Ok(());
                };
                let schedule_name = &values[0];
                let modal = {
                    let proposed = self.proposed.lock().await;
                    let schedule = proposed
                        .schedules
                        .iter()
                        .find(|s| &s.name == schedule_name)
                        .ok_or_else(|| anyhow!("Schedule '{schedule_name}' not found"))?;
                    modify_modal(schedule)
                };
                interaction.create_response(ctx, modal).await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn modal(&self, ctx: &Context, interaction: &ModalInteraction) -> Result<()> {
        let (name, start_str, end_str) = parse_modal_fields(interaction);
        let start_date = NaiveDate::parse_from_str(&start_str, "%Y-%m-%d")?;
        let end_date = NaiveDate::parse_from_str(&end_str, "%Y-%m-%d")?;

        match interaction.data.custom_id.as_str() {
            PROPOSE_MODAL_ID => {
                self.proposed.lock().await.schedules.push(ProposedSchedule {
                    name: name.clone(),
                    proposed_by: interaction.user.id,
                    is_active: true,
                    start_date,
                    end_date,
                });
                interaction
                    .create_response(
                        ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!(
                                    "Proposed **{name}** from {start_str} to {end_str}"
                                ))
                                .ephemeral(true),
                        ),
                    )
                    .await?;
            }
            id if id.starts_with(MODIFY_MODAL_PREFIX) => {
                let original_name = &id[MODIFY_MODAL_PREFIX.len()..];
                {
                    let mut proposed = self.proposed.lock().await;
                    if let Some(schedule) = proposed
                        .schedules
                        .iter_mut()
                        .find(|s| s.name == original_name)
                    {
                        schedule.name = name.clone();
                        schedule.start_date = start_date;
                        schedule.end_date = end_date;
                    }
                }
                interaction
                    .create_response(
                        ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!(
                                    "Updated schedule to **{name}** from {start_str} to {end_str}"
                                ))
                                .ephemeral(true),
                        ),
                    )
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct AllProposedSchedules {
    schedules: Vec<ProposedSchedule>,
}
impl AllProposedSchedules {
    fn active(&self) -> impl Iterator<Item = &ProposedSchedule> {
        self.schedules.iter().filter(|sched| sched.is_active)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ProposedSchedule {
    name: String,
    proposed_by: UserId,
    is_active: bool,
    start_date: NaiveDate,
    end_date: NaiveDate,
}
