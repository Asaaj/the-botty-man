use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::Result;
use serde::de::DeserializeOwned;
use serenity::all::Context;

use crate::log::discord_log;

static CACHE_LOC: OnceLock<PathBuf> = OnceLock::new();

pub struct Cache;
impl Cache {
    pub fn install(loc: impl AsRef<Path>) -> Result<(), PathBuf> {
        CACHE_LOC.set(to_safe_path(loc))
    }

    pub fn loc() -> &'static Path {
        CACHE_LOC.get().unwrap()
    }

    /// Creates the corresponding cache entry on disk
    pub async fn entry(ctx: &Context, name: impl Into<String>) -> CacheContainer {
        let name = name.into();
        let loc = Self::loc().join(to_safe_path(&name));
        CacheContainer::new(ctx, name, loc).await
    }
}

#[derive(Debug)]
pub struct CacheContainer {
    pub name: String,
    pub loc: PathBuf,
}
impl CacheContainer {
    async fn new(ctx: &Context, name: String, loc: PathBuf) -> Self {
        create_dirs(ctx, &loc).await;
        Self { name, loc }
    }

    pub async fn load<T: DeserializeOwned>(&self, item: impl AsRef<Path>) -> std::io::Result<T> {
        let full_path = self.loc.join(item.as_ref());
        let bytes = tokio::fs::read(full_path).await?;
        let res = serde_json::from_slice(&bytes)?;
        Ok(res)
    }
}

async fn create_dirs(ctx: &Context, path: &Path) {
    if let Err(e) = std::fs::create_dir_all(path) {
        discord_log!(
            ctx,
            "Failed to create cache directory {}: {e}",
            path.display()
        )
        .await;
    }
}

fn to_safe_path(s: impl AsRef<Path>) -> PathBuf {
    let s = s.as_ref().to_str().unwrap();
    let s: String = s
        .chars()
        .map(|c| if c == ' ' { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    s.into()
}
