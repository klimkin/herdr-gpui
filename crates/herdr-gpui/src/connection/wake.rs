//! A window wake carries no state. Connection inboxes retain the latest data.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub(crate) struct ConnectionWake {
    sender: async_channel::Sender<()>,
    retry: Arc<AtomicBool>,
}

impl ConnectionWake {
    pub(crate) fn channel() -> (Self, async_channel::Receiver<()>) {
        let (sender, receiver) = async_channel::bounded(1);
        (
            Self {
                sender,
                retry: Arc::default(),
            },
            receiver,
        )
    }

    pub(crate) fn notify(&self) {
        // Full already schedules a read; closed means the window is gone.
        let _ = self.sender.try_send(());
    }

    pub(crate) fn defer_inbox_read(&self) {
        // A consumed wake must survive contention without spinning the UI task.
        self.retry.store(true, Ordering::Release);
    }

    pub(crate) fn take_deferred_inbox_read(&self) -> bool {
        self.retry.swap(false, Ordering::AcqRel)
    }
}
