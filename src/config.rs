//! Versioned settings stored independently of UI types or runtime process IDs.

use crate::target::{BlacklistRule, is_windows_absolute};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionMode {
    Automatic,
    Manual,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Settings {
    pub version: u32,
    pub mode: SelectionMode,
    /// A path hint only. A new runtime process identity must be explicitly locked after restore.
    pub manual_path: Option<String>,
    pub blacklist: Vec<BlacklistRule>,
    pub english: bool,
    pub graphy: bool,
    pub presentmon_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            mode: SelectionMode::Automatic,
            manual_path: None,
            blacklist: Vec::new(),
            english: false,
            graphy: false,
            presentmon_path: "tools\\PresentMon-2.6.0-x64.exe".to_owned(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            bail!("unsupported settings version: {}", self.version);
        }
        if self.presentmon_path.trim().is_empty() || self.presentmon_path.len() > 32768 {
            bail!("PresentMon path must be nonempty and at most 32768 bytes");
        }
        if self
            .manual_path
            .as_deref()
            .is_some_and(|path| !is_windows_absolute(path))
        {
            bail!("manual target path must be absolute");
        }
        if self.blacklist.len() > 256 {
            bail!("blacklist exceeds 256 rules");
        }
        for rule in &self.blacklist {
            rule.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn app_default() -> Result<Self> {
        let root = std::env::var_os("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .context("LOCALAPPDATA is not set")?;
        Ok(Self::new(
            PathBuf::from(root).join("NxMonitor").join("settings.json"),
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Settings> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Settings::default());
            }
            Err(error) => {
                return Err(error).with_context(|| format!("opening {}", self.path.display()));
            }
        };
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
        let settings: Settings = serde_json::from_slice(bytes)
            .with_context(|| format!("parsing {}", self.path.display()))?;
        settings
            .validate()
            .with_context(|| format!("validating {}", self.path.display()))?;
        Ok(settings)
    }

    pub fn save(&self, settings: &Settings) -> Result<()> {
        // Validate before creating directories or touching the existing settings file.
        settings.validate()?;
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .context("settings path has no parent directory")?;
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)
            .with_context(|| format!("creating temporary settings in {}", parent.display()))?;
        serde_json::to_writer_pretty(temp.as_file_mut(), settings)
            .context("serializing settings")?;
        temp.write_all(b"\n").context("writing settings newline")?;
        temp.as_file_mut().sync_all().context("syncing settings")?;
        // Same-directory persist atomically replaces an existing file, including on Windows.
        temp.persist(&self.path)
            .map_err(|error| error.error)
            .with_context(|| format!("replacing {}", self.path.display()))?;
        Ok(())
    }
}
