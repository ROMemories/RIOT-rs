/// Categories a sensor can be part of.
///
/// A sensor can be part of multiple categories.
///
/// # For sensor driver implementors
///
/// Missing variants can be added when required.
/// Please open an issue to discuss it.
// Built upon https://doc.riot-os.org/group__drivers__saul.html#ga8f2dfec7e99562dbe5d785467bb71bbb
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum Category {
    /// Accelerometer.
    Accelerometer,
    /// Humidity sensor.
    Humidity,
    /// Humidity and temperature sensor.
    HumidityTemperature,
    /// Push button.
    PushButton,
    /// Temperature sensor.
    Temperature,
}
