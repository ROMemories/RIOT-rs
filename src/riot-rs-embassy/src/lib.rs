#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(used_with_arg)]

#[cfg(all(feature = "usb_ethernet", feature = "usb_hid"))]
compile_error!("feature \"usb_ethernet\" and feature \"usb_hid\" cannot be enabled at the same time");

use core::cell::Cell;

use linkme::distributed_slice;

use static_cell::make_static;
use embassy_executor::{InterruptExecutor, Spawner};

#[cfg(feature = "usb")]
use embassy_usb::{Builder, UsbDevice};

#[cfg(feature = "usb_hid")]
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex as EmbassyMutex};

pub mod blocker;

pub type Task = fn(Spawner, TaskArgs);

#[allow(non_snake_case)]
#[derive(Copy, Clone)]
pub struct TaskArgs {
    #[cfg(feature = "net")]
    pub stack: &'static Stack<Device<'static, MTU>>,
    #[cfg(feature = "usb_hid")]
    pub hid_writer:
        &'static EmbassyMutex<CriticalSectionRawMutex, hid::HidWriter<'static, UsbDriver, 8>>,
    #[cfg(context = "nrf52")]
    pub P0_11: &'static Cell<Option<embassy_nrf::peripherals::P0_11>>,
}

pub static EXECUTOR: InterruptExecutor = InterruptExecutor::new();

#[distributed_slice]
pub static EMBASSY_TASKS: [Task] = [..];

#[cfg(context = "nrf52")]
mod nrf52 {
    pub use embassy_nrf::interrupt;
    pub use embassy_nrf::interrupt::SWI0_EGU0 as SWI;
    pub use embassy_nrf::{init, Peripherals};

    #[cfg(feature = "usb")]
    use embassy_nrf::{bind_interrupts, peripherals, rng, usb as nrf_usb};

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
    pub use embassy_rp::{init, Peripherals};

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

#[cfg(feature = "net")]
//
// usb net begin
#[cfg(feature = "net")]
const MTU: usize = 1514;

#[cfg(feature = "net")]
use embassy_net::{Stack, StackResources};

#[cfg(feature = "usb_ethernet")]
use embassy_usb::class::cdc_ncm::embassy_net::{Device, Runner};

#[cfg(feature = "usb_ethernet")]
#[embassy_executor::task]
async fn usb_ncm_task(class: Runner<'static, UsbDriver, MTU>) -> ! {
    class.run().await
}

#[cfg(feature = "usb_ethernet")]
#[embassy_executor::task]
async fn net_task(stack: &'static Stack<Device<'static, MTU>>) -> ! {
    stack.run().await
}
// usb net end
//

//
// USB HID begin
#[cfg(feature = "usb_hid")]
use embassy_usb::class::hid;
#[cfg(feature = "usb_hid")]
pub use usbd_hid;

#[cfg(feature = "usb_hid")]
#[embassy_executor::task]
async fn hid_task(
    hid_reader: hid::HidReader<'static, UsbDriver, 1>,
    request_handler: &'static MyRequestHandler,
) -> ! {
    hid_reader.run(false, request_handler).await
}
// USB HID end
//

#[cfg(feature = "usb_ethernet")]
const fn usb_ethernet_config() -> embassy_usb::Config<'static> {
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

#[cfg(feature = "usb_hid")]
const fn usb_hid_config() -> embassy_usb::Config<'static> {
    // Create embassy-usb Config
    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB HID keyboard example");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    config
}

#[distributed_slice(riot_rs_rt::INIT_FUNCS)]
pub(crate) fn init() {
    riot_rs_rt::debug::println!("riot-rs-embassy::init()");
    let p = arch::init(Default::default());
    EXECUTOR.start(SWI);
    EXECUTOR.spawner().spawn(init_task(p)).unwrap();
    riot_rs_rt::debug::println!("riot-rs-embassy::init() done");
}

