//! Provides consistent GPIO access.

use embedded_hal::digital::StatefulOutputPin;

use crate::arch::{
    gpio::{
        input::{Input as ArchInput, InputPin as ArchInputPin},
        output::{
            DriveStrength as ArchDriveStrength, Output as ArchOutput, OutputPin as ArchOutputPin,
            Speed as ArchSpeed,
        },
    },
    peripheral::Peripheral,
};

use input::InputBuilder;
use output::OutputBuilder;

// We do not provide an `impl` block because it would be grouped separately in the documentation.
macro_rules! inner_impl_input_methods {
    ($inner:ident) => {
        /// Returns whether the input level is high.
        pub fn is_high(&self) -> bool {
            self.$inner.is_high()
        }

        /// Returns whether the input level is low.
        pub fn is_low(&self) -> bool {
            self.$inner.is_low()
        }

        /// Returns the input level.
        pub fn get_level(&self) -> Level {
            self.$inner.get_level().into()
        }
    };
}

/// A GPIO input.
///
/// If support for external interrupts is needed, use [`InputBuilder::build_with_interrupt()`] to
/// obtain an [`IntEnabledInput`].
pub struct Input {
    input: ArchInput<'static>, // FIXME: is this ok to require a 'static pin?
}

impl Input {
    /// Returns a configured [`Input`].
    pub fn new(pin: impl Peripheral<P: ArchInputPin> + 'static, pull: Pull) -> Self {
        Self::builder(pin, pull).build()
    }

    /// Returns an [`InputBuilder`], allowing to configure the GPIO input further.
    pub fn builder<P: Peripheral<P: ArchInputPin>>(pin: P, pull: Pull) -> InputBuilder<P> {
        InputBuilder {
            pin,
            pull,
            schmitt_trigger: false,
        }
    }

    inner_impl_input_methods!(input);
}

#[doc(hidden)]
impl embedded_hal::digital::ErrorType for Input {
    type Error = <ArchInput<'static> as embedded_hal::digital::ErrorType>::Error;
}

/// A GPIO input that supports external interrupts.
///
/// Can be obtained with [`InputBuilder::build_with_interrupt()`].
pub struct IntEnabledInput {
    input: ArchInput<'static>, // FIXME: is this ok to require a 'static pin?
}

impl IntEnabledInput {
    inner_impl_input_methods!(input);

    /// Asynchronously waits until the input level is high.
    /// Returns immediately if it is already high.
    pub async fn wait_for_high(&mut self) {
        self.input.wait_for_high().await;
    }

    /// Asynchronously waits until the input level is low.
    /// Returns immediately if it is already low.
    pub async fn wait_for_low(&mut self) {
        self.input.wait_for_low().await;
    }

    /// Asynchronously waits for the input level to transition from low to high.
    pub async fn wait_for_rising_edge(&mut self) {
        self.input.wait_for_rising_edge().await;
    }

    /// Asynchronously waits for the input level to transition from high to low.
    pub async fn wait_for_falling_edge(&mut self) {
        self.input.wait_for_falling_edge().await;
    }

    /// Asynchronously waits for the input level to transition from one level to the other.
    pub async fn wait_for_any_edge(&mut self) {
        self.input.wait_for_any_edge().await;
    }
}

#[doc(hidden)]
impl embedded_hal::digital::ErrorType for IntEnabledInput {
    type Error = <ArchInput<'static> as embedded_hal::digital::ErrorType>::Error;
}

impl embedded_hal_async::digital::Wait for IntEnabledInput {
    async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
        <ArchInput as embedded_hal_async::digital::Wait>::wait_for_high(&mut self.input).await
    }

    async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
        <ArchInput as embedded_hal_async::digital::Wait>::wait_for_low(&mut self.input).await
    }

    async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
        <ArchInput as embedded_hal_async::digital::Wait>::wait_for_rising_edge(&mut self.input)
            .await
    }

    async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
        <ArchInput as embedded_hal_async::digital::Wait>::wait_for_falling_edge(&mut self.input)
            .await
    }

    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        <ArchInput as embedded_hal_async::digital::Wait>::wait_for_any_edge(&mut self.input).await
    }
}

