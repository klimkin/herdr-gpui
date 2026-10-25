//! Opt-in native load fixture: seven streaming panes and one input pane.

use super::{EXIT_CODE, shell_ready, type_text};
use crate::{Command, HerdrWindow, NavigationTarget, RunCommand};
use anyhow::{Context as _, Result, anyhow, bail, ensure};
use gpui::{AnyWindowHandle, App, Keystroke, WindowHandle, px, size};
use herdr_client::protocol::{PaneSurfaceFrame, PaneSurfacePane};
use std::{
    collections::{HashMap, HashSet},
    fs,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

const PANES: usize = 8;
const SAMPLES: usize = 40;
const MEASUREMENT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Setup,
    Producers,
    FocusTarget,
    Measure,
    Verify,
}

struct Fixture {
    stage: Stage,
    split_expected: Option<usize>,
    pane_ids: Vec<String>,
    producer: usize,
    last_producer: Option<Instant>,
    measured_from: Option<(Instant, u64)>,
    pending_echo: Option<(Instant, String, String)>,
    pending_output: Option<(Instant, String)>,
    next_sample_at: Instant,
    samples: usize,
    last_surface_revision: Option<u64>,
    surface_changes: u64,
    content_revisions: HashMap<String, u64>,
    content_changes: HashMap<String, u64>,
    provisional_revisions: u64,
    last_probe: Instant,
    max_probe_gap: Duration,
    deadline: Instant,
}

impl Fixture {
    fn new() -> Self {
        Self {
            stage: Stage::Setup,
            split_expected: None,
            pane_ids: Vec::new(),
            producer: 1,
            last_producer: None,
            measured_from: None,
            pending_echo: None,
            pending_output: None,
            next_sample_at: Instant::now(),
            samples: 0,
            last_surface_revision: None,
            surface_changes: 0,
            content_revisions: HashMap::new(),
            content_changes: HashMap::new(),
            provisional_revisions: 0,
            last_probe: Instant::now(),
            max_probe_gap: Duration::ZERO,
            deadline: Instant::now() + Duration::from_secs(90),
        }
    }

    fn observe(&mut self, surface: &PaneSurfaceFrame) -> Result<()> {
        if self.last_surface_revision == Some(surface.surface_revision) {
            return Ok(());
        }
        surface
            .frame
            .validate()
            .context("invalid composite terminal frame")?;
        ensure!(
            surface.panes.len() == PANES,
            "pane count changed during measurement"
        );
        let ids = surface
            .panes
            .iter()
            .map(|pane| pane.pane_id.as_str())
            .collect::<HashSet<_>>();
        ensure!(ids.len() == PANES, "duplicate pane ID in surface");
        ensure!(
            self.pane_ids.iter().all(|id| ids.contains(id.as_str())),
            "pane identity changed during measurement"
        );
        let width = usize::from(surface.frame.width);
        let height = usize::from(surface.frame.height);
        for pane in &surface.panes {
            let rect = pane.inner_rect;
            ensure!(
                rect.width > 0
                    && rect.height > 0
                    && usize::from(rect.x) + usize::from(rect.width) <= width
                    && usize::from(rect.y) + usize::from(rect.height) <= height,
                "pane {} has invalid inner rectangle {rect:?}",
                pane.pane_id
            );
            if !pane.content_revision.is_multiple_of(2) {
                self.provisional_revisions += 1;
                continue;
            }
            if let Some(previous) = self.content_revisions.get(&pane.pane_id).copied() {
                ensure!(
                    pane.content_revision >= previous,
                    "pane {} content revision regressed {} -> {} across surface {} -> {} at projection {}",
                    pane.pane_id,
                    previous,
                    pane.content_revision,
                    self.last_surface_revision.unwrap_or_default(),
                    surface.surface_revision,
                    surface.projection_revision
                );
                if pane.content_revision > previous {
                    *self
                        .content_changes
                        .entry(pane.pane_id.clone())
                        .or_default() += 1;
                }
            }
            self.content_revisions
                .insert(pane.pane_id.clone(), pane.content_revision);
        }
        self.surface_changes += 1;
        self.last_surface_revision = Some(surface.surface_revision);
        Ok(())
    }
}

