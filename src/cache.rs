use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;

#[derive(Debug)]
pub struct Cache {
    path: PathBuf,
    cache_duration: Duration,
}

impl Cache {
    fn cache_path() -> PathBuf {
        PathBuf::from(".cache")
    }

    pub fn new(path: &str, cache_duration: Duration) -> Self {
        let path = Self::cache_path().join(path);
        Self {
            path,
            cache_duration,
        }
    }

    #[instrument]
    fn ensure_cache_folder() -> anyhow::Result<()> {
        let path = Self::cache_path();
        if !path.exists() {
            info!("creating cache folder…");
            fs::create_dir(&path).context("Failed to create cache folder")?;
        }
        Ok(())
    }

    #[instrument]
    pub fn needs_update(&self) -> bool {
        let metadata = match self.path.metadata() {
            Ok(metadata) => metadata,
            Err(_) => return true,
        };

        let modified = match metadata.modified() {
            Ok(modified) => modified,
            Err(_) => return true,
        };

        let elapsed = match modified.elapsed() {
            Ok(elapsed) => elapsed,
            Err(_) => return false,
        };

        elapsed > self.cache_duration
    }

    pub fn read(&self) -> anyhow::Result<String> {
        Self::ensure_cache_folder()?;
        Ok(fs::read_to_string(&self.path)?)
    }

    pub fn save(&self, content: &str) -> anyhow::Result<()> {
        Self::ensure_cache_folder()?;
        Ok(fs::write(&self.path, content)?)
    }
}
