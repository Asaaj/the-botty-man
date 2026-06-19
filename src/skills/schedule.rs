use chrono::{Duration, Utc};
use serenity::all::{
    ActionRowComponent, CommandInteraction, Context, CreateActionRow, CreateCommand,
    CreateInputText, CreateInteractionResponse, CreateInteractionResponseMessage, CreateModal,
    InputTextStyle, ModalInteraction,
};

pub const NAME: &str = "schedule";
pub const MODAL_ID: &str = "schedule_modal";

const FIELD_NAME: &str = "name";
const FIELD_START: &str = "start_date";
const FIELD_END: &str = "end_date";

pub fn register() -> CreateCommand {
    CreateCommand::new(NAME).description("Create a new schedule")
}

pub async fn run(ctx: &Context, interaction: &CommandInteraction) {
    let today = Utc::now().date_naive();
    let start = today + Duration::weeks(1);
    let end = today + Duration::weeks(3);

    let modal = CreateInteractionResponse::Modal(
        CreateModal::new(MODAL_ID, "New Schedule").components(vec![
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

    if let Err(e) = interaction.create_response(ctx, modal).await {
        eprintln!("Failed to open schedule modal: {e:?}");
    }
}

pub async fn handle_modal(ctx: &Context, interaction: &ModalInteraction) {
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
    if let Err(e) = interaction.create_response(ctx, response).await {
        eprintln!("Failed to respond to schedule modal: {e:?}");
    }
}
