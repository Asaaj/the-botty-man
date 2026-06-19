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

use super::{
    Skill,
    form::{Choice, Control, Field, Input, Route, Screen, Style},
};
use crate::{
    cache::{Cache, CacheContainer},
    log::discord_log,
};

const NAME: &str = "schedule";

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
        NAME.into()
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

/// The `/schedule` interaction as a state machine. Each variant is a point the
/// user can be at; the nested `Propose`/`Modify` types own their sub-menus.
///
/// Steps map one-to-one onto `custom_id`s via [`Step::route`] /
/// [`Step::from_route`], and render themselves via [`Step::screen`].
#[allow(dead_code)] // wired into the Skill impl in a later step
#[derive(Debug, Clone)]
enum Step {
    /// `/schedule` invoked → offer the two choices.
    Choose,
    /// The "propose a new schedule" branch.
    Propose(Propose),
    /// The "modify an existing schedule" branch.
    Modify(Modify),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
enum Propose {
    /// Button picked at the fork → show a blank form.
    New,
    /// The blank form was submitted (inbound only).
    Submit,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
enum Modify {
    /// Button picked at the fork → show the select menu (inbound + renders).
    List,
    /// A schedule was chosen; the name rides in `Input::selected` (inbound only).
    Pick,
    /// Show the pre-filled form for `target` (reached via `advance`, render only).
    Edit { target: String },
    /// The edited form was submitted (inbound only).
    Submit { target: String },
}

/// What handling an inbound interaction yields. `advance` produces this purely;
/// the dispatcher interprets it (rendering, or running the side-effect hook).
#[allow(dead_code)]
#[derive(Debug, Clone)]
enum Next {
    /// Render this step's `screen()` as the reply (e.g. a pick → an edit form).
    Show(Step),
    /// Perform this side-effect on the collection, then confirm. The actual
    /// mutation is the caller-supplied hook, keeping it out of `Step`.
    Run(Effect),
    /// Terminal message — a validation failure or simple acknowledgement.
    Reply(String),
}

/// A mutation `advance` requests but does not perform. Described declaratively
/// here; carried out by the interpreter's hook (which owns the collection and,
/// later, a `FormItem` to build/update records from `Input::fields`).
#[allow(dead_code)]
#[derive(Debug, Clone)]
enum Effect {
    /// Create a new record from the submitted form fields.
    Create,
    /// Update the record named `target` from the submitted form fields.
    Update { target: String },
}

#[allow(dead_code)]
impl Step {
    /// Pure transition for an inbound interaction. Renders are returned as
    /// `Show`; the only impure work — collection mutation — is deferred to a
    /// `Run(Effect)` the caller's hook interprets. This is what lets the edit
    /// form keep being described by `screen()`: a `Pick` resolves to `Edit`
    /// here, and rendering happens later, declaratively.
    fn advance(self, input: &Input) -> Next {
        match self {
            // Inbound steps that simply render their own screen.
            Step::Choose | Step::Propose(Propose::New) | Step::Modify(Modify::List) => {
                Next::Show(self)
            }
            // A schedule was picked: lift the selection into the edit form's
            // target, then let `screen()` render it.
            Step::Modify(Modify::Pick) => match input.first_selected() {
                Some(target) => Next::Show(Step::Modify(Modify::Edit {
                    target: target.to_string(),
                })),
                None => Next::Reply("No schedule selected.".into()),
            },
            // Render-only state; produced by the arm above. Showing is a no-op.
            Step::Modify(Modify::Edit { .. }) => Next::Show(self),
            // Form submissions become declarative side-effects.
            Step::Propose(Propose::Submit) => Next::Run(Effect::Create),
            Step::Modify(Modify::Submit { target }) => Next::Run(Effect::Update { target }),
        }
    }

    /// This step's inbound `custom_id`. Render-only `Edit` shares `Submit`'s
    /// address (the form it shows posts there) and is never decoded back.
    fn route(&self) -> Route {
        match self {
            Step::Choose => Route::new(NAME, std::iter::empty::<&str>()),
            Step::Propose(Propose::New) => Route::new(NAME, ["propose"]),
            Step::Propose(Propose::Submit) => Route::new(NAME, ["propose", "submit"]),
            Step::Modify(Modify::List) => Route::new(NAME, ["modify"]),
            Step::Modify(Modify::Pick) => Route::new(NAME, ["modify", "pick"]),
            Step::Modify(Modify::Edit { target } | Modify::Submit { target }) => {
                Route::new(NAME, [
                    "modify".to_string(),
                    "submit".to_string(),
                    target.clone(),
                ])
            }
        }
    }

