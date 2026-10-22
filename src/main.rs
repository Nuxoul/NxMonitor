use anyhow::Result;
use nxmonitor::{capture::Capture, options::Options};
use std::{thread, time::Duration};

fn main() -> Result<()> {
    let Some(options) = Options::parse()? else {
        return Ok(());
    };
    anyhow::ensure!(!options.window_only, "--window-only requires nxmonitor-hud");
    let mut capture = Capture::start(
        options.presentmon.as_ref().unwrap(),
        options.pid.unwrap(),
        options.seconds,
    )?;
    println!(
        "NxMonitor: PID {} / presented FPS / rolling 30s; no injection, no auto elevation",
        options.pid.unwrap()
    );
    loop {
        let state = capture.poll()?;
        let s = &state.snapshot;
        println!(
            "frames={} chain={} current={} fps {} ms | rolling avg={} min={} max={} low1={} low0.1={} | session avg={}{}",
            state.received,
            s.swapchain,
            number(s.current_fps),
            number(s.frame_ms),
            number(s.rolling.average_fps),
            number(s.rolling.min_fps),
            number(s.rolling.max_fps),
            number(s.rolling.low_1),
            number(s.rolling.low_01),
            number(s.session.average_fps),
            if state
                .last_frame
                .is_some_and(|t| t.elapsed() > Duration::from_secs(2))
            {
                " [STALE: no recent frames]"
            } else {
                ""
            }
        );
        if let Some(error) = state.error {
            anyhow::bail!("{error}\n{}", state.diagnostics)
        }
        if state.finished {
            if !state.diagnostics.is_empty() {
                eprintln!("{}", state.diagnostics);
            }
            anyhow::ensure!(
                state.received > 0,
                "capture completed without target frames"
            );
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}
fn number(value: Option<f64>) -> String {
    value.map_or_else(|| "—".into(), |v| format!("{v:.2}"))
}
