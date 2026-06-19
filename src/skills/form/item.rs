//! The per-record knowledge a [`FormCollection`](super::FormCollection) needs.

use std::fmt::Debug;

use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};

use super::{Field, Input};

/// A record a form workflow can create, edit, and list. Implementors own *all*
/// field layout and parsing; the collection, workflow, and rendering stay
/// generic over this trait.
///
/// `blank_fields`/`edit_fields` feed the modal; `create`/`update` read the
/// submitted [`Input`] back into a record; `label` identifies it in menus and is
/// the key `Effect::Update` matches on.
pub trait FormItem: Serialize + DeserializeOwned + Send + Sync + Sized + Debug {
    /// Fields for a blank "create" form.
    fn blank_fields() -> Vec<Field>;

    /// Fields pre-filled to edit this record.
    fn edit_fields(&self) -> Vec<Field>;

    /// Identifier shown in select menus and used to locate the record.
    fn label(&self) -> String;

    /// One-line summary used in confirmation messages.
    fn summary(&self) -> String;

    /// Build a new record from a submitted form.
    fn create(input: &Input) -> Result<Self>;

    /// Update this record from a submitted form.
    fn update(&mut self, input: &Input) -> Result<()>;
}
