use anyhow::{Context, Result, bail, ensure};
use std::path::PathBuf;

pub const HELP: &str = "NxMonitor — technical validation / 技术验证\n\n  --presentmon <path> --pid <external PID> [--seconds 60]\n  --lang zh|en           HUD language (default zh)\n  --style modern|graphy  HUD preset (default modern)\n  --window-only         Verify HUD without a collector (no fake data)\n  --smoke-seconds <n>    Automatically close HUD after n seconds\n\nCapture duration: 1..600 seconds. Presented FPS, not displayed FPS.\nHUD global hotkeys: Ctrl+Alt+F6 edit, Ctrl+Alt+F7 quit; fallback F8/F9 or F10/F11. The active pair is shown in the HUD.\n";

#[derive(Clone)]
#[non_exhaustive]
pub struct Options {
    pub presentmon: Option<PathBuf>,
    pub pid: Option<u32>,
    pub seconds: u32,
    pub english: bool,
    pub graphy: bool,
    pub window_only: bool,
    pub smoke_seconds: Option<u32>,
}
impl Options {
    pub fn parse() -> Result<Option<Self>> {
        let mut args = std::env::args().skip(1);
        let mut options = Self {
            presentmon: None,
            pid: None,
            seconds: 60,
            english: false,
            graphy: false,
            window_only: false,
            smoke_seconds: None,
        };
        while let Some(arg) = args.next() {
            if arg == "--help" || arg == "-h" {
                println!("{HELP}");
                return Ok(None);
            }
            if arg == "--window-only" {
                options.window_only = true;
                continue;
            }
            let value = args
                .next()
                .with_context(|| format!("missing value for {arg}"))?;
            match arg.as_str() {
                "--presentmon" => options.presentmon = Some(value.into()),
                "--pid" => options.pid = Some(value.parse().context("invalid PID")?),
                "--seconds" => options.seconds = value.parse().context("invalid duration")?,
                "--lang" => {
                    ensure!(value == "en" || value == "zh", "language must be zh or en");
                    options.english = value == "en";
                }
                "--style" => {
                    ensure!(
                        value == "modern" || value == "graphy",
                        "style must be modern or graphy"
                    );
                    options.graphy = value == "graphy";
                }
                "--smoke-seconds" => {
                    options.smoke_seconds = Some(value.parse().context("invalid smoke duration")?)
                }
                _ => bail!("unknown argument: {arg}\n{HELP}"),
            }
        }
        ensure!(
            (1..=600).contains(&options.seconds),
            "seconds must be 1..600"
        );
        ensure!(
            options.smoke_seconds.is_none_or(|n| (1..=600).contains(&n)),
            "smoke duration must be 1..600"
        );
        ensure!(
            options.window_only || (options.pid.is_some() && options.presentmon.is_some()),
            "supply --presentmon and --pid, or use --window-only\n{HELP}"
        );
        Ok(Some(options))
    }
}
