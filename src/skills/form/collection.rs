//! The reusable storage + side-effect spine behind form workflows.

use std::io::ErrorKind;

use anyhow::{Result, anyhow};
use serenity::{
    all::Context,
    futures::lock::{Mutex, MutexGuard},
};

use super::{FormItem, Input};
use crate::{cache::CacheContainer, log::discord_log};

/// A mutation a workflow's `advance` *describes* but does not perform. Carried
/// out by [`FormCollection::run`] — the custom hook that owns the records.
#[derive(Debug, Clone)]
pub enum Effect {
    /// Create a new record from the submitted form fields.
    Create,
    /// Update the record identified by `target` from the submitted fields.
    Update { target: String },
}

/// A cache-backed list of [`FormItem`] records plus the hook that interprets
/// [`Effect`]s against them. Every mutation is persisted to `file` before the
/// confirmation is returned, so the on-disk copy never lags the working set.
#[derive(Debug)]
pub struct FormCollection<T: FormItem> {
    items: Mutex<Vec<T>>,
    cache: CacheContainer,
    file: &'static str,
}

impl<T: FormItem> FormCollection<T> {
    /// Load the records from `file` within `cache`, defaulting to empty when the
    /// file is absent and logging (but tolerating) any other read failure.
    pub async fn load(ctx: &Context, cache: CacheContainer, file: &'static str) -> Self {
        let items = match cache.load::<Vec<T>>(file).await {
            Ok(items) => items,
            Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
            Err(e) => {
                discord_log!(ctx, "Failed to load {file}: {e}").await;
                Vec::new()
            }
        };
        Self {
            items: Mutex::new(items),
            cache,
            file,
        }
    }

    /// Lock the records — e.g. to render a list or pre-fill an edit form.
    pub async fn items(&self) -> MutexGuard<'_, Vec<T>> {
        self.items.lock().await
    }

    /// Interpret a side-effect against the collection, persist the result, and
    /// return a confirmation. This is the hook `advance` defers its mutations
    /// to: the workflow stays declarative, all record mutation happens here.
    ///
    /// The save runs while the lock is held so disk reflects exactly the state
    /// just produced; a save failure is logged but does not undo the in-memory
    /// change (it will be re-persisted on the next mutation).
    pub async fn run(&self, ctx: &Context, effect: &Effect, input: &Input) -> Result<String> {
        let mut items = self.items.lock().await;
        let message = match effect {
            Effect::Create => {
                let item = T::create(input)?;
                let summary = item.summary();
                items.push(item);
                format!("Created {summary}")
            }
            Effect::Update { target } => {
                let item = items
                    .iter_mut()
                    .find(|item| item.label() == *target)
                    .ok_or_else(|| anyhow!("'{target}' not found"))?;
                item.update(input)?;
                format!("Updated {}", item.summary())
            }
        };
        if let Err(e) = self.cache.save(self.file, &*items).await {
            discord_log!(ctx, "Failed to persist {}: {e}", self.file).await;
        }
        Ok(message)
    }
}