fn process_cpu_ticks() -> Result<u64> {
    let stat = fs::read_to_string("/proc/self/stat")?;
    let (_, fields) = stat.rsplit_once(") ").context("malformed process stat")?;
    let fields = fields.split_ascii_whitespace().collect::<Vec<_>>();
    Ok(fields
        .get(11)
        .context("missing user CPU ticks")?
        .parse::<u64>()?
        + fields
            .get(12)
            .context("missing system CPU ticks")?
            .parse::<u64>()?)
}

fn pane_line(surface: &PaneSurfaceFrame, pane: &PaneSurfacePane, text: &str, exact: bool) -> bool {
    let frame = &surface.frame;
    let width = usize::from(frame.width);
    let rect = pane.inner_rect;
    (usize::from(rect.y)..usize::from(rect.y) + usize::from(rect.height)).any(|y| {
        let start = y * width + usize::from(rect.x);
        let end = start + usize::from(rect.width);
        frame.cells.get(start..end).is_some_and(|cells| {
            let row = cells
                .iter()
                .map(|cell| cell.symbol.as_str())
                .collect::<String>();
            if exact {
                row.trim() == text
            } else {
                row.contains(text)
            }
        })
    })
}

fn pane_contains(surface: &PaneSurfaceFrame, id: &str, text: &str, exact: bool) -> bool {
    surface
        .panes
        .iter()
        .find(|pane| pane.pane_id == id)
        .is_some_and(|pane| pane_line(surface, pane, text, exact))
}

