pub mod schedule;

use serenity::all::{Context, CreateCommand, Interaction};

pub fn all_commands() -> Vec<CreateCommand> {
    vec![schedule::register()]
}

pub async fn dispatch(ctx: &Context, interaction: &Interaction) {
    match interaction {
        Interaction::Command(command) => match command.data.name.as_str() {
            schedule::NAME => schedule::run(ctx, command).await,
            name => eprintln!("Unknown command: {name}"),
        },
        Interaction::Modal(modal) => match modal.data.custom_id.as_str() {
            schedule::MODAL_ID => schedule::handle_modal(ctx, modal).await,
            id => eprintln!("Unknown modal: {id}"),
        },
        _ => {}
    }
}
