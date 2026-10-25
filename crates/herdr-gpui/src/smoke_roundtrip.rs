use super::{EXIT_CODE, has_output, shell_ready, type_text};
use crate::HerdrWindow;
use anyhow::{Result, anyhow, bail};
use gpui::{AnyWindowHandle, App, Keystroke, WindowHandle};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

pub(super) fn start(handle: WindowHandle<HerdrWindow>, cx: &mut App) {
    let timer = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let mut sample = 0;
        let mut pending: Option<(Instant, String)> = None;
        let mut drawn = false;
        let mut last_progress = Instant::now();
        loop {
            timer.timer(Duration::from_millis(1)).await;
            let result = AnyWindowHandle::from(handle).update(cx, |root, window, cx| -> Result<bool> {
                let view = root.downcast::<HerdrWindow>().map_err(|_| anyhow!("unexpected window root"))?;
                if !drawn {
                    cx.hide();
                    window.blur(cx);
                    window.focus(&view.read(cx).focus.clone(), cx);
                    window.refresh();
                    window.draw(cx).clear(cx);
                    drawn = true;
                }
                let live = view.read(cx).live.clone();
                if let Some(error) = view.read(cx).local_error.as_ref().or(live.error.as_ref()) {
                    bail!("connection failed: {error}");
                }
                if let Some((started, marker)) = &pending
                    && live.surface.as_ref().is_some_and(|surface| has_output(&surface.frame, marker))
                {
                    eprintln!("GUI_ROUNDTRIP_SAMPLE_MS {sample} {:.3}", started.elapsed().as_secs_f64() * 1000.);
                    sample += 1;
                    pending = None;
                    last_progress = Instant::now();
                    if sample == 40 {
                        return Ok(true);
                    }
                }
                if pending.is_none()
                    && live.status.is_connected()
                    && view.read(cx).input_ready()
                    && live.surface.as_ref().is_some_and(|surface| shell_ready(&surface.frame))
                {
                    let marker = format!("HERDR_RT_{}_{}_OK", std::process::id(), sample);
                    let command = format!("echo HERDR_RT_{}_{}\"_OK\"", std::process::id(), sample);
                    type_text(&command, &view, window, cx)?;
                    if !window.dispatch_keystroke(Keystroke::parse("enter")?, cx) {
                        bail!("Enter was not dispatched");
                    }
                    pending = Some((Instant::now(), marker));
                    last_progress = Instant::now();
                }
                if last_progress.elapsed() > Duration::from_secs(15) {
                    bail!("roundtrip timed out at sample {sample}; pending={pending:?} surface={:?}", live.surface.as_ref().map(|s| (s.surface_revision, s.frame.width, s.frame.height)));
                }
                Ok(false)
            });
            match result {
                Ok(Ok(true)) => {
                    eprintln!("GUI_ROUNDTRIP_PASS samples={sample}");
                    EXIT_CODE.store(0, Ordering::SeqCst);
                    cx.update(|cx| cx.quit());
                    break;
                }
                Ok(Ok(false)) => {}
                other => {
                    eprintln!("GUI_ROUNDTRIP_FAIL {other:?}");
                    EXIT_CODE.store(1, Ordering::SeqCst);
                    cx.update(|cx| cx.quit());
                    break;
                }
            }
        }
    })
    .detach();
}
