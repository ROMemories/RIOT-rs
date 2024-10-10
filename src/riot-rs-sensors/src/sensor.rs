//! Provides a [`Sensor`] trait abstracting over implementation details of a sensor driver.
//!
//! To obtain a reading and make sense of the result, two methods are required:
//! [`Sensor::measure()`] and [`Sensor::reading_axes()`]:
//!
//! - [`Sensor::measure()`] returns a [`PhysicalValues`], a data "tuple" containing values returned
//! by the sensor driver.
//! - The [`ReadingAxes`] type, returned by [`Sensor::reading_axes()`], tells what physical value
//! each value from that tuple corresponds to, using [`Label`].
//! For instance, this allows to disambiguate the values provided by a temperature & humidity
//! sensor.
//! The [`ReadingAxes`] are fixed for a given sensor driver, allowing to display information about
//! the physical values a sensor can measure without triggering a measurement.
//!
//! To avoid float handling, values returned by [`Sensor::measure()`] are integers, and a
//! fixed scaling value is provided in [`ReadingAxis`], for each [`PhysicalValue`] returned.
//! See [`PhysicalValue`] for more details.

use core::{
    any::Any,
    future::{self, Future},
    pin::Pin,
    task::{Context, Poll},
};

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, ReceiveFuture},
    mutex::Mutex,
    signal::Signal,
};
use portable_atomic::{AtomicU8, Ordering};

use crate::{interrupts::InterruptEventKind, Category, Label, PhysicalUnit};

pub use crate::{
    physical_value::{AccuracyError, PhysicalValue},
    Reading,
};

/// Represents a sensor device; implemented on sensor drivers.
/// See [the module level documentation](crate::sensor) for more.
pub trait Sensor: Any + Send + Sync {
    /// Triggers a measurement.
    fn trigger_measurement(&self) -> Result<(), MeasurementError>;

    /// Waits for the reading and returns it asynchronously.
    /// Depending on the sensor device and the sensor driver, this may use a sensor interrupt or
    /// data polling.
    ///
    /// Interpretation of the reading requires data from [`Sensor::reading_axes()`] as well.
    /// See [the module level documentation](crate::sensor) for more.
    fn wait_for_reading(&'static self) -> ReadingWaiter;

    // FIXME: rename this
    /// Provides information about the reading returned by [`Sensor::measure()`].
    #[must_use]
    fn reading_axes(&self) -> ReadingAxes;

    #[must_use]
    fn available_interrupt_events(&self) -> &[InterruptEventKind] {
        &[]
    }

    /// Sets the sensor driver mode and returns the previous state.
    /// Allows to put the sensor device to sleep if supported.
    fn set_mode(&self, mode: Mode) -> Result<State, ModeSettingError>;

    /// Returns the current sensor driver state.
    #[must_use]
    fn state(&self) -> State;

    /// Returns the categories the sensor device is part of.
    #[must_use]
    fn categories(&self) -> &'static [Category];

    /// String label of the sensor driver *instance*.
    ///
    /// This is intended to be configured when setting up the sensor driver instance.
    /// For instance, in the case of a temperature sensor, this allows to specify whether this
    /// specific sensor device is placed indoor or outdoor.
    #[must_use]
    fn label(&self) -> Option<&'static str>;

    /// Returns a human-readable name of the sensor driver.
    // TODO: i18n?
    #[must_use]
    fn display_name(&self) -> Option<&'static str>;

    /// Returns the hardware sensor device part number.
    ///
    /// Returns `None` when the sensor device does not have a part number.
    #[must_use]
    fn part_number(&self) -> Option<&'static str>;

    /// Returns the sensor driver version number.
    #[must_use]
    fn version(&self) -> u8;
}

// TODO: move this and make it clear it is only useful for sensor driver implementors
// TODO: rename this
pub struct SensorSignaling {
    trigger: Signal<CriticalSectionRawMutex, ()>,
    reading_channel: Channel<CriticalSectionRawMutex, ReadingResult<PhysicalValues>, 1>,
}

impl SensorSignaling {
    pub const fn new() -> Self {
        Self {
            trigger: Signal::new(),
            reading_channel: Channel::new(),
        }
    }

    pub fn trigger_measurement(&self) {
        // Remove the possibly lingering reading.
        self.reading_channel.clear();

        self.trigger.signal(());
    }

    pub async fn wait_for_trigger(&self) {
        self.trigger.wait().await;
    }

    pub async fn signal_reading(&self, reading: Result<PhysicalValues, ReadingError>) {
        self.reading_channel.send(reading).await;
    }

    pub fn wait_for_reading(&'static self) -> ReadingWaiter {
        ReadingWaiter::Waiter {
            waiter: self.reading_channel.receive(),
        }
    }
}

#[must_use = "futures do nothing unless you `.await` or poll them"]
#[pin_project::pin_project(project = ReadingWaiterProj)]
pub enum ReadingWaiter {
    Waiter {
        #[pin]
        waiter: ReceiveFuture<'static, CriticalSectionRawMutex, ReadingResult<PhysicalValues>, 1>,
    },
    Err(ReadingError),
    Resolved,
}

impl Future for ReadingWaiter {
    type Output = ReadingResult<PhysicalValues>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().project();
        match this {
            ReadingWaiterProj::Waiter { waiter } => waiter.poll(cx),
            ReadingWaiterProj::Err(err) => {
                // Replace the error with a dummy error value, crafted from thin air, and mark the
                // future as resolved, so that we do not take this dummy value into account later.
                // This avoids requiring `Clone` on `ReadingError`.
                let err = core::mem::replace(err, ReadingError::NonEnabled);
                *self = ReadingWaiter::Resolved;

                Poll::Ready(Err(err))
            }
            ReadingWaiterProj::Resolved => unreachable!(),
        }
    }
}