macro_rules! impl_embedded_hal_input_trait {
    ($type:ident, $arch_type:ident) => {
        impl embedded_hal::digital::InputPin for $type {
            fn is_high(&mut self) -> Result<bool, Self::Error> {
                <$arch_type as embedded_hal::digital::InputPin>::is_high(&mut self.input)
            }

            fn is_low(&mut self) -> Result<bool, Self::Error> {
                <$arch_type as embedded_hal::digital::InputPin>::is_low(&mut self.input)
            }
        }
    };
}

impl_embedded_hal_input_trait!(Input, ArchInput);
impl_embedded_hal_input_trait!(IntEnabledInput, ArchInput);

/// Digital level of an input or output.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Level {
    Low,
    High,
}

impl From<Level> for bool {
    fn from(level: Level) -> Self {
        match level {
            Level::Low => false,
            Level::High => true,
        }
    }
}

impl From<bool> for Level {
    fn from(boolean: bool) -> Self {
        match boolean {
            false => Level::Low,
            true => Level::High,
        }
    }
}

impl From<embedded_hal::digital::PinState> for Level {
    fn from(pin_state: embedded_hal::digital::PinState) -> Self {
        bool::from(pin_state).into()
    }
}

impl From<Level> for embedded_hal::digital::PinState {
    fn from(level: Level) -> Self {
        bool::from(level).into()
    }
}

macro_rules! impl_from_level {
    ($level:ident) => {
        impl From<crate::gpio::Level> for $level {
            fn from(level: crate::gpio::Level) -> Self {
                match level {
                    crate::gpio::Level::Low => $level::Low,
                    crate::gpio::Level::High => $level::High,
                }
            }
        }
    };
}
pub(crate) use impl_from_level;

pub mod input {
    //! Input-specific types.

    use crate::{
        arch::{self, gpio::input::InputPin as ArchInputPin, peripheral::Peripheral},
        extint_registry,
        gpio::Pull,
    };

    use super::{Input, IntEnabledInput};

    /// Builder type for [`Input`], can be obtained with [`Input::builder()`].
    pub struct InputBuilder<P: Peripheral<P: ArchInputPin>> {
        pub(crate) pin: P,
        pub(crate) pull: Pull,
        pub(crate) schmitt_trigger: bool,
    }

    impl<P: Peripheral<P: ArchInputPin> + 'static> InputBuilder<P> {
        /// Configures the input's Schmitt trigger.
        ///
        /// # Note
        ///
        /// Fails to compile if the architecture does not support configuring Schmitt trigger on
        /// inputs.
        pub fn schmitt_trigger(self, enable: bool) -> Self {
            const {
                assert!(
                    arch::gpio::input::SCHMITT_TRIGGER_AVAILABLE,
                    "This architecture does not support configuring Schmitt triggers on GPIO inputs."
                );
            }

            Self {
                schmitt_trigger: enable,
                ..self
            }
        }

        // It is unclear whether `opt_*()` functions are actually useful, so we provide them but do not
        // commit to them being part of our API for now.
        // We may remove them in the future if we realize they are never useful.
        #[doc(hidden)]
        pub fn opt_schmitt_trigger(self, enable: bool) -> Self {
            if arch::gpio::input::SCHMITT_TRIGGER_AVAILABLE {
                // We cannot reuse the non-`opt_*()`, otherwise the const assert inside it would always
                // be triggered.
                Self {
                    schmitt_trigger: enable,
                    ..self
                }
            } else {
                self
            }
        }
    }

    // Split the impl for consistency with outputs.
    impl<P: Peripheral<P: ArchInputPin> + 'static> InputBuilder<P> {
        /// Returns an [`Input`] by finalizing the builder.
        pub fn build(self) -> Input {
            let input =
                match arch::gpio::input::new(self.pin, false, self.pull, self.schmitt_trigger) {
                    Ok(input) => input,
                    Err(Error::InterruptChannel(_)) => unreachable!(),
                };

            Input { input }
        }

        /// Returns an [`IntEnabledInput`] by finalizing the builder.
        ///
        /// # Errors
        ///
        /// On some architectures, the number of external interrupts that can simultaneously be
        /// enabled is limited by the number of hardware interrupt channels.
        /// Some architectures also have other limitations, for instance it may not be possible to
        /// register interrupts on a pin if one is already registered on the pin with the same pin
        /// number of another port (e.g., `PA0` and `PB0`).
        /// In these cases, this returns an [`Error::InterruptChannel`], with an
        /// architecture-specific error.
        // FIXME: rename this
        pub fn build_with_interrupt(self) -> Result<IntEnabledInput, Error> {
            let input = arch::gpio::input::new(self.pin, true, self.pull, self.schmitt_trigger)?;

            Ok(IntEnabledInput { input })
        }
    }

    /// Input-related errors.
    #[derive(Debug)]
    pub enum Error {
        /// Error when hitting hardware limitations regarding interrupt registration.
        /// See
        /// [`InputBuilder::build_with_interrupt()`](super::InputBuilder::build_with_interrupt).
        InterruptChannel(extint_registry::Error),
    }

    impl From<extint_registry::Error> for Error {
        fn from(err: extint_registry::Error) -> Self {
            Error::InterruptChannel(err)
        }
    }
}

