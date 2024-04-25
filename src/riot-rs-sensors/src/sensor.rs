//! Provides a [`Sensor`] trait abstracting over implementation details of a sensor.

use core::{any::Any, future::Future};

// TODO: use a zero-copy channel instead?
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};

/// Represents a device providing sensor readings.
// TODO: introduce a trait currently deferring to Any
pub trait Sensor: Any + Send + Sync {
    // FIXME: return an enum instead?
    /// Returns the main sensor reading.
    fn read_main(&self) -> impl Future<Output = ReadingResult<PhysicalValue>>
    where
        Self: Sized;

    /// Returns a sensor reading.
    // fn read(&self) -> impl Future<Output = ReadingResult<impl Reading>>
    // where
    //     Self: Sized;

    /// Enables or disables the sensor driver.
    fn set_enabled(&self, enabled: bool);

    /// Returns whether the sensor driver is enabled.
    #[must_use]
    fn enabled(&self) -> bool;

    // TODO: support some hysteresis
    fn set_threshold(&self, kind: ThresholdKind, value: PhysicalValue);

    // TODO: merge this with set_threshold?
    fn set_threshold_enabled(&self, kind: ThresholdKind, enabled: bool);

    #[must_use]
    fn subscribe(&self) -> NotificationReceiver;

    #[must_use]
    fn category(&self) -> Category;

    /// The base-10 exponent used for all readings returned by the sensor.
    ///
    /// The actual physical value is [`value()`](PhysicalValue::value) ×
    /// 10^[`value_scale()`](Sensor::value_scale).
    /// For instance, in the case of a temperature sensor, if [`value()`](PhysicalValue::value)
    /// returns `2225` and [`value_scale()`](Sensor::value_scale) returns `-2`, it means that the
    /// temperature measured and returned by the hardware sensor is `22.25` (the sensor accuracy
    /// and precision must additionally be taken into account).
    ///
    /// This is required to avoid handling floats.
    // FIXME: how to handle sensors measuring different physical values?
    // TODO: rename this?
    #[must_use]
    fn value_scale(&self) -> i8;

    /// Returns the unit of measurement in which readings are returned.
    #[must_use]
    fn unit(&self) -> PhysicalUnit;

    /// Returns a human-readable name of the sensor.
    // TODO: i18n?
    #[must_use]
    fn display_name(&self) -> Option<&'static str>;

    /// Returns the hardware sensor part number.
    #[must_use]
    fn part_number(&self) -> &'static str;

    /// Returns the sensor driver version number.
    #[must_use]
    fn version(&self) -> u8;
}

pub trait Reading: core::fmt::Debug {
    fn value(&self) -> PhysicalValue;

    fn values(&self) -> impl ExactSizeIterator<Item = PhysicalValue> {
        [self.value()].into_iter()
    }
}

/// Represents a value obtained from a sensor.
// TODO: add a timestamp?
// TODO: add measurement error here (how to define it?)
#[derive(Debug, Copy, Clone)]
#[derive(serde::Serialize)]
pub struct PhysicalValue {
    value: i32,
}

impl PhysicalValue {
    /// Creates a new value.
    #[must_use]
    pub const fn new(value: i32) -> Self {
        Self { value }
    }

    /// Returns the value.
    #[must_use]
    pub fn value(&self) -> i32 {
        self.value
    }
}

/// Represents a unit of measurement.
// Built upon https://doc.riot-os.org/phydat_8h_source.html
// and https://bthome.io/format/#sensor-data
// and https://www.rfc-editor.org/rfc/rfc8798.html
#[derive(Debug, Copy, Clone)]
#[derive(serde::Serialize)]
#[non_exhaustive]
pub enum PhysicalUnit {
    /// Acceleration *g*.
    AccelG,
    /// Logic boolean.
    Bool,
    /// Degree Celsius.
    Celsius,
    // TODO: add other units
}

impl core::fmt::Display for PhysicalUnit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AccelG => write!(f, "g"),
            Self::Bool => write!(f, ""),
            Self::Celsius => write!(f, "°C"), // The Unicode Standard v15 recommends using U+00B0 + U+0043.
        }
    }
}

// Built upon https://doc.riot-os.org/group__drivers__saul.html#ga8f2dfec7e99562dbe5d785467bb71bbb
// FIXME: rename this to class?
#[derive(Debug)]
#[derive(serde::Serialize)]
pub enum Category {
    Temperature,
    PushButton
}

/// A notification provided by a sensor driver.
// TODO: should we pass the value as well? that may be difficult because of the required generics
#[derive(Debug, PartialEq, Eq)]
#[derive(serde::Serialize)]
#[non_exhaustive]
pub enum Notification {
    ReadingAvailable,
    Threshold(ThresholdKind),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[derive(serde::Serialize)]
#[non_exhaustive]
pub enum ThresholdKind {
    Lower,
    Higher,
}

// TODO: tune the channel size
pub type NotificationReceiver<'a> = Receiver<'a, CriticalSectionRawMutex, Notification, 1>;

/// Represents errors happening when accessing a sensor reading.
// TODO: is it more useful to indicate the error nature or whether it is temporary or permanent?
#[derive(Debug)]
pub enum ReadingError {
    /// The sensor is disabled.
    Disabled,
    /// Cannot access the sensor (e.g., because of a bus error).
    SensorAccess,
}

impl core::fmt::Display for ReadingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // FIXME: update this
        write!(f, "error when accessing a sensor reading")
    }
}

impl core::error::Error for ReadingError {}

pub type ReadingResult<R> = Result<R, ReadingError>;

/// Returns the result of calling [`Sensor::read()`] on the sensor concrete type.
///
/// Downcasts the provided sensor (which must be implementing the [`Sensor`] trait) to its concrete
/// type, and calls the async, non-dispatchable [`Sensor::read()`] method on it.
/// This is required to call [`Sensor::read()`] on a `dyn Sensor` trait object because
/// [`Sensor::read()`] is non-dispatchable and can therefore only be called on a concrete type.
///
/// This macro needs to be provided with the sensor and with the list of existing sensor concrete
/// types.
///
/// # Panics
///
/// Panics if the concrete type of the sensor was not present in the list of types provided.
// Should not be used by users directly, users should use the `riot_rs::read_sensor!()` proc-macro
// instead.
#[macro_export]
macro_rules! _await_read_sensor_main {
    ($sensor:ident, $first_sensor_type:path, $($sensor_type:path),* $(,)?) => {
        {
            // As sensor methods are non-dispatchable, we have to downcast
            if let Some($sensor) = ($sensor as &dyn core::any::Any)
                .downcast_ref::<$first_sensor_type>(
            ) {
                ($sensor.read_main().await, $sensor.unit(), $sensor.display_name())
            }
            $(
            else if let Some($sensor) = ($sensor as &dyn core::any::Any)
                .downcast_ref::<$sensor_type>(
            ) {
                ($sensor.read_main().await, $sensor.unit(), $sensor.display_name())
            }
            )*
            else {
                unreachable!();
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    // Assert that the Sensor trait is object-safe
    static _SENSOR_REFS: &[&dyn Sensor] = &[];
}
