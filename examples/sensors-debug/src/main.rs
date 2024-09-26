#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_assoc_type)]
#![feature(used_with_arg)]

#[riot_rs::hw_setup]
mod sensors {}
// mod sensors;

use embassy_time::{Duration, Timer};
use riot_rs::{
    arch::{self, peripherals},
    debug::println,
    gpio,
    sensors::{Reading, REGISTRY},
};

#[riot_rs::task(autostart)]
async fn main() {
    loop {
        println!("New measurements:");
        for sensor in REGISTRY.sensors() {
            match riot_rs::sensors::measure!(sensor).await {
                Ok(values) => {
                    for (i, value) in values.values().enumerate() {
                        let reading_info = sensor.reading_infos().iter().nth(i).unwrap();
                        let value = value.value() as f32
                            / 10i32.pow((-reading_info.scaling()) as u32) as f32;
                        println!(
                            "{} ({}): {} {} ({})",
                            sensor.display_name().unwrap_or("unknown"),
                            sensor.label().unwrap_or("no label"),
                            value,
                            reading_info.unit(),
                            reading_info.label(),
                        );
                    }
                }
                Err(err) => {
                    println!("error while reading sensor value: {}", err);
                }
            }
        }

        Timer::after(Duration::from_millis(1000)).await;
    }
}

// riot_rs::define_peripherals!(AccelInterruptsPeripherals { int1: PIN_22 });

// #[riot_rs::task(autostart, peripherals)]
// async fn accel_subscriber(peripherals: AccelInterruptsPeripherals) {
//     use riot_rs::sensors::interrupts::{
//         AccelerometerInterruptEvent, DeviceInterrupt, InterruptEvent, InterruptEventKind,
//     };
//
//     let event = InterruptEvent {
//         kind: InterruptEventKind::Accelerometer(AccelerometerInterruptEvent::Movement),
//         duration: None,
//     };
//
//     // TODO: codegen this, or make this part of the sensor init
//     let interrupt_pin = gpio::Input::new(peripherals.int1, gpio::Pull::None);
//     sensors::ACCEL
//         .register_interrupt_pin(interrupt_pin, DeviceInterrupt::Int1, event)
//         .await
//         .unwrap();
//
//     // let accel = REGISTRY
//     //     .sensors()
//     //     .find(|s| s.categories().contains(&riot_rs::sensors::Category::Accelerometer))
//     //     .unwrap();
//     loop {
//         println!("Waiting for movement");
//         sensors::ACCEL
//             .wait_for_interrupt_event(event)
//             .await
//             .unwrap();
//         println!("Moving!");
//     }
// }

#[cfg(capability = "hw/usb-device-port")]
#[riot_rs::config(usb)]
fn usb_config() -> riot_rs::reexports::embassy_usb::Config<'static> {
    let mut config = riot_rs::reexports::embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("Sensors example");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Required for Windows support.
    config.composite_with_iads = true;
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config
}
