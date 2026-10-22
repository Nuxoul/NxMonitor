//! Runtime process identity, target selection and Windows blacklist rules.

use crate::config::SelectionMode;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// Start time is part of the identity so a recycled PID cannot inherit a manual lock.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProcessIdentity {
    pub pid: u32,
    pub started_at: u64,
    pub name: String,
    pub path: Option<String>,
}

impl ProcessIdentity {
    pub fn new(pid: u32, started_at: u64, name: impl Into<String>, path: Option<String>) -> Self {
        Self {
            pid,
            started_at,
            name: name.into(),
            path,
        }
    }

    pub fn id(&self) -> (u32, u64) {
        (self.pid, self.started_at)
    }
}

/// Literal, case-insensitive Windows filename or full executable path; no wildcards.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlacklistRule {
    FileName(String),
    FullPath(String),
}

impl BlacklistRule {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::FileName(name) if name.trim().is_empty() || name.contains(['/', '\\']) => {
                bail!("blacklist filename must be a nonempty basename")
            }
            Self::FullPath(path) if !is_windows_absolute(path) => {
                bail!("blacklist full path must be an absolute Windows path")
            }
            _ => Ok(()),
        }
    }

    pub fn matches(&self, process: &ProcessIdentity) -> bool {
        if self.validate().is_err() {
            return false;
        }
        match self {
            Self::FileName(name) => {
                name.eq_ignore_ascii_case(&process.name)
                    || process.path.as_deref().is_some_and(|path| {
                        path.rsplit(['/', '\\'])
                            .next()
                            .is_some_and(|basename| name.eq_ignore_ascii_case(basename))
                    })
            }
            Self::FullPath(path) => process.path.as_deref().is_some_and(|actual| {
                is_windows_absolute(actual) && normalize_path(path) == normalize_path(actual)
            }),
        }
    }
}
/// Accept drive-rooted or UNC server/share paths, independent of the host OS.
pub(crate) fn is_windows_absolute(path: &str) -> bool {
    let normalized = path.replace('/', "\\");
    let bytes = normalized.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\' {
        return true;
    }
    if let Some(unc) = normalized.strip_prefix("\\\\") {
        let mut parts = unc.split('\\');
        return parts.next().is_some_and(|server| !server.is_empty())
            && parts.next().is_some_and(|share| !share.is_empty());
    }
    false
}

fn normalize_path(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

/// Manual identity is runtime-only; constructing a Manual selection never relocks a saved path.
#[derive(Clone, Debug)]
pub struct Selection {
    mode: SelectionMode,
    manual: Option<ProcessIdentity>,
}

impl Selection {
    pub fn new(mode: SelectionMode) -> Self {
        Self { mode, manual: None }
    }

    pub fn set_mode(&mut self, mode: SelectionMode) {
        self.mode = mode;
        if mode == SelectionMode::Automatic {
            self.manual = None;
        }
    }

    pub fn lock(&mut self, process: ProcessIdentity) {
        self.manual = Some(process);
        self.mode = SelectionMode::Manual;
    }

    pub fn resolve(
        &self,
        foreground: Option<&ProcessIdentity>,
        running: &[ProcessIdentity],
        blacklist: &[BlacklistRule],
        own_pid: u32,
    ) -> Option<ProcessIdentity> {
        let eligible = |p: &ProcessIdentity| p.pid != own_pid && p.pid != 0 && p.pid != 4;
        match self.mode {
            SelectionMode::Automatic => {
                let foreground = foreground.filter(|p| {
                    eligible(p)
                        && p.path.as_deref().is_some_and(is_windows_absolute)
                        && !blacklist.iter().any(|rule| rule.matches(p))
                })?;
                running
                    .iter()
                    .find(|p| {
                        p.id() == foreground.id()
                            && p.path.as_deref().is_some_and(is_windows_absolute)
                    })
                    .cloned()
            }
            SelectionMode::Manual => {
                let manual = self.manual.as_ref().filter(|p| eligible(p))?;
                running
                    .iter()
                    .find(|p| eligible(p) && p.id() == manual.id())
                    .cloned()
            }
        }
    }
}