pub(super) fn start(handle: WindowHandle<HerdrWindow>, cx: &mut App) {
    let rate = std::env::var("HERDR_MULTIPANE_RATE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100);
    let probe_ms = std::env::var("HERDR_MULTIPANE_PROBE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(10);
    let script = std::env::var("HERDR_MULTIPANE_SCRIPT").unwrap_or_default();
    let timer = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let mut fixture = Fixture::new();
        let mut drawn = false;
        loop {
            timer.timer(Duration::from_millis(probe_ms)).await;
            let result = AnyWindowHandle::from(handle).update(cx, |root, window, cx| -> Result<bool> {
                let now = Instant::now();
                fixture.max_probe_gap = fixture.max_probe_gap.max(now.duration_since(fixture.last_probe));
                fixture.last_probe = now;
                ensure!(Instant::now() < fixture.deadline, "multipane fixture timed out: stage={} panes={} producers={} samples={}", fixture.stage as u8, fixture.pane_ids.len(), fixture.producer.saturating_sub(1), fixture.samples);
                let view = root.downcast::<HerdrWindow>().map_err(|_| anyhow!("unexpected window root"))?;
                if !drawn {
                    window.resize(size(px(1400.), px(800.)));
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
                match fixture.stage {
                    Stage::Setup => {
                        if surface.panes.len() < PANES {
                            if fixture.split_expected.is_some_and(|count| surface.panes.len() < count) {
                                return Ok(false);
                            }
                            fixture.split_expected = None;
                            if !view.read(cx).navigation_ready() {
                                return Ok(false);
                            }
                            let pane = surface.panes.iter().max_by_key(|pane| {
                                u32::from(pane.rect.width) * u32::from(pane.rect.height)
                            }).context("surface has no panes")?;
                            if snapshot.focused_pane_id.as_deref() != Some(&pane.pane_id) {
                                view.update(cx, |view, cx| {
                                    view.navigate(NavigationTarget::Pane(&pane.pane_id), cx);
                                });
                                return Ok(false);
                            }
                            let command = if pane.rect.width >= pane.rect.height.saturating_mul(2) {
                                Command::SplitRight
                            } else {
                                Command::SplitDown
                            };
                            window.dispatch_action(Box::new(RunCommand { command }), cx);
                            fixture.split_expected = Some(surface.panes.len() + 1);
                            return Ok(false);
                        }
                        ensure!(surface.panes.len() == PANES, "unexpected pane count {}", surface.panes.len());
                        let mut panes = surface.panes.iter().collect::<Vec<_>>();
                        panes.sort_by_key(|pane| (pane.rect.y, pane.rect.x));
                        fixture.pane_ids = panes.iter().map(|pane| pane.pane_id.clone()).collect();
                        eprintln!("GUI_MULTIPANE_LAYOUT panes={} size={}x{} ids={:?}", PANES, surface.frame.width, surface.frame.height, fixture.pane_ids);
                        fixture.stage = Stage::Producers;
                    }
                    Stage::Producers => {
                        if rate == 0 || fixture.producer == PANES {
                            fixture.stage = Stage::FocusTarget;
                            return Ok(false);
                        }
                        let id = &fixture.pane_ids[fixture.producer];
                        if !view.read(cx).navigation_ready() {
                            return Ok(false);
                        }
                        if snapshot.focused_pane_id.as_deref() != Some(id) {
                            view.update(cx, |view, cx| {
                                view.navigate(NavigationTarget::Pane(id), cx);
                            });
                            return Ok(false);
                        }
                        if !shell_ready(&surface.frame) {
                            return Ok(false);
                        }
                        let command = format!("./multipane_load.py {} {rate} &", fixture.producer);
                        type_text(&command, &view, window, cx)?;
                        ensure!(window.dispatch_keystroke(Keystroke::parse("enter")?, cx), "producer Enter was not dispatched");
                        eprintln!("GUI_MULTIPANE_PRODUCER_STARTED index={} pane={id}", fixture.producer);
                        fixture.producer += 1;
                        fixture.last_producer = Some(Instant::now());
                    }
                    Stage::FocusTarget => {
                        if !view.read(cx).navigation_ready() {
                            return Ok(false);
                        }
                        let target = &fixture.pane_ids[0];
                        if snapshot.focused_pane_id.as_deref() != Some(target) {
                            view.update(cx, |view, cx| {
                                view.navigate(NavigationTarget::Pane(target), cx);
                            });
                            return Ok(false);
                        }
                        if !shell_ready(&surface.frame)
                            || fixture.last_producer.is_some_and(|at| at.elapsed() < Duration::from_millis(500))
                        {
                            return Ok(false);
                        }
                        fixture.measured_from = Some((Instant::now(), process_cpu_ticks()?));
                        fixture.next_sample_at = Instant::now();
                        fixture.stage = Stage::Measure;
                        eprintln!("GUI_MULTIPANE_MEASURE_START rate={rate} target={target}");
                    }
                    Stage::Measure => {
                        fixture.observe(surface)?;
                        let target = &fixture.pane_ids[0];
                        ensure!(snapshot.focused_pane_id.as_deref() == Some(target), "background output moved focus away from target pane");
                        if let Some((started, text, marker)) = &fixture.pending_echo
                            && pane_contains(surface, target, text, false)
                        {
                            eprintln!("GUI_MULTIPANE_ECHO_MS {} {:.3} max_probe_gap_ms={:.3}",
                                fixture.samples, started.elapsed().as_secs_f64() * 1000., fixture.max_probe_gap.as_secs_f64() * 1000.);
                            ensure!(window.dispatch_keystroke(Keystroke::parse("enter")?, cx), "target Enter was not dispatched");
                            fixture.pending_output = Some((Instant::now(), marker.clone()));
                            fixture.pending_echo = None;
                        }
                        if let Some((started, marker)) = &fixture.pending_output
                            && pane_contains(surface, target, marker, true)
                        {
                            ensure!(
                                surface.panes.iter().filter(|pane| pane.pane_id != *target)
                                    .all(|pane| !pane_line(surface, pane, marker, false)),
                                "target output appeared in a background pane"
                            );
                            eprintln!("GUI_MULTIPANE_OUTPUT_MS {} {:.3}", fixture.samples, started.elapsed().as_secs_f64() * 1000.);
                            fixture.samples += 1;
                            fixture.pending_output = None;
                            fixture.next_sample_at = Instant::now() + Duration::from_millis(230);
                        }
                        let (started, cpu) = fixture.measured_from.context("missing CPU baseline")?;
                        ensure!(started.elapsed() < Duration::from_secs(16), "mixed workload stalled at sample {} echo={:?} output={:?}", fixture.samples, fixture.pending_echo.as_ref().map(|(_, text, _)| text), fixture.pending_output.as_ref().map(|(_, marker)| marker));
                        if fixture.samples == SAMPLES && started.elapsed() >= MEASUREMENT {
                            eprintln!("GUI_MULTIPANE_CPU ticks={} wall_ms={:.3} surface_changes={} provisional_revisions={}",
                                process_cpu_ticks()? - cpu, started.elapsed().as_secs_f64() * 1000., fixture.surface_changes, fixture.provisional_revisions);
                            fixture.stage = Stage::Verify;
                            return Ok(false);
                        }
                        if fixture.pending_echo.is_none()
                            && fixture.pending_output.is_none()
                            && fixture.samples < SAMPLES
                            && Instant::now() >= fixture.next_sample_at
                            && view.read(cx).input_ready()
                        {
                            let text = format!("T{}_{}", std::process::id(), fixture.samples);
                            let marker = format!("{text}_OK");
                            type_text(&format!("echo {text}\"_OK\""), &view, window, cx)?;
                            fixture.pending_echo = Some((Instant::now(), text, marker));
                            fixture.max_probe_gap = Duration::ZERO;
                            fixture.last_probe = Instant::now();
                        }
                    }
                    Stage::Verify => {
                        ensure!(surface.panes.len() == PANES, "pane count changed after load");
                        ensure!(snapshot.focused_pane_id.as_deref() == Some(&fixture.pane_ids[0]), "target focus changed after load");
                        if rate > 0 {
                            for index in 1..PANES {
                                let marker = format!("A{index}_DONE");
                                if !pane_contains(surface, &fixture.pane_ids[index], &marker, false) {
                                    return Ok(false);
                                }
                                ensure!(
                                    surface.panes.iter().filter(|pane| pane.pane_id != fixture.pane_ids[index])
                                        .all(|pane| !pane_line(surface, pane, &marker, false)),
                                    "producer {index} final marker appeared in another pane"
                                );
                            }
                            for index in 1..PANES {
                                let count = fs::read_to_string(format!("{script}.agent{index}.count"))?
                                    .trim().parse::<usize>()?;
                                ensure!(count >= rate * 15, "producer {index} emitted only {count} lines");
                                ensure!(fixture.content_changes.get(&fixture.pane_ids[index]).copied().unwrap_or(0) > 0,
                                    "producer pane {index} never advanced its content revision");
                                eprintln!("GUI_MULTIPANE_PRODUCER index={index} lines={count} content_changes={}", fixture.content_changes[&fixture.pane_ids[index]]);
                            }
                        }
                        eprintln!("GUI_MULTIPANE_PASS panes={PANES} producers={} samples={} target_focus_stable=true", if rate > 0 { PANES - 1 } else { 0 }, fixture.samples);
                        return Ok(true);
                    }
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
                    eprintln!("GUI_MULTIPANE_FAIL {other:?}");
                    EXIT_CODE.store(1, Ordering::SeqCst);
                    cx.update(|cx| cx.quit());
                    break;
                }
            }
        }
    })
    .detach();
}
