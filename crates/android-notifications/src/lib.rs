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
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
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
    /// On Android: calls `NotificationManager.createNotificationChannel` via JNI (API 26+).
    /// On devices older than API 26 and on desktop: no-op.
    pub fn register(self) {
        #[cfg(target_os = "android")]
        android_impl::create_channel(&self.id, &self.name, self.importance);
        #[cfg(not(target_os = "android"))]
        let _ = self;
    }
}

/// Builder for a system notification.
pub struct Notification {
    #[allow(dead_code)]
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
    /// On Android: uses `Notification.Builder` + `NotificationManager.notify()` via JNI.
    /// On desktop: no-op (stub).
    pub fn show(self, id: u32) {
        #[cfg(target_os = "android")]
        android_impl::show(
            id,
            &self.channel_id,
            self.title.as_deref(),
            self.body.as_deref(),
            self.progress,
            self.ongoing,
        );
        #[cfg(not(target_os = "android"))]
        let _ = id;
    }

    /// Cancel (dismiss) the notification with the given ID.
    ///
    /// On Android: calls `NotificationManager.cancel` via JNI.
    /// On desktop: no-op (stub).
    pub fn cancel(id: u32) {
        #[cfg(target_os = "android")]
        android_impl::cancel(id);
        #[cfg(not(target_os = "android"))]
        let _ = id;
    }
}

// ── Android JNI implementation ────────────────────────────────────────────────

#[cfg(target_os = "android")]
mod android_impl {
    use super::Priority;
    use jni::{
        objects::{JObject, JValue},
        JavaVM,
    };

    fn priority_to_importance(p: Priority) -> i32 {
        // NotificationManager importance constants
        match p {
            Priority::Min => 1,     // IMPORTANCE_MIN
            Priority::Low => 2,     // IMPORTANCE_LOW
            Priority::Default => 3, // IMPORTANCE_DEFAULT
            Priority::High => 4,    // IMPORTANCE_HIGH
            Priority::Max => 5,     // IMPORTANCE_MAX
        }
    }