impl dyn Sensor {
    pub fn downcast_ref<S: Sensor>(&self) -> Option<&S> {
        (self as &dyn Any).downcast_ref::<S>()
    }
}

/// Mode of a sensor driver.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Mode {
    /// The sensor driver is disabled.
    Disabled,
    /// The sensor driver is enabled.
    Enabled,
    /// The sensor driver is sleeping.
    /// The sensor device may be in a low-power mode.
    Sleeping,
}

pub enum ModeSettingError {
    Uninitialized,
}

/// State of a sensor driver.
#[derive(Copy, Clone, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    /// The sensor driver is uninitialized.
    /// It has not been initialized yet, or initialization could not succeed.
    #[default]
    Uninitialized = 0,
    /// The sensor driver is disabled.
    Disabled = 1,
    /// The sensor driver is enabled.
    Enabled = 2,
    /// The sensor driver is sleeping.
    Sleeping = 3,
}

impl From<Mode> for State {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Disabled => Self::Disabled,
            Mode::Enabled => Self::Enabled,
            Mode::Sleeping => Self::Sleeping,
        }
    }
}

impl TryFrom<u8> for State {
    type Error = TryFromIntError;

    fn try_from(int: u8) -> Result<Self, Self::Error> {
        match int {
            0 => Ok(State::Uninitialized),
            1 => Ok(State::Disabled),
            2 => Ok(State::Enabled),
            3 => Ok(State::Sleeping),
            _ => Err(TryFromIntError),
        }
    }
}

#[derive(Debug)]
pub struct TryFromIntError;

/// A helper to store [`State`] as an atomic.
///
/// Intended for sensor driver implementors.
#[derive(Default)]
pub struct StateAtomic {
    state: AtomicU8,
}

impl StateAtomic {
    /// Creates a new [`StateAtomic`].
    pub const fn new(state: State) -> Self {
        // Make sure `State` fits into a `u8`.
        const {
            assert!(core::mem::size_of::<State>() == core::mem::size_of::<u8>());
        }

        Self {
            state: AtomicU8::new(state as u8),
        }
    }

    /// Returns the current state.
    pub fn get(&self) -> State {
        // NOTE(no-panic): cast cannot fail because the integer value always comes from *us*
        // internally casting `State`.
        State::try_from(self.state.load(Ordering::Acquire)).unwrap()
    }

    /// Sets the current state.
    pub fn set(&self, state: State) {
        self.state.store(state as u8, Ordering::Release)
    }

    /// Sets the current mode.
    pub fn set_mode(&self, mode: Mode) -> State {
        let new_state = State::from(mode);

        // Set the mode if the current state is not uninitialized
        let res = self
            .state
            .fetch_update(Ordering::Release, Ordering::Acquire, |s| {
                if s == State::Uninitialized as u8 {
                    None
                } else {
                    Some(new_state as u8)
                }
            });

        if res.is_err() {
            State::Uninitialized
        } else {
            new_state
        }
    }
}

riot_rs_macros::define_count_adjusted_enums!();

/// Provides meta-data about a [`PhysicalValue`].
#[derive(Debug, Copy, Clone, serde::Serialize)]
pub struct ReadingAxis {
    label: Label,
    scaling: i8,
    unit: PhysicalUnit,
}

impl ReadingAxis {
    /// Creates a new [`ReadingAxis`].
    #[must_use]
    pub fn new(label: Label, scaling: i8, unit: PhysicalUnit) -> Self {
        Self {
            label,
            scaling,
            unit,
        }
    }

    /// Returns the [`Label`] for this axis.
    #[must_use]
    pub fn label(&self) -> Label {
        self.label
    }

    /// Returns the scaling for this axis.
    #[must_use]
    pub fn scaling(&self) -> i8 {
        self.scaling
    }

    /// Returns the unit for this axis.
    #[must_use]
    pub fn unit(&self) -> PhysicalUnit {
        self.unit
    }
}

/// Represents errors happening when *triggering* a sensor measurement.
#[derive(Debug)]
pub enum MeasurementError {
    /// The sensor driver is not enabled (e.g., it may be disabled or sleeping).
    NonEnabled,
}

impl core::fmt::Display for MeasurementError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NonEnabled => write!(f, "sensor driver is not enabled"),
        }
    }
}

impl core::error::Error for MeasurementError {}

/// Represents errors happening when accessing a sensor reading.
// TODO: is it more useful to indicate the error nature or whether it is temporary or permanent?
#[derive(Debug)]
pub enum ReadingError {
    /// The sensor driver is not enabled (e.g., it may be disabled or sleeping).
    NonEnabled,
    /// Cannot access the sensor device (e.g., because of a bus error).
    SensorAccess,
}

impl core::fmt::Display for ReadingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NonEnabled => write!(f, "sensor driver is not enabled"),
            Self::SensorAccess => write!(f, "sensor device could not be accessed"),
        }
    }
}

impl core::error::Error for ReadingError {}

pub type ReadingResult<R> = Result<R, ReadingError>;

#[cfg(test)]
mod tests {
    use super::*;

    // Assert that the Sensor trait is object-safe
    static _SENSOR_REFS: &[&dyn Sensor] = &[];
}
