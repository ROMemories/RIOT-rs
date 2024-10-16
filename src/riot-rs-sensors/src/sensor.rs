//! Provides a [`Sensor`] trait abstracting over implementation details of a sensor driver.
//!
//! After triggering a measurement with [`Sensor::trigger_measurement()`], a reading can be
//! obtained using [`Sensor::wait_for_reading()`].
//! It is additionally necessary to use [`Sensor::reading_axes()`] to make sensor of the obtained
//! reading:
//!
//! - [`Sensor::wait_for_reading()`] returns a [`PhysicalValues`], a data "tuple" containing values
//!   returned by the sensor driver.
//! - [`Sensor::reading_axes()`] returns a [`ReadingAxes`] which indicates what physical value
//!   each value from that tuple corresponds to, using a [`Label`].
//!   For instance, this allows to disambiguate the values provided by a temperature & humidity
//!   sensor.
//!   Each [`ReadingAxis`] also provides information about the measurement accuracy, through
//!   [`ReadingAxis::accuracy_fn()`].
//!
//! To avoid float handling, values returned by [`Sensor::wait_for_reading()`] are integers, and a
//! fixed scaling value is provided in [`ReadingAxis`], for each [`PhysicalValue`] returned.
//! See [`PhysicalValue`] for more details.

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, ReceiveFuture},
    signal::Signal,
};
use portable_atomic::{AtomicU8, Ordering};

use crate::{interrupts::InterruptEventKind, Category, Label, PhysicalUnit};

pub use crate::{
    physical_value::{AccuracyError, PhysicalValue},
    Reading,
};

/// Represents a sensor device; implemented on sensor drivers.
///
/// See [the module level documentation](crate::sensor) for more.
pub trait Sensor: Send + Sync {
    /// Triggers a measurement.
    /// Clears the previous reading.
    ///
    /// To obtain readings from every sensor drivers this method can be called in a loop over all
    /// sensors returned by [`Registry::sensors()`](crate::registry::Registry::sensors), before
    /// obtaining the readings with [`Self::wait_for_reading()`], so that the measurements happen
    /// concurrently.
    ///
    /// # Errors
    ///
    /// Returns [`MeasurementError::NonEnabled`] if the sensor driver is not enabled.
    fn trigger_measurement(&self) -> Result<(), MeasurementError>;

    /// Waits for the reading and returns it asynchronously.
    /// Depending on the sensor device and the sensor driver, this may use a sensor interrupt or
    /// data polling.
    ///
    /// Interpretation of the reading requires data from [`Sensor::reading_axes()`] as well.
    /// See [the module level documentation](crate::sensor) for more.
    ///
    /// # Errors
    ///
    /// - Returns [`ReadingError::NonEnabled`] if the sensor driver is not enabled.
    /// - Returns [`ReadingError::SensorAccess`] if the sensor device cannot be accessed.
    fn wait_for_reading(&'static self) -> ReadingWaiter;

    /// Provides information about the reading returned by [`Sensor::wait_for_reading()`].
    #[must_use]
    fn reading_axes(&self) -> ReadingAxes;

    #[must_use]
    fn available_interrupt_events(&self) -> &[InterruptEventKind] {
        &[]
    }

    /// Sets the sensor driver mode and returns the previous state.
    /// Allows to put the sensor device to sleep if supported.
    ///
    /// # Errors
    ///
    /// Returns [`ModeSettingError::Uninitialized`] if the sensor driver is not initialized.
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
    ///
    /// For instance, "push button" and "3-axis accelerometer" are appropriate display names.
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

// TODO: move this
// TODO: rename this
/// Intended for sensor driver implementors only.
pub struct SensorSignaling {
    trigger: Signal<CriticalSectionRawMutex, ()>,
    reading_channel: Channel<CriticalSectionRawMutex, ReadingResult<PhysicalValues>, 1>,
}

impl SensorSignaling {
    #[must_use]
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

    pub async fn signal_reading(&self, reading: PhysicalValues) {
        self.reading_channel.send(Ok(reading)).await;
    }

    pub async fn signal_reading_err(&self, reading_err: ReadingError) {
        self.reading_channel.send(Err(reading_err)).await;
    }

    pub fn wait_for_reading(&'static self) -> ReadingWaiter {
        ReadingWaiter::Waiter {
            waiter: self.reading_channel.receive(),
        }
    }
}

/// Future returned by [`Sensor::wait_for_reading()`].
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[pin_project::pin_project(project = ReadingWaiterProj)]
pub enum ReadingWaiter {
    #[doc(hidden)]
    Waiter {
        #[pin]
        waiter: ReceiveFuture<'static, CriticalSectionRawMutex, ReadingResult<PhysicalValues>, 1>,
    },
    #[doc(hidden)]
    Err(ReadingError),
    #[doc(hidden)]
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
/// Intended for sensor driver implementors only.
#[derive(Default)]
pub struct StateAtomic {
    state: AtomicU8,
}

impl StateAtomic {
    /// Creates a new [`StateAtomic`].
    #[must_use]
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
    #[expect(clippy::missing_panics_doc, reason = "cannot actually panic")]
    pub fn get(&self) -> State {
        // NOTE(no-panic): cast cannot fail because the integer value always comes from *us*
        // internally casting `State`.
        State::try_from(self.state.load(Ordering::Acquire)).unwrap()
    }

    /// Sets the current state.
    pub fn set(&self, state: State) {
        self.state.store(state as u8, Ordering::Release);
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

/// Provides metadata about a [`PhysicalValue`].
#[derive(Debug, Copy, Clone)]
pub struct ReadingAxis {
    label: Label,
    scaling: i8,
    unit: PhysicalUnit,
    accuracy: AccuracyFn,
}

impl ReadingAxis {
    /// Creates a new [`ReadingAxis`].
    ///
    /// Intended for sensor driver implementors only.
    #[must_use]
    pub fn new(label: Label, scaling: i8, unit: PhysicalUnit, accuracy: AccuracyFn) -> Self {
        Self {
            label,
            scaling,
            unit,
            accuracy,
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

    /// Returns a function allowing to obtain the accuracy error of a recently obtained
    /// [`PhysicalValue`].
    ///
    /// # Note
    ///
    /// As the accuracy may depend on the sensor driver configuration, that accuracy function
    /// should only be used for one [`PhysicalValue`] instance, and it is necessary to obtain an
    /// up-to-date function through an up-to-date [`ReadingAxis`].
    #[must_use]
    pub fn accuracy_fn(&self) -> AccuracyFn {
        self.accuracy
    }
}

/// Function allowing to obtain the accuracy error of a [`PhysicalValue`], returned by
/// [`ReadingAxis::accuracy_fn()`].
pub type AccuracyFn = fn(PhysicalValue) -> AccuracyError;

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

/// A specialized [`Result`] type for [`Reading`] operations.
pub type ReadingResult<R> = Result<R, ReadingError>;

#[cfg(test)]
mod tests {
    use super::*;

    // Assert that the Sensor trait is object-safe
    static _SENSOR_REFS: &[&dyn Sensor] = &[];
}
