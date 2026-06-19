//! Backend-agnostic UI description and its single translation into serenity.
//!
//! Workflow code builds `Screen`s; [`Screen::into_response`] is the *only* place
//! that constructs serenity component/modal builders. To change how a control
//! looks, or to ride out a serenity API change, edit this file and nothing else.

use serenity::all::{
    ButtonStyle, CreateActionRow, CreateButton, CreateInputText, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateModal, CreateSelectMenu, CreateSelectMenuKind,
    CreateSelectMenuOption, InputTextStyle,
};

use super::Route;

/// What to present to the user in response to an interaction.
#[derive(Debug, Clone)]
pub enum Screen {
    /// A row of buttons, each routing somewhere when clicked.
    Buttons {
        prompt: Option<String>,
        controls: Vec<Control>,
    },
    /// A single-select dropdown. Chosen values arrive back in `Input::selected`.
    Select {
        route: Route,
        placeholder: String,
        options: Vec<Choice>,
    },
    /// A modal form. Submitted values arrive back in `Input::fields`.
    Form {
        route: Route,
        title: String,
        fields: Vec<Field>,
    },
    /// A terminal text reply.
    Message(String),
}

/// A button: a label, a visual style, and where clicking it goes.
#[derive(Debug, Clone)]
pub struct Control {
    pub label: String,
    pub style: Style,
    pub route: Route,
}

/// One option in a select menu.
#[derive(Debug, Clone)]
pub struct Choice {
    pub label: String,
    pub value: String,
}

/// One field in a modal form. `id` keys the value back out of `Input::fields`.
#[derive(Debug, Clone)]
pub struct Field {
    pub id: String,
    pub label: String,
    pub value: String,
    pub placeholder: Option<String>,
    pub multiline: bool,
}

/// Button styling, decoupled from serenity's `ButtonStyle`.
#[derive(Debug, Clone, Copy)]
pub enum Style {
    Primary,
    Secondary,
    Danger,
}

impl Screen {
    /// The single bottleneck translating a `Screen` into serenity's response
    /// type. Replies are ephemeral; change that here if a workflow needs it.
    pub fn into_response(self) -> CreateInteractionResponse {
        match self {
            Screen::Buttons { prompt, controls } => {
                let buttons = controls.into_iter().map(Control::into_button).collect();
                let mut msg = CreateInteractionResponseMessage::new()
                    .ephemeral(true)
                    .components(vec![CreateActionRow::Buttons(buttons)]);
                if let Some(prompt) = prompt {
                    msg = msg.content(prompt);
                }
                CreateInteractionResponse::Message(msg)
            }
            Screen::Select {
                route,
                placeholder,
                options,
            } => {
                let options = options.into_iter().map(Choice::into_option).collect();
                let menu =
                    CreateSelectMenu::new(route.encode(), CreateSelectMenuKind::String { options })
                        .placeholder(placeholder);
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .ephemeral(true)
                        .components(vec![CreateActionRow::SelectMenu(menu)]),
                )
            }
            Screen::Form {
                route,
                title,
                fields,
            } => {
                let rows = fields.into_iter().map(Field::into_row).collect();
                CreateInteractionResponse::Modal(
                    CreateModal::new(route.encode(), title).components(rows),
                )
            }
            Screen::Message(content) => CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .ephemeral(true)
                    .content(content),
            ),
        }
    }
}

impl Control {
    fn into_button(self) -> CreateButton {
        CreateButton::new(self.route.encode())
            .label(self.label)
            .style(self.style.into())
    }
}

impl Choice {
    fn into_option(self) -> CreateSelectMenuOption {
        CreateSelectMenuOption::new(self.label, self.value)
    }
}

impl Field {
    fn into_row(self) -> CreateActionRow {
        let style = if self.multiline {
            InputTextStyle::Paragraph
        } else {
            InputTextStyle::Short
        };
        // serenity arg order is (style, label, custom_id) — easy to swap; see CLAUDE.md.
        let mut input = CreateInputText::new(style, self.label, self.id).value(self.value);
        if let Some(placeholder) = self.placeholder {
            input = input.placeholder(placeholder);
        }
        CreateActionRow::InputText(input)
    }
}

impl From<Style> for ButtonStyle {
    fn from(style: Style) -> Self {
        match style {
            Style::Primary => ButtonStyle::Primary,
            Style::Secondary => ButtonStyle::Secondary,
            Style::Danger => ButtonStyle::Danger,
        }
    }
}
