#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(used_with_arg)]

use core::cell::OnceCell;

use linkme::distributed_slice;
use static_cell::make_static;

use embassy_executor::{InterruptExecutor, Spawner};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};

#[cfg(feature = "usb")]
use embassy_usb::{Builder as UsbBuilder, UsbDevice};

pub mod blocker;

pub type Task = fn(Spawner, TaskArgs);

#[derive(Copy, Clone)]
pub struct TaskArgs {
    pub peripherals: &'static Mutex<CriticalSectionRawMutex, arch::OptionalPeripherals>,
    #[cfg(feature = "usb_ethernet")]
    pub stack: &'static OnceCell<&'static UsbEthernetStack>,
}

pub type UsbEthernetStack = Stack<UsbEthernetDevice<'static, ETHERNET_MTU>>;

pub static EXECUTOR: InterruptExecutor = InterruptExecutor::new();

#[distributed_slice]
pub static EMBASSY_TASKS: [Task] = [..];

#[cfg(context = "nrf52")]
mod nrf52 {
    pub use embassy_nrf::interrupt;
    pub use embassy_nrf::interrupt::SWI0_EGU0 as SWI;
    pub use embassy_nrf::{init, peripherals, OptionalPeripherals};

    #[cfg(feature = "usb")]
    use embassy_nrf::{bind_interrupts, rng, usb as nrf_usb};

    #[cfg(feature = "usb")]
    bind_interrupts!(struct Irqs {
        USBD => nrf_usb::InterruptHandler<peripherals::USBD>;
        POWER_CLOCK => nrf_usb::vbus_detect::InterruptHandler;
        RNG => rng::InterruptHandler<peripherals::RNG>;
    });

    #[interrupt]
    unsafe fn SWI0_EGU0() {
        crate::EXECUTOR.on_interrupt()
    }

    #[cfg(feature = "usb")]
    pub mod usb {
        use embassy_nrf::peripherals;
        use embassy_nrf::usb::{vbus_detect::HardwareVbusDetect, Driver};
        pub type UsbDriver = Driver<'static, peripherals::USBD, HardwareVbusDetect>;
        pub fn driver(usbd: peripherals::USBD) -> UsbDriver {
            use super::Irqs;
            Driver::new(usbd, Irqs, HardwareVbusDetect::new(Irqs))
        }
    }
}

#[cfg(context = "rp2040")]
mod rp2040 {
    pub use embassy_rp::interrupt;
    pub use embassy_rp::interrupt::SWI_IRQ_1 as SWI;
    pub use embassy_rp::{init, peripherals, OptionalPeripherals};

    #[cfg(feature = "usb")]
    use embassy_rp::{bind_interrupts, peripherals::USB, usb::InterruptHandler};

    // rp2040 usb start
    #[cfg(feature = "usb")]
    bind_interrupts!(struct Irqs {
        USBCTRL_IRQ => InterruptHandler<USB>;
    });

    #[interrupt]
    unsafe fn SWI_IRQ_1() {
        crate::EXECUTOR.on_interrupt()
    }

    #[cfg(feature = "usb")]
    pub mod usb {
        use embassy_rp::peripherals;
        use embassy_rp::usb::Driver;
        pub type UsbDriver = Driver<'static, peripherals::USB>;
        pub fn driver(usb: peripherals::USB) -> UsbDriver {
            Driver::new(usb, super::Irqs)
        }
    }
}

#[cfg(context = "nrf52")]
use nrf52 as arch;

#[cfg(context = "rp2040")]
use rp2040 as arch;

use arch::SWI;

//
// usb common start
#[cfg(feature = "usb")]
use arch::usb::UsbDriver;

#[cfg(feature = "usb")]
#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, UsbDriver>) -> ! {
    device.run().await
}
// usb common end
//

//
// net begin
const ETHERNET_MTU: usize = 1514;

#[cfg(feature = "net")]
use embassy_net::{Stack, StackResources};
// net end
//

//
// usb net begin
#[cfg(feature = "usb_ethernet")]
use embassy_usb::class::cdc_ncm::embassy_net::{
    Device as UsbEthernetDevice, Runner as UsbEthernetRunner,
};

#[cfg(feature = "usb_ethernet")]
#[embassy_executor::task]
async fn usb_ncm_task(class: UsbEthernetRunner<'static, UsbDriver, ETHERNET_MTU>) -> ! {
    class.run().await
}

#[cfg(feature = "usb_ethernet")]
#[embassy_executor::task]
async fn usb_ethernet_task(stack: &'static UsbEthernetStack) -> ! {
    stack.run().await
}
// usb net end
//

#[cfg(feature = "usb")]
const fn usb_default_config() -> embassy_usb::Config<'static> {
    // Create embassy-usb Config
    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB-Ethernet example");
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