/// Pull-up/pull-down resistor configuration.
///
/// All the architectures we support have pull-up and pull-down resistors.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Pull {
    /// No pull-up or pull-down resistor.
    None,
    /// Pull-up resistor.
    Up,
    /// Pull-down resistor.
    Down,
}

/// A GPIO output.
pub struct Output {
    output: ArchOutput<'static>, // FIXME: is this ok to require a 'static pin?
}

impl Output {
    /// Returns a configured [`Output`].
    pub fn new(pin: impl Peripheral<P: ArchOutputPin> + 'static, initial_level: Level) -> Self {
        Self::builder(pin, initial_level).build()
    }

    /// Returns an [`OutputBuilder`], allowing to configure the GPIO output further.
    pub fn builder<P: Peripheral<P: ArchOutputPin>>(
        pin: P,
        initial_level: Level,
    ) -> OutputBuilder<P> {
        OutputBuilder {
            pin,
            initial_level,
            drive_strength: DriveStrength::default(),
            speed: Speed::default(),
        }
    }

    /// Sets the output as high.
    pub fn set_high(&mut self) {
        // All architectures are infallible.
        let _ = <Self as embedded_hal::digital::OutputPin>::set_high(self);
    }

    /// Sets the output as low.
    pub fn set_low(&mut self) {
        // All architectures are infallible.
        let _ = <Self as embedded_hal::digital::OutputPin>::set_low(self);
    }

    /// Sets the output level.
    pub fn set_level(&mut self, level: Level) {
        let state = level.into();
        // All architectures are infallible.
        let _ = <Self as embedded_hal::digital::OutputPin>::set_state(self, state);
    }

    /// Toggles the output level.
    pub fn toggle(&mut self) {
        // All architectures are infallible.
        let _ = <Self as StatefulOutputPin>::toggle(self);
    }
}

/// Drive strength of an output.
///
/// This enum allows to either use high-level, portable values, roughly normalized across
/// architectures, or to use architecture-specific values if needed.
// TODO: should this be marked non_exaustive?
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum DriveStrength {
    Arch(ArchDriveStrength),
    Lowest,
    // Reset value of most GPIOs.
    Standard,
    Medium,
    High,
    Highest,
}

impl Default for DriveStrength {
    fn default() -> Self {
        Self::Standard
    }
}

// We introduce our own trait instead of using `From` because this conversion is not
// value-preserving.
pub(crate) trait FromDriveStrength {
    fn from(drive_strength: DriveStrength) -> ArchDriveStrength;
}

/// Speed setting of an output.
///
/// Speed can be increased when needed, at the price of increasing high-frequency noise.
///
/// This enum allows to either use high-level, portable values, roughly normalized across
/// architectures, or to use architecture-specific values if needed.
#[doc(alias = "SlewRate")]
#[derive(Copy, Clone, PartialEq, Eq)]
// FIXME: should we call this slew rate instead?
pub enum Speed {
    Arch(ArchSpeed),
    Low,
    Medium,
    High,
    VeryHigh,
}

impl Default for Speed {
    fn default() -> Self {
        Self::Low
    }
}

// We introduce our own trait instead of using `From` because this conversion is not
// value-preserving.
pub(crate) trait FromSpeed {
    fn from(speed: Speed) -> ArchSpeed;
}

pub mod output {
    //! Output-specific types.

    use crate::{
        arch::{self, gpio::output::OutputPin as ArchOutputPin, peripheral::Peripheral},
        gpio::{DriveStrength, FromDriveStrength, FromSpeed, Level, Speed},
    };

    use super::{ArchDriveStrength, ArchSpeed, Output};

