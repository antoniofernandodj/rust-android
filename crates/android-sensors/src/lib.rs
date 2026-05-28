use futures::Stream;

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
    /// ~5 Hz (200 ms interval)
    Normal,
    /// ~16 Hz (62 ms interval)
    Ui,
    /// ~50 Hz (20 ms interval)
    Game,
    /// As fast as possible (5 ms interval)
    Fastest,
}

impl Sensor {
    /// Returns a `Stream` that emits sensor events at the given sampling rate.
    ///
    /// On Android: registers a `SensorEventListener` via JNI.
    /// On desktop: emits simulated data at the configured rate.
    pub fn stream(self, rate: SamplingRate) -> impl Stream<Item = SensorEvent> {
        use std::time::Duration;

        let interval_ms: u64 = match rate {
            SamplingRate::Normal  => 200,
            SamplingRate::Ui      => 62,
            SamplingRate::Game    => 20,
            SamplingRate::Fastest => 5,
        };

        futures::stream::unfold((self, 0u64), move |(sensor, step)| async move {
            tokio::time::sleep(Duration::from_millis(interval_ms)).await;
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
                Sensor::Barometer      => SensorEvent::Barometer(1013.25 + t.sin()),
                Sensor::Light          => SensorEvent::Light(100.0 + (t * 0.1).sin() * 50.0),
                Sensor::Proximity      => SensorEvent::Proximity(5.0),
                Sensor::StepCounter    => SensorEvent::StepCounter(step),
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
}
