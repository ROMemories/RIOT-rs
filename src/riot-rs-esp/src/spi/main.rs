use esp_hal::{
    dma::{self, DmaPriority},
    gpio::{self, InputPin, OutputPin},
    peripheral::Peripheral,
    peripherals,
    spi::{
        master::dma::{SpiDma as InnerSpi, WithDmaSpi2},
        FullDuplexMode,
    },
    Async,
};
use riot_rs_shared_types::{
    impl_async_spibus_for_driver_enum,
    spi::{BitOrder, Mode},
};

#[derive(Clone)]
#[non_exhaustive]
pub struct Config {
    pub frequency: Frequency,
    pub mode: Mode,
    pub bit_order: BitOrder,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            frequency: Frequency::_1M,
            mode: Mode::Mode0,
            bit_order: BitOrder::default(),
        }
    }
}

// Possible values are copied from embassy-nrf
// TODO: check how well this matches the ESP32 capabilities
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u32)]
pub enum Frequency {
    _125k,
    _250k,
    _500k,
    _1M,
    _2M,
    _4M,
    _8M,
    _16M,
    _32M,
}

riot_rs_shared_types::impl_spi_from_frequency!();
riot_rs_shared_types::impl_spi_frequency_const_functions_32M!();

impl From<Frequency> for fugit::HertzU32 {
    fn from(freq: Frequency) -> Self {
        match freq {
            Frequency::_125k => fugit::Rate::<u32, 1, 1>::kHz(125),
            Frequency::_250k => fugit::Rate::<u32, 1, 1>::kHz(250),
            Frequency::_500k => fugit::Rate::<u32, 1, 1>::kHz(500),
            Frequency::_1M => fugit::Rate::<u32, 1, 1>::MHz(1),
            Frequency::_2M => fugit::Rate::<u32, 1, 1>::MHz(2),
            Frequency::_4M => fugit::Rate::<u32, 1, 1>::MHz(4),
            Frequency::_8M => fugit::Rate::<u32, 1, 1>::MHz(8),
            Frequency::_16M => fugit::Rate::<u32, 1, 1>::MHz(16),
            Frequency::_32M => fugit::Rate::<u32, 1, 1>::MHz(32),
        }
    }
}

pub(crate) fn init(peripherals: &mut crate::OptionalPeripherals) {
    // Take all SPI peripherals and do nothing with them.
    cfg_if::cfg_if! {
        if #[cfg(context = "esp32c6")] {
            let _ = peripherals.SPI2.take().unwrap();
        } else {
            compile_error!("this ESP32 chip is not supported");
        }
    }
}

macro_rules! define_spi_drivers {
    ($( $peripheral:ident ),* $(,)?) => {
        $(
            /// Peripheral-specific SPI driver.
            pub struct $peripheral {
                spim: InnerSpi<'static, peripherals::$peripheral, dma::DmaChannel1, FullDuplexMode, Async>,
            }

            impl $peripheral {
                #[must_use]
                pub fn new(
                    sck_pin: impl Peripheral<P: OutputPin> + 'static,
                    miso_pin: impl Peripheral<P: InputPin> + 'static,
                    mosi_pin: impl Peripheral<P: OutputPin> + 'static,
                    dma_ch: dma::ChannelCreator<1>, // FIXME: try not to hard-code the DMA channel
                    config: Config,
                ) -> Spi {
                    let frequency = config.frequency.into();
                    let clocks = crate::CLOCKS.get().unwrap();

                    // Make this struct a compile-time-enforced singleton: having multiple statics
                    // defined with the same name would result in a compile-time error.
                    paste::paste! {
                        #[allow(dead_code)]
                        static [<PREVENT_MULTIPLE_ $peripheral>]: () = ();
                    }

                    // FIXME(safety): enforce that the init code indeed has run
                    // SAFETY: this struct being a singleton prevents us from stealing the
                    // peripheral multiple times.
                    let spi_peripheral = unsafe { peripherals::$peripheral::steal() };

                    let spi = esp_hal::spi::master::Spi::new(
                        spi_peripheral,
                        frequency,
                        crate::spi::from_mode(config.mode),
                        clocks,
                    );
                    let spi = spi.with_bit_order(
                        crate::spi::from_bit_order(config.bit_order), // Read order
                        crate::spi::from_bit_order(config.bit_order), // Write order
                    );
                    // The order of MOSI/MISO pins is inverted.
                    let spi = spi.with_pins(
                        Some(sck_pin),
                        Some(mosi_pin),
                        Some(miso_pin),
                        gpio::NO_PIN, // The CS pin is managed separately
                    );

                    // FIXME: is this correct?
                    // Use the highest priority, as SPI is the DMA-enabled peripheral that is the
                    // most latency-sensitive.
                    let burst_mode = false;
                    let dma_priority = DmaPriority::Priority5;
                    let dma_channel = dma_ch.configure_for_async(burst_mode, dma_priority);
                    // FIXME: adjust the value (copied from Embassy SPI example for now)
                    // This value defines the maximum transaction length these DMA channels can
                    // handle.
                    let (tx_dma_descriptors, rx_dma_descriptors) = esp_hal::dma_descriptors!(32000);

                    // FIXME: we need to rebase esp-hal to have the new DMA API:
                    // https://github.com/esp-rs/esp-hal/commit/41f9925e2c393b1b753585e85e21f74cf5a8d131
                    let spi = spi.with_dma(
                        dma_channel,
                        tx_dma_descriptors,
                        rx_dma_descriptors,
                    );

                    Spi::$peripheral(Self { spim: spi })
                }
            }
        )*

        /// Peripheral-agnostic driver.
        pub enum Spi {
            $( $peripheral($peripheral) ),*
        }

        impl embedded_hal_async::spi::ErrorType for Spi {
            type Error = esp_hal::spi::Error;
        }

        impl_async_spibus_for_driver_enum!(Spi, $( $peripheral ),*);
    };
}

// FIXME: there seems to be a DMA-enabled SPI3 on ESP32-S2 and ESP32-S3
// Define a driver per peripheral
#[cfg(context = "esp32c6")]
define_spi_drivers!(SPI2);
