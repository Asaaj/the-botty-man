use anyhow::{Result, anyhow};
use chrono::{Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serenity::{
    all::{
        CommandInteraction, ComponentInteraction, Context, CreateCommand,
        CreateInteractionResponse, ModalInteraction, UserId,
    },
    async_trait,
};

use super::{
    Skill,
    form::{Choice, Control, Effect, Field, FormCollection, FormItem, Input, Route, Screen, Style},
};
use crate::cache::Cache;

const NAME: &str = "schedule";

const FIELD_NAME: &str = "name";
const FIELD_START: &str = "start_date";
const FIELD_END: &str = "end_date";

#[derive(Debug)]
pub struct Schedule {
    collection: FormCollection<ProposedSchedule>,
}

impl Schedule {
    pub async fn new(ctx: &Context) -> Self {
        let cache = Cache::entry(ctx, NAME).await;
        Self {
            collection: FormCollection::load(ctx, cache, "proposed.json").await,
        }
    }

    /// Decode an inbound component/modal interaction and run it through the
    /// workflow: `Step` → `advance` → render the next screen or run the
    /// side-effect hook.
    async fn drive(&self, ctx: &Context, input: Input) -> Result<CreateInteractionResponse> {
        let step = Step::from_route(&input.route)
            .ok_or_else(|| anyhow!("unknown route: {}", input.route.encode()))?;
        let screen = match step.advance(&input) {
            Next::Show(step) => {
                let items = self.collection.items().await;
                step.screen(items.as_slice())
            }
            Next::Run(effect) => Screen::Message(self.collection.run(ctx, &effect, &input).await?),
            Next::Reply(message) => Screen::Message(message),
        };
        Ok(screen.into_response())
    }
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
        let response = {
            let items = self.collection.items().await;
            Step::Choose.screen(items.as_slice()).into_response()
        };
        interaction.create_response(ctx, response).await?;
        Ok(())
    }

    async fn component(&self, ctx: &Context, interaction: &ComponentInteraction) -> Result<()> {
        let Some(input) = Input::from_component(interaction) else {
            return Ok(());
        };
        let response = self.drive(ctx, input).await?;
        interaction.create_response(ctx, response).await?;
        Ok(())
    }

    async fn modal(&self, ctx: &Context, interaction: &ModalInteraction) -> Result<()> {
        let Some(input) = Input::from_modal(interaction) else {
            return Ok(());
        };
        let response = self.drive(ctx, input).await?;
        interaction.create_response(ctx, response).await?;
        Ok(())
    }
}

/// The `/schedule` interaction as a state machine. Each variant is a point the
/// user can be at; the nested `Propose`/`Modify` types own their sub-menus.
///
/// Steps map one-to-one onto `custom_id`s via [`Step::route`] /
/// [`Step::from_route`], and render themselves via [`Step::screen`].
#[derive(Debug, Clone)]
enum Step {
    /// `/schedule` invoked → offer the two choices.
    Choose,
    /// The "propose a new schedule" branch.
    Propose(Propose),
    /// The "modify an existing schedule" branch.
    Modify(Modify),
}

#[derive(Debug, Clone)]
enum Propose {
    /// Button picked at the fork → show a blank form.
    New,
    /// The blank form was submitted (inbound only).
    Submit,
}

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
    /// `advance` before anything is rendered; their arms here are placeholders.
    /// Field and label layout come from the record's [`FormItem`] impl.
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
            Step::Propose(Propose::New) => Screen::Form {
                route: Step::Propose(Propose::Submit).route(),
                title: "New Schedule".into(),
                fields: ProposedSchedule::blank_fields(),
            },
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
                        fields: schedule.edit_fields(),
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

fn date_field(id: &str, label: &str, date: NaiveDate) -> Field {
    Field {
        id: id.into(),
        label: label.into(),
        value: date.format("%Y-%m-%d").to_string(),
        placeholder: None,
        multiline: false,
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

impl FormItem for ProposedSchedule {
    fn blank_fields() -> Vec<Field> {
        let today = Utc::now().date_naive();
        vec![
            Field {
                id: FIELD_NAME.into(),
                label: "Schedule Name".into(),
                value: String::new(),
                placeholder: Some("e.g. Next D&D session".into()),
                multiline: false,
            },
            date_field(FIELD_START, "Start Date", today + Duration::weeks(1)),
            date_field(FIELD_END, "End Date", today + Duration::weeks(3)),
        ]
    }

    fn edit_fields(&self) -> Vec<Field> {
        vec![
            Field {
                id: FIELD_NAME.into(),
                label: "Schedule Name".into(),
                value: self.name.clone(),
                placeholder: None,
                multiline: false,
            },
            date_field(FIELD_START, "Start Date", self.start_date),
            date_field(FIELD_END, "End Date", self.end_date),
        ]
    }

    fn label(&self) -> String {
        self.name.clone()
    }

    fn summary(&self) -> String {
        format!(
            "**{}** from {} to {}",
            self.name,
            self.start_date.format("%Y-%m-%d"),
            self.end_date.format("%Y-%m-%d"),
        )
    }

    fn create(input: &Input) -> Result<Self> {
        Ok(ProposedSchedule {
            name: input.field(FIELD_NAME).unwrap_or_default().to_string(),
            proposed_by: input.user,
            is_active: true,
            start_date: parse_date(input.field(FIELD_START))?,
            end_date: parse_date(input.field(FIELD_END))?,
        })
    }

    fn update(&mut self, input: &Input) -> Result<()> {
        // Parse everything before mutating so a bad date can't half-update the
        // record (which would then get persisted).
        let name = input.field(FIELD_NAME).unwrap_or_default().to_string();
        let start_date = parse_date(input.field(FIELD_START))?;
        let end_date = parse_date(input.field(FIELD_END))?;
        self.name = name;
        self.start_date = start_date;
        self.end_date = end_date;
        Ok(())
    }
}

fn parse_date(value: Option<&str>) -> Result<NaiveDate> {
    Ok(NaiveDate::parse_from_str(
        value.unwrap_or_default(),
        "%Y-%m-%d",
    )?)
}
