#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(used_with_arg)]

use embassy_time::{Duration, Timer};
use riot_rs::embassy::{
    arch::peripherals,
    gpio::{DriveStrength, Input, Level, Output, Pull},
};

#[cfg(context = "nrf52840dk")]
riot_rs::define_peripherals!(BlinkyPeripherals { led1: P0_13 });

#[cfg(context = "nrf52840dk")]
riot_rs::define_peripherals!(BlinkyButtonPeripherals {
    led2: P0_14,
    btn2: P0_12,
});

#[cfg(context = "rp")]
riot_rs::define_peripherals!(BlinkyPeripherals { led1: PIN_1 });

#[cfg(context = "rp")]
riot_rs::define_peripherals!(BlinkyButtonPeripherals {
    led2: PIN_2,
    btn2: PIN_6,
});

#[cfg(context = "esp")]
riot_rs::define_peripherals!(BlinkyPeripherals { led1: GPIO_0 });

#[cfg(context = "esp")]
riot_rs::define_peripherals!(BlinkyButtonPeripherals {
    led2: GPIO_1,
    btn2: GPIO_2,
});

#[cfg(context = "stm32")]
riot_rs::define_peripherals!(BlinkyPeripherals { led1: PA6 });

#[cfg(context = "stm32")]
riot_rs::define_peripherals!(BlinkyButtonPeripherals {
    led2: PA7,
    btn2: PA9,
});

#[riot_rs::task(autostart, peripherals)]
async fn blinky(peripherals: BlinkyPeripherals) {
    // All of the following are possible (not all of them are equivalent):
    //
    // let mut led1 = Output::new(peripherals.led1, Level::High);
    //
    let mut led1 = Output::builder(peripherals.led1, Level::Low)
        .opt_drive_strength(DriveStrength::default())
        .build();
    //
    // #[cfg(context = "nrf")]
    // let mut led1 = Output::builder(peripherals.led1, Level::High)
    //     .drive_strength(DriveStrength::Medium)
    //     .build();
    //
    // #[cfg(context = "nrf")]
    // let mut led1 = Output::builder(peripherals.led1, Level::High)
    //     .drive_strength(DriveStrength::Arch(
    //         riot_rs::embassy::arch::DriveStrength::High,
    //     ))
    //     .build();

    loop {
        led1.toggle();
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[riot_rs::task(autostart, peripherals)]
async fn blinky_button(peripherals: BlinkyButtonPeripherals) {
    let btn2_builder = Input::builder(peripherals.btn2, Pull::Up);
    #[cfg(context = "rp")]
    let btn2_builder = btn2_builder.schmitt_trigger(true);
    let mut btn2 = btn2_builder.build_with_interrupt().unwrap();

    let mut led2 = Output::new(peripherals.led2, Level::High);

    loop {
        // Wait for the button to be pressed
        btn2.wait_for_low().await;
        led2.toggle();
        Timer::after(Duration::from_millis(200)).await;
    }
}