#[embassy_executor::task]
async fn init_task(peripherals: arch::Peripherals) {
    riot_rs_rt::debug::println!("riot-rs-embassy::init_task()");

    #[cfg(all(context = "nrf52", feature = "usb"))]
    {
        // nrf52840
        let clock: embassy_nrf::pac::CLOCK = unsafe { core::mem::transmute(()) };

        riot_rs_rt::debug::println!("nrf: enabling ext hfosc...");
        clock.tasks_hfclkstart.write(|w| unsafe { w.bits(1) });
        while clock.events_hfclkstarted.read().bits() != 1 {}
    }

    #[cfg(feature = "usb")]
    let mut usb_builder = {
        #[cfg(feature = "usb_ethernet")]
        let usb_config = usb_ethernet_config();
        #[cfg(feature = "usb_hid")]
        let usb_config = usb_hid_config();

        #[cfg(context = "nrf52")]
        let usb_driver = nrf52::usb::driver(peripherals.USBD);

        #[cfg(context = "rp2040")]
        let usb_driver = rp2040::usb::driver(peripherals.USB);

        // Create embassy-usb DeviceBuilder using the driver and config.
        let builder = Builder::new(
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

    #[cfg(feature = "usb_hid")]
    let hid_writer = {
        let request_handler = make_static!(MyRequestHandler {});
        let config = embassy_usb::class::hid::Config {
            report_descriptor: <usbd_hid::descriptor::KeyboardReport as usbd_hid::descriptor::SerializedDescriptor>::desc(),
            request_handler: Some(request_handler),
            poll_ms: 60,
            max_packet_size: 64,
        };
        // FIXME: use a proper USB HID configuration for the USB Builder
        let hid_rw = hid::HidReaderWriter::<_, 1, 8>::new(
            &mut usb_builder,
            make_static!(hid::State::new()),
            config,
        );
        let (hid_reader, hid_writer) = hid_rw.split();
        spawner
            .spawn(hid_task(hid_reader, request_handler))
            .unwrap();
        make_static!(EmbassyMutex::new(hid_writer))
    };

    #[cfg(feature = "usb")]
    let usb = { usb_builder.build() };
    #[cfg(feature = "usb")]
    spawner.spawn(usb_task(usb)).unwrap();

    #[cfg(feature = "usb_ethernet")]
    let device = {
        use embassy_usb::class::cdc_ncm::embassy_net::State as NetState;
        let (runner, device) = usb_cdc_ecm
            .into_embassy_net_device::<MTU, 4, 4>(make_static!(NetState::new()), our_mac_addr);

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
        let stack = &*make_static!(Stack::new(
            device,
            config,
            make_static!(StackResources::<2>::new()),
            seed
        ));

        spawner.spawn(net_task(stack)).unwrap();

        stack
    };

    let args = TaskArgs {
        #[cfg(feature = "net")]
        stack,
        #[cfg(feature = "usb_hid")]
        hid_writer,
        #[cfg(context = "nrf52")]
        P0_11: make_static!(Cell::new(Some(peripherals.P0_11))),
    };

    for task in EMBASSY_TASKS {
        task(spawner, args);
    }

    // mark used
    let _ = peripherals;

    riot_rs_rt::debug::println!("riot-rs-embassy::init_task() done");
}

#[cfg(feature = "usb_hid")]
struct MyRequestHandler;

#[cfg(feature = "usb_hid")]
impl hid::RequestHandler for MyRequestHandler {
    fn get_report(&self, id: hid::ReportId, _buf: &mut [u8]) -> Option<usize> {
        riot_rs_rt::debug::println!("Get report for {:?}", id);
        None
    }

    fn set_report(&self, id: hid::ReportId, data: &[u8]) -> embassy_usb::control::OutResponse {
        riot_rs_rt::debug::println!("Set report for {:?}: {:?}", id, data);
        embassy_usb::control::OutResponse::Accepted
    }

    fn set_idle_ms(&self, id: Option<hid::ReportId>, dur: u32) {
        riot_rs_rt::debug::println!("Set idle rate for {:?} to {:?}", id, dur);
    }

    fn get_idle_ms(&self, id: Option<hid::ReportId>) -> Option<u32> {
        riot_rs_rt::debug::println!("Get idle rate for {:?}", id);
        None
    }
}
