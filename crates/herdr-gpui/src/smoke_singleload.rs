//! Opt-in native fixture for typing in a pane that is also streaming output.

use super::{EXIT_CODE, shell_ready, type_text};
use crate::HerdrWindow;
use anyhow::{Context as _, Result, anyhow, bail, ensure};
use gpui::{AnyWindowHandle, App, Keystroke, WindowHandle};
use herdr_client::protocol::FrameData;
use std::{
    fs,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

const SAMPLES: usize = 5;
const WINDOW: Duration = Duration::from_secs(5);

fn process_cpu_ticks() -> Result<u64> {
    let stat = fs::read_to_string("/proc/self/stat")?;
    let (_, fields) = stat.rsplit_once(") ").context("malformed process stat")?;
    let fields = fields.split_ascii_whitespace().collect::<Vec<_>>();
    Ok(fields
        .get(11)
        .context("missing user ticks")?
        .parse::<u64>()?
        + fields
            .get(12)
            .context("missing system ticks")?
            .parse::<u64>()?)
}

fn contains(frame: &FrameData, marker: &str) -> bool {
    frame.width > 0
        && frame.cells.chunks(usize::from(frame.width)).any(|row| {
            row.iter()
                .map(|cell| cell.symbol.as_str())
                .collect::<String>()
                .contains(marker)
        })
}

pub(super) fn start(handle: WindowHandle<HerdrWindow>, cx: &mut App) {
    let rate = std::env::var("HERDR_SINGLELOAD_RATE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(2000);
    let probe_ms = std::env::var("HERDR_SINGLELOAD_PROBE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1);
    let script = std::env::var("HERDR_SINGLELOAD_SCRIPT").unwrap_or_default();
    let timer = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let mut drawn = false;
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut producer_started: Option<Instant> = None;
        let mut measured_from: Option<(Instant, u64)> = None;
        let mut pending: Option<(Instant, String)> = None;
        let mut next_send = Instant::now();
        let mut samples = 0;
        let mut misses = 0;
        let mut last_surface_revision = None;
        let mut surface_changes = 0_u64;
        loop {
            timer.timer(Duration::from_millis(probe_ms)).await;
            let result = AnyWindowHandle::from(handle).update(cx, |root, window, cx| -> Result<bool> {
                ensure!(Instant::now() < deadline, "single-pane load fixture timed out at sample {samples}");
                let view = root.downcast::<HerdrWindow>().map_err(|_| anyhow!("unexpected window root"))?;
                if !drawn {
                    window.focus(&view.read(cx).focus.clone(), cx);
                    window.refresh();
                    window.draw(cx).clear(cx);
                    drawn = true;
                }
                let live = view.read(cx).live.clone();
                if let Some(error) = view.read(cx).local_error.as_ref().or(live.error.as_ref()) {
                    bail!("connection failed: {error}");
                }
                let (Some(snapshot), Some(surface)) = (&live.snapshot, &live.surface) else {
                    return Ok(false);
                };
                ensure!(surface.panes.len() == 1, "single-pane load changed pane count");
                ensure!(snapshot.focused_pane_id.as_deref() == Some(surface.panes[0].pane_id.as_str()), "single-pane load lost focus");
                if last_surface_revision != Some(surface.surface_revision) {
                    surface.frame.validate().context("invalid terminal frame")?;
                    last_surface_revision = Some(surface.surface_revision);
                    surface_changes += 1;
                }
                if measured_from.is_none() {
                    if !view.read(cx).input_ready() || (producer_started.is_none() && !shell_ready(&surface.frame)) {
                        return Ok(false);
                    }
                    if rate > 0 && producer_started.is_none() {
                        type_text(&format!("./singleload.py {rate} &"), &view, window, cx)?;
                        ensure!(window.dispatch_keystroke(Keystroke::parse("enter")?, cx), "producer Enter was not dispatched");
                        producer_started = Some(Instant::now());
                        return Ok(false);
                    }
                    if producer_started.is_some_and(|at| at.elapsed() < Duration::from_millis(500)) {
                        return Ok(false);
                    }
                    measured_from = Some((Instant::now(), process_cpu_ticks()?));
                    next_send = Instant::now();
                    eprintln!("GUI_SINGLELOAD_START rate={rate} probe_ms={probe_ms}");
                }
                if let Some((started, marker)) = &pending {
                    let elapsed = started.elapsed();
                    if contains(&surface.frame, marker) {
                        eprintln!("GUI_SINGLELOAD_ECHO_MS {samples} {:.3}", elapsed.as_secs_f64() * 1000.);
                    } else if elapsed >= Duration::from_millis(300) {
                        eprintln!("GUI_SINGLELOAD_ECHO_MISS {samples}");
                        misses += 1;
                    } else {
                        return Ok(false);
                    }
                    ensure!(window.dispatch_keystroke(Keystroke::parse("enter")?, cx), "sample Enter was not dispatched");
                    pending = None;
                    samples += 1;
                    next_send = Instant::now() + Duration::from_millis(700);
                }
                let (started, cpu) = measured_from.context("missing CPU baseline")?;
                if samples == SAMPLES && started.elapsed() >= WINDOW {
                    eprintln!("GUI_SINGLELOAD_CPU ticks={} wall_ms={:.3} surface_changes={surface_changes}",
                        process_cpu_ticks()? - cpu, started.elapsed().as_secs_f64() * 1000.);
                    if rate > 0 {
                        let count = fs::read_to_string(format!("{script}.count"))?.trim().parse::<usize>()?;
                        ensure!(count >= rate * 4, "producer emitted only {count} lines");
                        eprintln!("GUI_SINGLELOAD_LINES {count}");
                    }
                    eprintln!("GUI_SINGLELOAD_PASS rate={rate} samples={samples} misses={misses}");
                    return Ok(true);
                } else if samples < SAMPLES && pending.is_none() && Instant::now() >= next_send && view.read(cx).input_ready() {
                    let marker = format!("E{}_{}", std::process::id(), samples);
                    type_text(&format!("echo {marker}"), &view, window, cx)?;
                    pending = Some((Instant::now(), marker));
                }
                Ok(false)
            });
            match result {
                Ok(Ok(true)) => {
                    EXIT_CODE.store(0, Ordering::SeqCst);
                    cx.update(|cx| cx.quit());
                    break;
                }
                Ok(Ok(false)) => {}
                other => {
                    eprintln!("GUI_SINGLELOAD_FAIL {other:?}");
                    EXIT_CODE.store(1, Ordering::SeqCst);
                    cx.update(|cx| cx.quit());
                    break;
                }
            }
        }
    }).detach();
}