    /// Builder type for [`Output`], can be obtained with [`Output::builder()`].
    pub struct OutputBuilder<P: Peripheral<P: ArchOutputPin>> {
        pub(crate) pin: P,
        pub(crate) initial_level: Level,
        pub(crate) drive_strength: DriveStrength,
        pub(crate) speed: Speed,
    }

    // We define this in a macro because it will be useful for open-drain outputs.
    macro_rules! impl_output_builder {
        ($type:ident, $pin_trait:ident) => {
            impl<P: Peripheral<P: $pin_trait> + 'static> $type<P> {
                pub fn drive_strength(self, drive_strength: DriveStrength) -> Self {
                    const {
                        assert!(
                            arch::gpio::output::DRIVE_STRENGTH_AVAILABLE,
                            "This architecture does not support setting the drive strength of GPIO outputs."
                        );
                    }

                    Self {
                        drive_strength,
                        ..self
                    }
                }

                // It is unclear whether `opt_*()` functions are actually useful, so we provide them but do not
                // commit to them being part of our API for now.
                // We may remove them in the future if we realize they are never useful.
                #[doc(hidden)]
                // TODO: or `drive_strength_opt`?
                pub fn opt_drive_strength(self, drive_strength: DriveStrength) -> Self {
                    if arch::gpio::output::DRIVE_STRENGTH_AVAILABLE {
                        // We cannot reuse the non-`opt_*()`, otherwise the const assert inside it would always
                        // be triggered.
                        Self {
                            drive_strength,
                            ..self
                        }
                    } else {
                        self
                    }
                }

                pub fn speed(self, speed: Speed) -> Self {
                    const {
                        assert!(
                            arch::gpio::output::SPEED_AVAILABLE,
                            "This architecture does not support setting the speed of GPIO outputs."
                        );
                    }

                    Self { speed, ..self }
                }

                // It is unclear whether `opt_*()` functions are actually useful, so we provide them but do not
                // commit to them being part of our API for now.
                // We may remove them in the future if we realize they are never useful.
                #[doc(hidden)]
                // TODO: or `speed_opt`?
                pub fn opt_speed(self, speed: Speed) -> Self {
                    if arch::gpio::output::SPEED_AVAILABLE {
                        // We cannot reuse the non-`opt_*()`, otherwise the const assert inside it would always
                        // be triggered.
                        Self { speed, ..self }
                    } else {
                        self
                    }
                }
            }
        }
    }

    impl_output_builder!(OutputBuilder, ArchOutputPin);

    impl<P: Peripheral<P: ArchOutputPin> + 'static> OutputBuilder<P> {
        /// Returns an [`Output`] by finalizing the builder.
        pub fn build(self) -> Output {
            // TODO: should we move this into `output::new()`s?
            let drive_strength =
                <ArchDriveStrength as FromDriveStrength>::from(self.drive_strength);
            // TODO: should we move this into `output::new()`s?
            let speed = <ArchSpeed as FromSpeed>::from(self.speed);

            let output =
                arch::gpio::output::new(self.pin, self.initial_level, drive_strength, speed);

            Output { output }
        }
    }
}

// We define this in a macro because it will be useful for open-drain outputs.
macro_rules! impl_embedded_hal_output_traits {
    ($type:ident, $arch_type:ident) => {
        #[doc(hidden)]
        impl embedded_hal::digital::ErrorType for $type {
            type Error = <$arch_type<'static> as embedded_hal::digital::ErrorType>::Error;
        }

        impl embedded_hal::digital::OutputPin for $type {
            fn set_high(&mut self) -> Result<(), Self::Error> {
                <$arch_type as embedded_hal::digital::OutputPin>::set_high(&mut self.output)
            }

            fn set_low(&mut self) -> Result<(), Self::Error> {
                <$arch_type as embedded_hal::digital::OutputPin>::set_low(&mut self.output)
            }
        }

        // Outputs are all stateful outputs on:
        // - embassy-nrf
        // - embassy-rp
        // - esp-hal
        // - embassy-stm32
        impl StatefulOutputPin for $type {
            fn is_set_high(&mut self) -> Result<bool, Self::Error> {
                <$arch_type as StatefulOutputPin>::is_set_high(&mut self.output)
            }

            fn is_set_low(&mut self) -> Result<bool, Self::Error> {
                <$arch_type as StatefulOutputPin>::is_set_low(&mut self.output)
            }
        }
    };
}

impl_embedded_hal_output_traits!(Output, ArchOutput);
