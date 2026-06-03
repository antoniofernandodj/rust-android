#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Quaternion {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone)]
pub enum SensorEvent {
    Accelerometer(Vec3),
    Gyroscope(Vec3),
    Magnetometer(Vec3),
    Barometer(f32),
    Light(f32),
    Proximity(f32),
    StepCounter(u64),
    RotationVector(Quaternion),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sensor {
    Accelerometer,
    Gyroscope,
    Magnetometer,
    Barometer,
    Light,
    Proximity,
    StepCounter,
    RotationVector,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SamplingRate {
    /// ~5 Hz (200 ms)
    Normal,
    /// ~16 Hz (62 ms)
    Ui,
    /// ~50 Hz (20 ms)
    Game,
    /// Máxima velocidade (5 ms)
    Fastest,
}

impl SamplingRate {
    #[allow(dead_code)]
    fn interval_ms(self) -> u64 {
        match self {
            SamplingRate::Normal => 200,
            SamplingRate::Ui => 62,
            SamplingRate::Game => 20,
            SamplingRate::Fastest => 5,
        }
    }

    #[cfg(target_os = "android")]
    #[allow(dead_code)]
    fn interval_us(self) -> i32 {
        (self.interval_ms() * 1000) as i32
    }
}

/// Stream de eventos de sensor.
///
/// Requer feature `stream` (ativa tokio + futures).
#[cfg(feature = "stream")]
impl Sensor {
    pub fn stream(self, rate: SamplingRate) -> impl futures::Stream<Item = SensorEvent> {
        #[cfg(target_os = "android")]
        {
            android_impl::sensor_stream(self, rate)
        }
        #[cfg(not(target_os = "android"))]
        {
            desktop_stub_stream(self, rate)
        }
    }
}

// ── Desktop stub stream ───────────────────────────────────────────────────────

#[cfg(all(feature = "stream", not(target_os = "android")))]
fn desktop_stub_stream(
    sensor: Sensor,
    rate: SamplingRate,
) -> impl futures::Stream<Item = SensorEvent> {
    use std::time::Duration;
    futures::stream::unfold((sensor, 0u64), move |(sensor, step)| async move {
        tokio::time::sleep(Duration::from_millis(rate.interval_ms())).await;
        let t = step as f32 * 0.1;
        let event = match sensor {
            Sensor::Accelerometer => SensorEvent::Accelerometer(Vec3 {
                x: t.sin() * 0.5,
                y: t.cos() * 0.5,
                z: 9.8,
            }),
            Sensor::Gyroscope => SensorEvent::Gyroscope(Vec3 {
                x: (t * 0.3).sin() * 0.1,
                y: (t * 0.5).cos() * 0.1,
                z: 0.0,
            }),
            Sensor::Magnetometer => SensorEvent::Magnetometer(Vec3 {
                x: 20.0,
                y: -10.0,
                z: 40.0,
            }),
            Sensor::Barometer => SensorEvent::Barometer(1013.25 + t.sin()),
            Sensor::Light => SensorEvent::Light(100.0 + (t * 0.1).sin() * 50.0),
            Sensor::Proximity => SensorEvent::Proximity(5.0),
            Sensor::StepCounter => SensorEvent::StepCounter(step),
            Sensor::RotationVector => SensorEvent::RotationVector(Quaternion {
                w: 1.0,
                x: 0.0,
                y: 0.0,
                z: 0.0,
            }),
        };
        Some((event, (sensor, step + 1)))
    })
}

// ── Android NDK implementation ────────────────────────────────────────────────
//
// Uses ASensorManager (native C API via ndk-sys) instead of JNI to avoid the
// overhead of creating a Java SensorEventListener.
//
// Each stream spawns a background OS thread that owns the ALooper + event queue.
// Events are sent to a tokio mpsc channel and exposed as a futures::Stream.
// When the Stream (receiver) is dropped, the next failed send causes the thread
// to clean up and exit.

#[cfg(all(feature = "stream", target_os = "android"))]
mod android_impl {
    use super::*;
    use ndk_sys::{
        ALooper_prepare, ALooper_pollAll,
        ASensorEventQueue_disableSensor, ASensorEventQueue_enableSensor,
        ASensorEventQueue_getEvents, ASensorEventQueue_setEventRate,
        ASensorManager_createEventQueue, ASensorManager_destroyEventQueue,
        ASensorManager_getDefaultSensor, ASensorManager_getInstance,
        ASensorManager_getInstanceForPackage,
        ALOOPER_PREPARE_ALLOW_NON_CALLBACKS,
        ASENSOR_TYPE_ACCELEROMETER, ASENSOR_TYPE_GYROSCOPE, ASENSOR_TYPE_MAGNETIC_FIELD,
        ASENSOR_TYPE_PRESSURE, ASENSOR_TYPE_LIGHT, ASENSOR_TYPE_PROXIMITY,
        ASENSOR_TYPE_STEP_COUNTER, ASENSOR_TYPE_ROTATION_VECTOR,
    };
    use std::ffi::CString;

    fn sensor_type(s: Sensor) -> i32 {
        match s {
            Sensor::Accelerometer => ASENSOR_TYPE_ACCELEROMETER,
            Sensor::Gyroscope => ASENSOR_TYPE_GYROSCOPE,
            Sensor::Magnetometer => ASENSOR_TYPE_MAGNETIC_FIELD,
            Sensor::Barometer => ASENSOR_TYPE_PRESSURE,
            Sensor::Light => ASENSOR_TYPE_LIGHT,
            Sensor::Proximity => ASENSOR_TYPE_PROXIMITY,
            Sensor::StepCounter => ASENSOR_TYPE_STEP_COUNTER,
            Sensor::RotationVector => ASENSOR_TYPE_ROTATION_VECTOR,
        }
    }

    unsafe fn vec3_from_sensor_event(event: &ndk_sys::ASensorEvent) -> Vec3 {
        let v = event.__bindgen_anon_1.__bindgen_anon_1.vector.__bindgen_anon_1.v;
        Vec3 { x: v[0], y: v[1], z: v[2] }
    }

    unsafe fn scalar_from_sensor_event(event: &ndk_sys::ASensorEvent) -> f32 {
        event.__bindgen_anon_1.__bindgen_anon_1.data[0]
    }

    unsafe fn convert_event(
        event: &ndk_sys::ASensorEvent,
        sensor: Sensor,
    ) -> Option<SensorEvent> {
        Some(match sensor {
            Sensor::Accelerometer => SensorEvent::Accelerometer(vec3_from_sensor_event(event)),
            Sensor::Gyroscope => SensorEvent::Gyroscope(vec3_from_sensor_event(event)),
            Sensor::Magnetometer => SensorEvent::Magnetometer(vec3_from_sensor_event(event)),
            Sensor::Barometer => SensorEvent::Barometer(scalar_from_sensor_event(event)),
            Sensor::Light => SensorEvent::Light(scalar_from_sensor_event(event)),
            Sensor::Proximity => SensorEvent::Proximity(scalar_from_sensor_event(event)),
            Sensor::StepCounter => {
                let steps = event.__bindgen_anon_1.u64_.step_counter;
                SensorEvent::StepCounter(steps)
            }
            Sensor::RotationVector => {
                let data = event.__bindgen_anon_1.__bindgen_anon_1.data;
                SensorEvent::RotationVector(Quaternion {
                    x: data[0],
                    y: data[1],
                    z: data[2],
                    w: data[3],
                })
            }
        })
    }

    pub fn sensor_stream(
        sensor: Sensor,
        rate: SamplingRate,
    ) -> impl futures::Stream<Item = SensorEvent> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<SensorEvent>();
        let asensor_type = sensor_type(sensor);
        let interval_us = rate.interval_us();

        // ASensorManager and ALooper must be accessed from a dedicated OS thread.
        std::thread::spawn(move || {
            // Get the package name for the new getInstanceForPackage API
            let pkg_name = get_package_name().unwrap_or_else(|| "com.example.rustandroid".into());
            let pkg_cstr = CString::new(pkg_name).unwrap_or_default();

            unsafe {
                // getInstanceForPackage (API 26+); fall back to deprecated getInstance.
                let mgr = {
                    let m = ASensorManager_getInstanceForPackage(pkg_cstr.as_ptr());
                    if m.is_null() { ASensorManager_getInstance() } else { m }
                };
                if mgr.is_null() { return; }

                let sensor_ptr = ASensorManager_getDefaultSensor(mgr, asensor_type);
                if sensor_ptr.is_null() { return; }

                // Prepare a looper for this thread (required by event queue)
                let looper =
                    ALooper_prepare(ALOOPER_PREPARE_ALLOW_NON_CALLBACKS as std::os::raw::c_int);
                if looper.is_null() { return; }

                let queue =
                    ASensorManager_createEventQueue(mgr, looper, 1, None, std::ptr::null_mut());
                if queue.is_null() { return; }

                ASensorEventQueue_enableSensor(queue, sensor_ptr);
                ASensorEventQueue_setEventRate(queue, sensor_ptr, interval_us);

                let mut buf = [std::mem::zeroed::<ndk_sys::ASensorEvent>(); 32];
                let mut errors: u32 = 0;

                loop {
                    ALooper_pollAll(
                        100,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    );

                    let n = ASensorEventQueue_getEvents(queue, buf.as_mut_ptr(), buf.len());

                    if n < 0 {
                        // Transient errors are normal (e.g. queue not yet ready).
                        // Only give up after many consecutive failures (~5 s).
                        errors += 1;
                        if errors > 50 { break; }
                        continue;
                    }
                    errors = 0;

                    for event in &buf[..n as usize] {
                        if let Some(e) = convert_event(event, sensor) {
                            if tx.send(e).is_err() {
                                // Receiver dropped — clean up and exit.
                                ASensorEventQueue_disableSensor(queue, sensor_ptr);
                                ASensorManager_destroyEventQueue(mgr, queue);
                                return;
                            }
                        }
                    }
                }

                ASensorEventQueue_disableSensor(queue, sensor_ptr);
                ASensorManager_destroyEventQueue(mgr, queue);
            }
        });

        futures::stream::unfold(rx, |mut rx| async move {
            let item = rx.recv().await?;
            Some((item, rx))
        })
    }

    fn get_package_name() -> Option<String> {
        use jni::{objects::JObject, JavaVM};
        let ctx = ndk_context::android_context();
        let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
        let mut env = vm.attach_current_thread().ok()?;
        let context = unsafe { JObject::from_raw(ctx.context().cast()) };
        let pkg = env
            .call_method(&context, "getPackageName", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        let jstr = jni::objects::JString::from(pkg);
        env.get_string(&jstr).ok().map(|s| s.into())
    }
}
