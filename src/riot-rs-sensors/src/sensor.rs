//! Provides a [`Sensor`] trait abstracting over implementation details of a sensor.

use core::{any::Any, future::Future};

// TODO: use a zero-copy channel instead?
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};

use crate::{Category, Label, PhysicalUnit};

/// Represents a device providing sensor readings.
// TODO: introduce a trait currently deferring to Any
pub trait Sensor: Any + Send + Sync {
    // FIXME: clarify the semantics: should this always return new data (in which rename this to `measure`)?
    // TODO: add a link to an explanation of the setup file
    /// Returns the sensor reading.
    ///
    /// As this method is non-dispatchable, the
    /// [`riot_rs::await_read_sensor!()`](riot_rs_macros::await_read_sensor!) macro must be used
    /// instead of calling this method directly, this macro requires a  properly configured setup
    /// file.
    fn read(&self) -> impl Future<Output = ReadingResult<impl Reading>>
    where
        Self: Sized;

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
    fn categories(&self) -> &'static [Category];

    // FIXME: update this doc comment
    /// The base-10 exponent used for all readings returned by the sensor.
    // TODO: rename this?
    #[must_use]
    fn value_scales(&self) -> ValueScales;

    /// Returns the unit of measurement in which readings are returned.
    #[must_use]
    fn units(&self) -> PhysicalUnits;

    #[must_use]
    fn reading_labels(&self) -> Labels;

    /// String label of the sensor instance.
    ///
    /// For instance, in the case of a temperature sensor, this allows to specify whether it is
    /// placed indoor or outdoor.
    #[must_use]
    fn label(&self) -> Option<&'static str>;

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

impl dyn Sensor {
    pub fn downcast_ref<T: Sensor>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }
}

/// Implemented on types returned by [`Sensor::read()`].
///
/// [`PhysicalValues`] implements this trait, and should usually be used by sensor driver
/// implementors.
pub trait Reading: core::fmt::Debug {
    /// Returns the [`PhysicalValue`] of a sensor reading.
    fn value(&self) -> PhysicalValue;

    /// Returns an iterator over [`PhysicalValue`]s of a sensor reading.
    ///
    /// The default implementation must be overridden on types containing multiple
    /// [`PhysicalValue`]s.
    fn values(&self) -> impl ExactSizeIterator<Item = PhysicalValue> {
        [self.value()].into_iter()
    }
}

riot_rs_macros::define_count_adjusted_enums!();

/// Represents a value obtained from a sensor.
///
/// The [`Sensor::value_scales()`] must be taken into account using the following formula:
///
/// <math xmlns="http://www.w3.org/1998/Math/MathML" display="block"><mrow><mi mathvariant="monospace">PhysicalValue::value()</mi></mrow><mo>·</mo><msup><mn>10</mn><mrow><mi mathvariant="monospace">value_scale</mi></mrow></msup></math>
///
/// For instance, in the case of a temperature sensor, if [`PhysicalValue::value()`] returns `2225`
/// and [value scale](Sensor::value_scales) is `-2`, it means that the temperature measured
/// and returned by the hardware sensor is `22.25` (the [measurement error](PhysicalValue::error())
/// must additionally be taken into account).
///
/// The unit of measurement can be obtained using [`Sensor::units()`].
///
/// This is required to avoid handling floats.
#[derive(Debug, Copy, Clone, serde::Serialize)]
pub struct PhysicalValue {
    value: i32,
    error: MeasurementError,
}

impl PhysicalValue {
    /// Creates a new value.
    #[must_use]
    pub const fn new(value: i32, error: MeasurementError) -> Self {
        Self { value, error }
    }

    /// Returns the value.
    #[must_use]
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Returns the measurement error.
    #[must_use]
    pub fn error(&self) -> MeasurementError {
        self.error
    }
}

/// Specifies the accuracy error of a measurement.
///
/// It is assumed that the accuracy error is symmetrical around a possibly non-zero bias.
///
/// The unit of measurement is that of the sensor driver, as provided by [`Sensor::units()`].
/// The `scale` is used for both `deviation` and `bias`.
/// The accuracy error is thus given by the following formulas:
///
/// <math xmlns="http://www.w3.org/1998/Math/MathML" display="block"><mo>+</mo><mo>(</mo><mrow><mi mathvariant="monospace">bias</mi></mrow><mo>+</mo><mrow><mi mathvariant="monospace">deviation</mi></mrow><mo>)</mo><mo>·</mo><msup><mn>10</mn><mrow><mi mathvariant="monospace">scale</mi></mrow></msup>/<mo>-</mo><mo>(</mo><mrow><mi mathvariant="monospace">bias</mi></mrow><mo>-</mo><mrow><mi mathvariant="monospace">deviation</mi></mrow><mo>)</mo><mo>·</mo><msup><mn>10</mn><mrow><mi mathvariant="monospace">scale</mi></mrow></msup></math>
///
/// # Examples
///
/// The DS18B20 temperature sensor accuracy error is <mo>+</mo><mn>0.05</mn>/<mo>-</mo><mn>0.45</mn> at 20 °C (see Figure 1 of
/// its datasheet).
/// [`MeasurementError`] would thus be the following:
///
/// ```
/// # use riot_rs_sensors::sensor::MeasurementError;
/// MeasurementError {
///     deviation: 25,
///     bias: -20,
///     scale: -2,
/// }
/// # ;
/// ```
#[derive(Debug, Copy, Clone, serde::Serialize)]
pub enum MeasurementError {
    /// Unknown measurement error.
    Unknown,
    /// No measurement error (e.g., boolean values).
    None,
    /// Measurement error symmetrical around the [`bias`](MeasurementError.bias).
    Symmetrical {
        deviation: i16, // FIXME: rename this and provide a clear definition (3-sigma precision?)
        bias: i16,
        scale: i8,
    },
}

/// A notification provided by a sensor driver.
// TODO: should we pass the value as well? that may be difficult because of the required generics
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub enum Notification {
    ReadingAvailable,
    Threshold(ThresholdKind),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize)]
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

/// Returns the result of calling the method `$method` on the sensor concrete type.
///
/// Downcasts the provided sensor (which must be implementing the [`Sensor`] trait) to its concrete
/// type, and calls the async, non-dispatchable `Sensor::$method()` method on it.
/// This is in particular required to call [`Sensor::read()`] on a `dyn Sensor` trait object because
/// [`Sensor::read()`] is non-dispatchable and can therefore only be called on a concrete type.
///
/// This macro needs to be provided with the sensor, the method to call and with the list of
/// existing sensor concrete types.
///
/// # Panics
///
/// Panics if the concrete type of the sensor was not present in the list of types provided.
// Should not be used by users directly, users should use the `riot_rs::await_sensor!()` proc-macro
// instead.
#[doc(hidden)]
#[macro_export]
macro_rules! _await_read_sensor {
    ($sensor:path, $first_sensor_type:path, $($sensor_type:path),* $(,)?) => {
        {
            use $crate::Sensor;
            // As `Sensor::read()` is non-dispatchable, we have to downcast
            if let Some(sensor) = $sensor.downcast_ref::<$first_sensor_type>() {
                sensor.read().await
            }
            $(
            else if let Some(sensor) = $sensor.downcast_ref::<$sensor_type>() {
                sensor.read().await
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
