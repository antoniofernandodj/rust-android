use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Priority {
    Min,
    Low,
    Default,
    High,
    Max,
}

#[derive(Debug, Clone)]
pub struct Action {
    pub id: String,
    pub label: String,
}

impl Action {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// A notification channel groups notifications for the user to manage.
pub struct Channel {
    id: String,
    name: String,
    importance: Priority,
}

impl Channel {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            importance: Priority::Default,
        }
    }

    pub fn importance(mut self, p: Priority) -> Self {
        self.importance = p;
        self
    }

    /// Register this channel with the system.
    ///
    /// On Android: calls `NotificationManager.createNotificationChannel` via JNI.
    /// On desktop: no-op.
    pub fn register(self) {
        let _ = self;
    }
}

/// Builder for a system notification.
pub struct Notification {
    channel_id: String,
    title: Option<String>,
    body: Option<String>,
    progress: Option<(u32, u32)>,
    ongoing: bool,
    actions: Vec<Action>,
}

impl Notification {
    pub fn new(channel_id: impl Into<String>) -> Self {
        Self {
            channel_id: channel_id.into(),
            title: None,
            body: None,
            progress: None,
            ongoing: false,
            actions: Vec::new(),
        }
    }

    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    pub fn body(mut self, b: impl Into<String>) -> Self {
        self.body = Some(b.into());
        self
    }

    pub fn progress(mut self, current: u32, total: u32) -> Self {
        self.progress = Some((current, total));
        self
    }

    pub fn ongoing(mut self, v: bool) -> Self {
        self.ongoing = v;
        self
    }

    pub fn action(mut self, a: Action) -> Self {
        self.actions.push(a);
        self
    }

    /// Register an async handler for notification action button taps.
    ///
    /// On Android: wired via a `BroadcastReceiver` that triggers the future.
    /// On desktop: handler is accepted but never called (stub).
    pub fn on_action<F, Fut>(self, _handler: F) -> Self
    where
        F: Fn(String) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self
    }

    /// Display the notification with the given numeric ID.
    ///
    /// On Android: uses `NotificationManagerCompat` via JNI.
    /// On desktop: no-op (stub).
    pub fn show(self, id: u32) {
        let _ = id;
    }

    /// Cancel (dismiss) the notification with the given ID.
    ///
    /// On Android: calls `NotificationManager.cancel` via JNI.
    /// On desktop: no-op (stub).
    pub fn cancel(id: u32) {
        let _ = id;
    }
}
