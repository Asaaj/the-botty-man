//! Generic building blocks for menu-driven skill workflows.
//!
//! A workflow is a state machine whose states are types defined *by each skill*
//! (kept skill-flavored on purpose). This module supplies the reusable spine:
//!
//! - [`Screen`] + [`Screen::into_response`] — the only place that touches
//!   serenity's component/modal builders.
//! - [`Route`] — typed `custom_id` encode/decode, replacing prefix matching.
//! - [`Input`] — a neutral view of an inbound interaction, decoded once at the
//!   dispatch boundary so workflow logic never sees `ComponentInteraction` /
//!   `ModalInteraction`.
#![allow(dead_code)] // reusable vocabulary; some surface (e.g. Style::Danger) isn't exercised yet

mod collection;
mod item;
mod route;
mod screen;

use std::collections::HashMap;

pub use collection::{Effect, FormCollection};
pub use item::FormItem;
pub use route::Route;
pub use screen::{Choice, Control, Field, Screen, Style};
use serenity::all::{
    ActionRowComponent, ComponentInteraction, ComponentInteractionDataKind, ModalInteraction,
    UserId,
};

/// A neutral, backend-agnostic view of an inbound interaction.
///
/// Decoded once at the dispatch boundary. A button click and a select choice
/// both populate `route` (+ `selected` for selects); a modal submit populates
/// `route` + `fields`.
#[derive(Debug, Clone)]
pub struct Input {
    /// The route encoded in the triggering control's `custom_id`.
    pub route: Route,
    /// The user who triggered the interaction.
    pub user: UserId,
    /// Values from a select menu (empty for buttons / modals).
    pub selected: Vec<String>,
    /// Submitted modal fields, keyed by [`Field::id`]. Empty for components.
    pub fields: HashMap<String, String>,
}

impl Input {
    /// Build from a button click or select-menu choice.
    pub fn from_component(interaction: &ComponentInteraction) -> Option<Self> {
        let route = Route::decode(&interaction.data.custom_id)?;
        let selected = match &interaction.data.kind {
            ComponentInteractionDataKind::StringSelect { values } => values.clone(),
            _ => Vec::new(),
        };
        Some(Self {
            route,
            user: interaction.user.id,
            selected,
            fields: HashMap::new(),
        })
    }

    /// Build from a modal submission.
    pub fn from_modal(interaction: &ModalInteraction) -> Option<Self> {
        let route = Route::decode(&interaction.data.custom_id)?;
        let mut fields = HashMap::new();
        for row in &interaction.data.components {
            for component in &row.components {
                if let ActionRowComponent::InputText(input) = component {
                    fields.insert(
                        input.custom_id.clone(),
                        input.value.clone().unwrap_or_default(),
                    );
                }
            }
        }
        Some(Self {
            route,
            user: interaction.user.id,
            selected: Vec::new(),
            fields,
        })
    }

    /// The first selected value, if any.
    pub fn first_selected(&self) -> Option<&str> {
        self.selected.first().map(String::as_str)
    }

    /// A submitted field by id.
    pub fn field(&self, id: &str) -> Option<&str> {
        self.fields.get(id).map(String::as_str)
    }
}