    /// Decode an inbound interaction's `custom_id` into the step it addresses.
    fn from_route(route: &Route) -> Option<Self> {
        if route.skill != NAME {
            return None;
        }
        let segments: Vec<&str> = route.segments.iter().map(String::as_str).collect();
        Some(match segments.as_slice() {
            [] => Step::Choose,
            ["propose"] => Step::Propose(Propose::New),
            ["propose", "submit"] => Step::Propose(Propose::Submit),
            ["modify"] => Step::Modify(Modify::List),
            ["modify", "pick"] => Step::Modify(Modify::Pick),
            ["modify", "submit", target] => Step::Modify(Modify::Submit {
                target: target.to_string(),
            }),
            _ => return None,
        })
    }

    /// The UI this step presents. Interactive elements route to the *next* step.
    ///
    /// The inbound-only states (`*::Submit`, `Modify::Pick`) are consumed by
    /// `advance` (a later step) before anything is rendered; their arms here are
    /// placeholders. Item field/label access is inlined for now and will move
    /// onto a `FormItem` trait when the generic collection lands.
    fn screen(&self, items: &[ProposedSchedule]) -> Screen {
        match self {
            Step::Choose => Screen::Buttons {
                prompt: None,
                controls: vec![
                    Control {
                        label: "Propose a date range".into(),
                        style: Style::Primary,
                        route: Step::Propose(Propose::New).route(),
                    },
                    Control {
                        label: "Modify an existing proposal".into(),
                        style: Style::Secondary,
                        route: Step::Modify(Modify::List).route(),
                    },
                ],
            },
            Step::Propose(Propose::New) => {
                let today = Utc::now().date_naive();
                Screen::Form {
                    route: Step::Propose(Propose::Submit).route(),
                    title: "New Schedule".into(),
                    fields: vec![
                        Field {
                            id: FIELD_NAME.into(),
                            label: "Schedule Name".into(),
                            value: String::new(),
                            placeholder: Some("e.g. Next D&D session".into()),
                            multiline: false,
                        },
                        date_field(FIELD_START, "Start Date", today + Duration::weeks(1)),
                        date_field(FIELD_END, "End Date", today + Duration::weeks(3)),
                    ],
                }
            }
            Step::Modify(Modify::List) => {
                if items.is_empty() {
                    Screen::Message("No proposed schedules to modify.".into())
                } else {
                    Screen::Select {
                        route: Step::Modify(Modify::Pick).route(),
                        placeholder: "Choose a schedule".into(),
                        options: items
                            .iter()
                            .map(|s| Choice {
                                label: s.name.clone(),
                                value: s.name.clone(),
                            })
                            .collect(),
                    }
                }
            }
            Step::Modify(Modify::Edit { target }) => {
                match items.iter().find(|s| &s.name == target) {
                    Some(schedule) => Screen::Form {
                        route: Step::Modify(Modify::Submit {
                            target: target.clone(),
                        })
                        .route(),
                        title: "Modify Schedule".into(),
                        fields: vec![
                            Field {
                                id: FIELD_NAME.into(),
                                label: "Schedule Name".into(),
                                value: schedule.name.clone(),
                                placeholder: None,
                                multiline: false,
                            },
                            date_field(FIELD_START, "Start Date", schedule.start_date),
                            date_field(FIELD_END, "End Date", schedule.end_date),
                        ],
                    },
                    None => Screen::Message(format!("Schedule '{target}' not found.")),
                }
            }
            // Inbound-only steps; `advance` handles these before any render.
            Step::Propose(Propose::Submit)
            | Step::Modify(Modify::Pick)
            | Step::Modify(Modify::Submit { .. }) => Screen::Message("Working…".into()),
        }
    }
}

#[allow(dead_code)]
fn date_field(id: &str, label: &str, date: NaiveDate) -> Field {
    Field {
        id: id.into(),
        label: label.into(),
        value: date.format("%Y-%m-%d").to_string(),
        placeholder: None,
        multiline: false,
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