#[distributed_slice(riot_rs_rt::INIT_FUNCS)]
pub(crate) fn init() {
    riot_rs_rt::debug::println!("riot-rs-embassy::init()");
    let p = arch::OptionalPeripherals::from(arch::init(Default::default()));
    EXECUTOR.start(SWI);
    EXECUTOR.spawner().spawn(init_task(p)).unwrap();
    riot_rs_rt::debug::println!("riot-rs-embassy::init() done");
}

#[embassy_executor::task]
async fn init_task(peripherals: arch::OptionalPeripherals) {
    riot_rs_rt::debug::println!("riot-rs-embassy::init_task()");

    #[cfg(all(context = "nrf52", feature = "usb"))]
    {
        // nrf52840
        let clock: embassy_nrf::pac::CLOCK = unsafe { core::mem::transmute(()) };

        riot_rs_rt::debug::println!("nrf: enabling ext hfosc...");
        clock.tasks_hfclkstart.write(|w| unsafe { w.bits(1) });
        while clock.events_hfclkstarted.read().bits() != 1 {}
    }

    let spawner = Spawner::for_current_executor().await;

    let args = TaskArgs {
        peripherals: make_static!(Mutex::new(peripherals)),
        #[cfg(feature = "usb_ethernet")]
        stack: make_static!(OnceCell::new()),
    };

    for task in EMBASSY_TASKS {
        task(spawner, args);
    }

    riot_rs_rt::debug::println!("riot-rs-embassy::init_task() done");
}

#[cfg(feature = "usb_ethernet")]
pub async fn init_usb_ethernet_stack(
    peripherals: &mut arch::OptionalPeripherals,
) -> &'static UsbEthernetStack {
    #[cfg(feature = "usb")]
    let mut usb_builder = {
        let usb_config = usb_default_config();

        let usbd = peripherals.USBD.take().unwrap();

        #[cfg(context = "nrf52")]
        let usb_driver = nrf52::usb::driver(usbd);

        #[cfg(context = "rp2040")]
        let usb_driver = rp2040::usb::driver(usbd);

        // Create embassy-usb DeviceBuilder using the driver and config.
        let builder = UsbBuilder::new(
            usb_driver,
            usb_config,
            &mut make_static!([0; 256])[..],
            &mut make_static!([0; 256])[..],
            &mut make_static!([0; 256])[..],
            &mut make_static!([0; 128])[..],
            &mut make_static!([0; 128])[..],
        );

        builder
    };

    // Our MAC addr.
    #[cfg(feature = "usb_ethernet")]
    let our_mac_addr = [0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC];

    #[cfg(feature = "usb_ethernet")]
    let usb_cdc_ecm = {
        // Host's MAC addr. This is the MAC the host "thinks" its USB-to-ethernet adapter has.
        let host_mac_addr = [0x88, 0x88, 0x88, 0x88, 0x88, 0x88];

        use embassy_usb::class::cdc_ncm::{CdcNcmClass, State};

        // Create classes on the builder.
        CdcNcmClass::new(
            &mut usb_builder,
            make_static!(State::new()),
            host_mac_addr,
            64,
        )
    };

    let spawner = Spawner::for_current_executor().await;

    #[cfg(feature = "usb_ethernet")]
    let usb_ethernet_device = {
        use embassy_usb::class::cdc_ncm::embassy_net::State as NetState;
        let (runner, device) = usb_cdc_ecm.into_embassy_net_device::<ETHERNET_MTU, 4, 4>(
            make_static!(NetState::new()),
            our_mac_addr,
        );

        spawner.spawn(usb_ncm_task(runner)).unwrap();

        device
    };

    #[cfg(feature = "usb_ethernet")]
    let stack = {
        // network stack
        //let config = embassy_net::Config::dhcpv4(Default::default());
        use embassy_net::{Ipv4Address, Ipv4Cidr};
        let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
            address: Ipv4Cidr::new(Ipv4Address::new(10, 42, 0, 61), 24),
            dns_servers: heapless::Vec::new(),
            gateway: Some(Ipv4Address::new(10, 42, 0, 1)),
        });

        // Generate random seed
        // let mut rng = Rng::new(p.RNG, Irqs);
        // let mut seed = [0; 8];
        // rng.blocking_fill_bytes(&mut seed);
        // let seed = u64::from_le_bytes(seed);
        let seed = 1234u64;

        // Init network stack
        let stack = &*make_static!(UsbEthernetStack::new(
            usb_ethernet_device,
            config,
            make_static!(StackResources::<2>::new()),
            seed
        ));

        spawner.spawn(usb_ethernet_task(stack)).unwrap();

        stack
    };

    #[cfg(feature = "usb")]
    {
        let usb = usb_builder.build();
        spawner.spawn(usb_task(usb)).unwrap();
    }

    stack
}
