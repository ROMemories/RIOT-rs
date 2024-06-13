use embassy_futures::select::Either;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, mutex::Mutex};
use embassy_time::{Duration, Timer};
use lis3dh_async::{Configuration, DataRate, Lis3dh as InnerLis3dh, Lis3dhI2C};
use portable_atomic::{AtomicU8, Ordering};
use riot_rs_embassy::{arch, Spawner};
use riot_rs_sensors::{
    sensor::{
        AccuracyError, Mode as SensorMode, PhysicalValue, PhysicalValues, ReadingError,
        ReadingInfo, ReadingInfos, ReadingResult, State,
    },
    Category, Label, PhysicalUnit, Sensor,
};

pub use crate::lis3dh_spi::Lis3dhSpi;
pub use lis3dh_async::{Mode, SlaveAddr as Address};

// FIXME: what's the best way to instantiate sensor driver configuration?
#[derive(Debug)]
#[non_exhaustive]
pub struct Config {
    pub address: Address,
    pub mode: Mode,
    pub datarate: DataRate,
    pub enable_x_axis: bool,
    pub enable_y_axis: bool,
    pub enable_z_axis: bool,
    pub block_data_update: bool, // TODO: do we need to expose this?
    pub enable_temperature: bool,
}

impl Default for Config {
    fn default() -> Self {
        let config = Configuration::default();
        Self {
            address: Address::Alternate,
            mode: config.mode,
            datarate: config.datarate,
            enable_x_axis: config.enable_x_axis,
            enable_y_axis: config.enable_y_axis,
            enable_z_axis: config.enable_z_axis,
            block_data_update: config.block_data_update,
            enable_temperature: config.enable_temperature,
        }
    }
}

riot_rs_embassy::define_peripherals!(Peripherals {});

// TODO: could maybe use a OnceCell instead of an Option
pub struct Lis3dhI2c {
    state: AtomicU8,
    label: Option<&'static str>,
    // TODO: consider using MaybeUninit?
    accel: Mutex<CriticalSectionRawMutex, Option<InnerLis3dh<Lis3dhI2C<arch::i2c::I2cDevice>>>>,
}

impl Lis3dhI2c {
    #[expect(clippy::new_without_default)]
    #[must_use]
    pub const fn new(label: Option<&'static str>) -> Self {
        Self {
            state: AtomicU8::new(State::Uninitialized as u8),
            label,
            accel: Mutex::new(None),
        }
    }

    pub fn init(
        &'static self,
        _spawner: Spawner,
        peripherals: Peripherals,
        i2c: arch::i2c::I2cDevice,
        config: Config,
    ) {
        if self.state.load(Ordering::Acquire) == State::Uninitialized as u8 {
            // TODO: can this be made shorter?
            let mut lis3dh_config = Configuration::default();
            lis3dh_config.mode = config.mode;
            lis3dh_config.datarate = config.datarate;
            lis3dh_config.enable_x_axis = config.enable_x_axis;
            lis3dh_config.enable_y_axis = config.enable_y_axis;
            lis3dh_config.enable_z_axis = config.enable_z_axis;
            lis3dh_config.block_data_update = config.block_data_update;
            lis3dh_config.enable_temperature = config.enable_temperature;

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

            let driver = embassy_futures::block_on(InnerLis3dh::new_i2c_with_config(
                i2c,
                config.address,
                lis3dh_config,
            ))
            .unwrap();

            // We use `try_lock()` instead of `lock()` to not make this function async.
            // This mutex cannot be locked at this point as it is private and can only be
            // locked when the sensor has been initialized successfully.
            let mut accel = self.accel.try_lock().unwrap();
            *accel = Some(driver);

            self.state.store(State::Enabled as u8, Ordering::Release);
        }
    }
}

impl Sensor for Lis3dhI2c {
    #[allow(refining_impl_trait)]
    async fn measure(&self) -> ReadingResult<PhysicalValues> {
        if self.state.load(Ordering::Acquire) != State::Enabled as u8 {
            return Err(ReadingError::NonEnabled);
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
        // FIXME: specify the measurement error
        let x = PhysicalValue::new((data.x * 100.) as i32, AccuracyError::Unknown);
        let y = PhysicalValue::new((data.y * 100.) as i32, AccuracyError::Unknown);
        let z = PhysicalValue::new((data.z * 100.) as i32, AccuracyError::Unknown);

        Ok(PhysicalValues::V3([x, y, z]))
    }

    fn set_mode(&self, mode: SensorMode) {
        if self.state.load(Ordering::Acquire) != State::Uninitialized as u8 {
            let state = State::from(mode);
            self.state.store(state as u8, Ordering::Release);
        }
        // TODO: return an error otherwise?
    }

    fn state(&self) -> State {
        let state = self.state.load(Ordering::Acquire);
        // NOTE(no-panic): the state atomic is only written from a State
        State::try_from(state).unwrap()
    }

    fn categories(&self) -> &'static [Category] {
        &[Category::Accelerometer]
    }

    fn reading_infos(&self) -> ReadingInfos {
        ReadingInfos::V3([
            ReadingInfo::new(Label::X, -2, PhysicalUnit::AccelG),
            ReadingInfo::new(Label::Y, -2, PhysicalUnit::AccelG),
            ReadingInfo::new(Label::Z, -2, PhysicalUnit::AccelG),
        ])
    }

    fn label(&self) -> Option<&'static str> {
        self.label
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
