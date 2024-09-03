pub trait IntoLevel {
    fn into(level: Self) -> riot_rs_shared_types::gpio::Level;
}

pub mod input {
    use embassy_rp::{
        gpio::{Level, Pull},
        Peripheral,
    };

    #[cfg(feature = "external-interrupts")]
    use riot_rs_shared_types::gpio::input::InterruptError;

    pub use embassy_rp::gpio::{Input, Pin as InputPin};

    // Re-export `Input` as `IntEnabledInput` as they are interrupt-enabled.
    #[cfg(feature = "external-interrupts")]
    pub use embassy_rp::gpio::Input as IntEnabledInput;

    pub const SCHMITT_TRIGGER_CONFIGURABLE: bool = true;

    pub fn new(
        pin: impl Peripheral<P: InputPin> + 'static,
        pull: riot_rs_shared_types::gpio::Pull,
        schmitt_trigger: bool,
    ) -> Result<Input<'static>, core::convert::Infallible> {
        let pull = from_pull(pull);

        let mut input = Input::new(pin, pull);
        input.set_schmitt(schmitt_trigger);

        Ok(input)
    }

    #[cfg(feature = "external-interrupts")]
    pub fn new_int_enabled(
        pin: impl Peripheral<P: InputPin> + 'static,
        pull: riot_rs_shared_types::gpio::Pull,
        schmitt_trigger: bool,
    ) -> Result<IntEnabledInput<'static>, InterruptError> {
        // This architecture does not require special treatment of external interrupts.
        match new(pin, pull, schmitt_trigger) {
            Ok(input) => Ok(input),
            Err(err) => match err {
                // Compile-time check that this never happens as the Result is Infallible.
            },
        }
    }

    impl crate::gpio::IntoLevel for Level {
        fn into(level: Self) -> riot_rs_shared_types::gpio::Level {
            match level {
                Level::Low => riot_rs_shared_types::gpio::Level::Low,
                Level::High => riot_rs_shared_types::gpio::Level::High,
            }
        }
    }

    fn from_pull(pull: riot_rs_shared_types::gpio::Pull) -> Pull {
        match pull {
            riot_rs_shared_types::gpio::Pull::None => Pull::None,
            riot_rs_shared_types::gpio::Pull::Up => Pull::Up,
            riot_rs_shared_types::gpio::Pull::Down => Pull::Down,
        }
    }
    // impl From<crate::gpio::Pull> for Pull {
    //     fn from(pull: crate::gpio::Pull) -> Self {
    //         match pull {
    //             crate::gpio::Pull::None => Pull::None,
    //             crate::gpio::Pull::Up => Pull::Up,
    //             crate::gpio::Pull::Down => Pull::Down,
    //         }
    //     }
    // }
    //
    // impl From<Level> for crate::gpio::Level {
    //     fn from(level: Level) -> Self {
    //         match level {
    //             Level::Low => crate::gpio::Level::Low,
    //             Level::High => crate::gpio::Level::High,
    //         }
    //     }
    // }
}

pub mod output {
    use embassy_rp::{
        gpio::{Drive, Level, SlewRate},
        Peripheral,
    };
    use riot_rs_shared_types::gpio::{FromDriveStrength, FromSpeed};

    pub use embassy_rp::gpio::{Output, Pin as OutputPin};

    pub const DRIVE_STRENGTH_CONFIGURABLE: bool = true;
    pub const SPEED_CONFIGURABLE: bool = true;

    pub fn new(
        pin: impl Peripheral<P: OutputPin> + 'static,
        initial_level: riot_rs_shared_types::gpio::Level,
        drive_strength: DriveStrength,
        speed: Speed,
    ) -> Output<'static> {
        let initial_level = match initial_level {
            riot_rs_shared_types::gpio::Level::Low => Level::Low,
            riot_rs_shared_types::gpio::Level::High => Level::High,
        };
        let mut output = Output::new(pin, initial_level);
        output.set_drive_strength(drive_strength.into());
        output.set_slew_rate(speed.into());
        output
    }

    // crate::gpio::impl_from_level!(Level);

    // We provide our own type because the upstream type is not `Copy` and has no `Default` impl.
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub enum DriveStrength {
        _2mA,
        _4mA,
        _8mA,
        _12mA,
    }

    impl Default for DriveStrength {
        fn default() -> Self {
            // Reset value
            Self::_4mA
        }
    }

    impl From<DriveStrength> for Drive {
        fn from(drive_strength: DriveStrength) -> Self {
            match drive_strength {
                DriveStrength::_2mA => Self::_2mA,
                DriveStrength::_4mA => Self::_4mA,
                DriveStrength::_8mA => Self::_8mA,
                DriveStrength::_12mA => Self::_12mA,
            }
        }
    }

    impl FromDriveStrength for DriveStrength {
        fn from(drive_strength: riot_rs_shared_types::gpio::DriveStrength<Self>) -> Self {
            use riot_rs_shared_types::gpio::DriveStrength::*;

            // ESPs are able to output up to 40 mA, so we somewhat normalize this.
            match drive_strength {
                Arch(drive_strength) => drive_strength,
                Lowest => DriveStrength::_2mA,
                Standard => DriveStrength::default(),
                Medium => DriveStrength::_8mA,
                High => DriveStrength::_12mA,
                Highest => DriveStrength::_12mA,
            }
        }
    }

    // These values do not seem to be quantitatively defined on the RP2040.
    // We provide our own type because the `SlewRate` upstream type is not `Copy` and has no
    // `Default` impl.
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub enum Speed {
        Low,
        High,
    }

    impl Default for Speed {
        fn default() -> Self {
            // Reset value
            Self::Low
        }
    }

    impl From<Speed> for SlewRate {
        fn from(speed: Speed) -> Self {
            match speed {
                Speed::Low => SlewRate::Slow,
                Speed::High => SlewRate::Fast,
            }
        }
    }

    impl FromSpeed for Speed {
        fn from(speed: riot_rs_shared_types::gpio::Speed<Self>) -> Self {
            use riot_rs_shared_types::gpio::Speed::*;

            match speed {
                Arch(speed) => speed,
                Low => Speed::Low,
                Medium => Speed::Low,
                High => Speed::High,
                VeryHigh => Speed::High,
            }
        }
    }
}