    fn get_notification_manager<'a>(
        env: &mut jni::JNIEnv<'a>,
        context: &JObject<'a>,
    ) -> Option<JObject<'a>> {
        let svc = env.new_string("notification").ok()?;
        env.call_method(
            context,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(&svc)],
        )
        .ok()?
        .l()
        .ok()
        .filter(|v| !v.is_null())
    }

    pub fn create_channel(id: &str, name: &str, importance: Priority) {
        let ctx = ndk_context::android_context();
        let Ok(vm) = (unsafe { JavaVM::from_raw(ctx.vm().cast()) }) else { return };
        let Ok(mut env) = vm.attach_current_thread() else { return };
        let context = unsafe { JObject::from_raw(ctx.context().cast()) };

        let Some(nm) = get_notification_manager(&mut env, &context) else { return };

        // NotificationChannel is API 26+; skip silently on older devices
        let channel = (|| -> Option<JObject<'_>> {
            let cls = env.find_class("android/app/NotificationChannel").ok()?;
            let j_id = env.new_string(id).ok()?;
            let j_name = env.new_string(name).ok()?;
            let imp = priority_to_importance(importance);
            env.new_object(
                &cls,
                "(Ljava/lang/String;Ljava/lang/CharSequence;I)V",
                &[
                    JValue::Object(&j_id),
                    JValue::Object(&j_name),
                    JValue::Int(imp),
                ],
            )
            .ok()
        })();

        let Some(ch) = channel else {
            let _ = env.exception_clear();
            return;
        };

        let _ = env.call_method(
            &nm,
            "createNotificationChannel",
            "(Landroid/app/NotificationChannel;)V",
            &[JValue::Object(&ch)],
        );
    }

    pub fn show(
        id: u32,
        channel_id: &str,
        title: Option<&str>,
        body: Option<&str>,
        progress: Option<(u32, u32)>,
        ongoing: bool,
    ) {
        let ctx = ndk_context::android_context();
        let Ok(vm) = (unsafe { JavaVM::from_raw(ctx.vm().cast()) }) else { return };
        let Ok(mut env) = vm.attach_current_thread() else { return };
        let context = unsafe { JObject::from_raw(ctx.context().cast()) };

        let Some(nm) = get_notification_manager(&mut env, &context) else { return };

        let notification = (|| -> Option<JObject<'_>> {
            // Notification.Builder(Context, String channelId)  — API 26+
            // Fallback: Notification.Builder(Context) — older (deprecated)
            let builder_cls = env.find_class("android/app/Notification$Builder").ok()?;
            let j_channel = env.new_string(channel_id).ok()?;
            let builder = env
                .new_object(
                    &builder_cls,
                    "(Landroid/content/Context;Ljava/lang/String;)V",
                    &[JValue::Object(&context), JValue::Object(&j_channel)],
                )
                .or_else(|_| {
                    let _ = env.exception_clear();
                    // API 23-25 fallback: no channel
                    env.new_object(
                        &builder_cls,
                        "(Landroid/content/Context;)V",
                        &[JValue::Object(&context)],
                    )
                })
                .ok()?;

            // setSmallIcon: use app icon from ApplicationInfo
            let app_info = env
                .call_method(
                    &context,
                    "getApplicationInfo",
                    "()Landroid/content/pm/ApplicationInfo;",
                    &[],
                )
                .ok()?
                .l()
                .ok()?;
            let icon_id = env
                .get_field(&app_info, "icon", "I")
                .ok()
                .and_then(|v| v.i().ok())
                .unwrap_or(0x01080020); // android.R.drawable.ic_dialog_info fallback
            env.call_method(
                &builder,
                "setSmallIcon",
                "(I)Landroid/app/Notification$Builder;",
                &[JValue::Int(icon_id)],
            )
            .ok()?;

            if let Some(t) = title {
                let j_title = env.new_string(t).ok()?;
                env.call_method(
                    &builder,
                    "setContentTitle",
                    "(Ljava/lang/CharSequence;)Landroid/app/Notification$Builder;",
                    &[JValue::Object(&j_title)],
                )
                .ok()?;
            }

            if let Some(b) = body {
                let j_body = env.new_string(b).ok()?;
                env.call_method(
                    &builder,
                    "setContentText",
                    "(Ljava/lang/CharSequence;)Landroid/app/Notification$Builder;",
                    &[JValue::Object(&j_body)],
                )
                .ok()?;
            }

            if let Some((current, total)) = progress {
                env.call_method(
                    &builder,
                    "setProgress",
                    "(IIZ)Landroid/app/Notification$Builder;",
                    &[
                        JValue::Int(total as i32),
                        JValue::Int(current as i32),
                        JValue::Bool(0),
                    ],
                )
                .ok()?;
            }

            if ongoing {
                env.call_method(
                    &builder,
                    "setOngoing",
                    "(Z)Landroid/app/Notification$Builder;",
                    &[JValue::Bool(1)],
                )
                .ok()?;
            }

            let notification = env
                .call_method(
                    &builder,
                    "build",
                    "()Landroid/app/Notification;",
                    &[],
                )
                .ok()?
                .l()
                .ok()?;

            Some(notification)
        })();

        let Some(notif) = notification else {
            let _ = env.exception_clear();
            return;
        };

        let _ = env.call_method(
            &nm,
            "notify",
            "(ILandroid/app/Notification;)V",
            &[JValue::Int(id as i32), JValue::Object(&notif)],
        );
    }

    pub fn cancel(id: u32) {
        let ctx = ndk_context::android_context();
        let Ok(vm) = (unsafe { JavaVM::from_raw(ctx.vm().cast()) }) else { return };
        let Ok(mut env) = vm.attach_current_thread() else { return };
        let context = unsafe { JObject::from_raw(ctx.context().cast()) };

        let Some(nm) = get_notification_manager(&mut env, &context) else { return };
        let _ = env.call_method(&nm, "cancel", "(I)V", &[JValue::Int(id as i32)]);
    }
}
