use embassy_futures::select::Either;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use lis3dh_async::{Configuration, Lis3dh as InnerLis3dh, Lis3dhI2C, SlaveAddr}; // TODO: rename this
use portable_atomic::{AtomicBool, Ordering};
use riot_rs_embassy::{arch, Spawner};
use riot_rs_sensors::{
    sensor::{
        Category, NotificationReceiver, PhysicalValue, ReadingError, ReadingResult, ThresholdKind,
    },
    PhysicalUnit, Sensor,
};

// TODO: support SPI as well
// TODO: could maybe use a OncelCell instead of an Option
pub struct Lis3dh {
    initialized: AtomicBool, // TODO: use an atomic bitset for initialized and enabled
    enabled: AtomicBool,
    // TODO: consider using MaybeUninit?
    accel: Mutex<CriticalSectionRawMutex, Option<InnerLis3dh<Lis3dhI2C<arch::i2c::I2c>>>>,
}

// TODO: need to impl Lis3dhCore?

impl Lis3dh {
    #[expect(clippy::new_without_default)]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            enabled: AtomicBool::new(false),
            accel: Mutex::new(None),
        }
    }

    pub fn init(&'static self, _spawner: Spawner, i2c: arch::i2c::I2c) {
        if !self.initialized.load(Ordering::Acquire) {
            let addr = SlaveAddr::Alternate; // FIXME
            let config = Configuration::default(); // FIXME

            // FIXME: add a timeout, blocks indefinitely if no device is connected
            // FIXME: is using block_on ok here?
            // FIXME: handle the Result
            // FIXME: this does not work because of https://github.com/embassy-rs/embassy/issues/2830
            // let init = embassy_futures::block_on(embassy_futures::select::select(
            //     InnerLis3dh::new_i2c_with_config(i2c, addr, config),
            //     Timer::after(Duration::from_secs(1)),
            // ));
            // let driver = match init {
            //     Either::First(driver) => driver.unwrap(),
            //     Either::Second(_) => panic!("timeout when initializing Lis3dh"), // FIXME
            // };

            let driver =
                embassy_futures::block_on(InnerLis3dh::new_i2c_with_config(i2c, addr, config))
                    .unwrap();

            // We use `try_lock()` instead of `lock()` to not make this function async.
            // This mutex cannot be locked at this point as it is private and can only be
            // locked when the sensor has been initialized successfully.
            let mut accel = self.accel.try_lock().unwrap();
            *accel = Some(driver);

            self.initialized.store(true, Ordering::Release);
            self.enabled.store(true, Ordering::Release);
        }
    }
}

impl Sensor for Lis3dh {
    async fn read_main(&self) -> ReadingResult<PhysicalValue> {
        if !self.enabled.load(Ordering::Acquire) {
            return Err(ReadingError::Disabled);
        }

        // TODO: maybe should check is_data_ready()?
        let data = self
            .accel
            .lock()
            .await
            .as_mut()
            .unwrap()
            .accel_norm()
            .await
            .map_err(|_| ReadingError::SensorAccess)?;

        #[allow(clippy::cast_possible_truncation)]
        // FIXME: dumb scaling, take precision into account
        Ok(PhysicalValue::new((data.z * 100.) as i32))
    }

    fn set_enabled(&self, enabled: bool) {
        if self.initialized.load(Ordering::Acquire) {
            self.enabled.store(enabled, Ordering::Release);
        }
        // TODO: return an error otherwise?
    }

    fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    fn set_threshold(&self, kind: ThresholdKind, value: PhysicalValue) {
        todo!()
    }

    fn set_threshold_enabled(&self, kind: ThresholdKind, enabled: bool) {
        todo!()
    }

    fn subscribe(&self) -> NotificationReceiver {
        todo!()
    }

    fn category(&self) -> Category {
        Category::Accelerometer
    }

    fn value_scale(&self) -> i8 {
        -2
    }

    fn unit(&self) -> PhysicalUnit {
        // FIXME: what's the actual unit?
        PhysicalUnit::AccelG
    }

    fn display_name(&self) -> Option<&'static str> {
        Some("3-axis accelerometer")
    }

    fn part_number(&self) -> &'static str {
        "LIS3DH"
    }

    fn version(&self) -> u8 {
        0
    }
}

// TODO: consider accelerometer.rs as well
// impl Accelerometer for Lis3dh {
//
// }
