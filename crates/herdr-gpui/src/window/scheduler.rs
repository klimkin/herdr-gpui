use super::HerdrWindow;
use gpui::{Context, Task, Window};
use std::{
    future::Future,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tick {
    Connection,
    Maintenance,
}

async fn next_tick(
    events: &async_channel::Receiver<()>,
    maintenance: impl Future<Output = ()>,
) -> Option<Tick> {
    // Poll maintenance first so a sustained producer cannot starve its deadline.
    futures_lite::future::or(
        async {
            maintenance.await;
            Some(Tick::Maintenance)
        },
        async { events.recv().await.ok().map(|()| Tick::Connection) },
    )
    .await
}

impl HerdrWindow {
    pub(super) fn spawn_scheduler(
        window: &mut Window,
        cx: &mut Context<Self>,
        events: async_channel::Receiver<()>,
        wake: crate::connection::ConnectionWake,
        interval: Duration,
    ) -> Task<()> {
        let timer = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut deadline = Instant::now() + interval;
            loop {
                let Some(tick) = next_tick(
                    &events,
                    timer.timer(deadline.saturating_duration_since(Instant::now())),
                )
                .await
                else {
                    break;
                };
                if this
                    .update_in(cx, |this, window, cx| {
                        let retry = wake.take_deferred_inbox_read();
                        this.tick(window, cx, tick == Tick::Connection || retry);
                    })
                    .is_err()
                {
                    break;
                }
                if tick == Tick::Maintenance {
                    deadline = Instant::now() + interval;
                }
                // An already queued arrival or busy inbox must yield to other UI work.
                futures_lite::future::yield_now().await;
            }
        })
    }
}

#[cfg(test)]
mod tests;
