#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(used_with_arg)]
// Cast to &dyn Any from refs
#![feature(trait_upcasting)]
#![deny(unused_must_use)]

mod pins;
mod routes;

use riot_rs as _;

use riot_rs::embassy::{arch, network};
use riot_rs::rt::debug::println;

use embassy_net::tcp::TcpSocket;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::Duration;
use picoserve::routing::get;
use static_cell::make_static;

#[cfg(feature = "button-readings")]
use embassy_nrf::gpio::{Input, Pin, Pull};

struct AppState {
    #[cfg(feature = "button-readings")]
    buttons: ButtonInputs,
}

#[cfg(feature = "button-readings")]
#[derive(Copy, Clone)]
struct ButtonInputs(&'static Mutex<CriticalSectionRawMutex, Buttons>);

#[cfg(feature = "button-readings")]
struct Buttons {
    button1: Input<'static>,
    button2: Input<'static>,
    button3: Input<'static>,
    button4: Input<'static>,
}

#[cfg(feature = "button-readings")]
impl picoserve::extract::FromRef<AppState> for ButtonInputs {
    fn from_ref(state: &AppState) -> Self {
        state.buttons
    }
}

static TEMP_SENSOR: arch::internal_temp::InternalTemp = arch::internal_temp::InternalTemp::new();
// TODO: can we make this const?
#[riot_rs::linkme::distributed_slice(riot_rs::sensors::registry::SENSOR_REFS)]
#[linkme(crate = riot_rs::linkme)]
static TEMP_SENSOR_REF: &'static dyn riot_rs::sensors::sensor::Sensor = &TEMP_SENSOR;

type AppRouter = impl picoserve::routing::PathRouter<AppState>;

const WEB_TASK_POOL_SIZE: usize = 2;

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
async fn web_task(
    id: usize,
    app: &'static picoserve::Router<AppRouter, AppState>,
    config: &'static picoserve::Config<Duration>,
    state: AppState,
) -> ! {
    let stack = network::network_stack().await.unwrap();

    let mut rx_buffer = [0; 1024];
    let mut tx_buffer = [0; 1024];

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        println!("{}: Listening on TCP:80...", id);
        if let Err(e) = socket.accept(80).await {
            println!("{}: accept error: {:?}", id, e);
            continue;
        }

        let remote_endpoint = socket.remote_endpoint();

        println!("{}: Received connection from {:?}", id, remote_endpoint);

        match picoserve::serve_with_state(app, config, &mut [0; 2048], socket, &state).await {
            Ok(handled_requests_count) => {
                println!(
                    "{} requests handled from {:?}",
                    handled_requests_count, remote_endpoint,
                );
            }
            Err(err) => println!("{:?}", err),
        }
    }
}

// TODO: macro up this up
use riot_rs::embassy::{arch::OptionalPeripherals, Spawner};
#[riot_rs::embassy::distributed_slice(riot_rs::embassy::EMBASSY_TASKS)]
#[linkme(crate = riot_rs::embassy::linkme)]
fn web_server_init(spawner: &Spawner, peripherals: &mut arch::OptionalPeripherals) {
    #[cfg(feature = "button-readings")]
    let button_inputs = {
        let buttons = pins::Buttons::take_from(peripherals).unwrap();

        let buttons = Buttons {
            button1: Input::new(buttons.btn1.degrade(), Pull::Up),
            button2: Input::new(buttons.btn2.degrade(), Pull::Up),
            button3: Input::new(buttons.btn3.degrade(), Pull::Up),
            button4: Input::new(buttons.btn4.degrade(), Pull::Up),
        };

        ButtonInputs(make_static!(Mutex::new(buttons)))
    };

    #[cfg(context = "nrf52840")]
    {
        use riot_rs::sensors::registry::REGISTRY;

        let temp_peripheral = peripherals.TEMP.take().unwrap();
        TEMP_SENSOR.init(temp_peripheral);
    }

    fn make_app() -> picoserve::Router<AppRouter, AppState> {
        let router = picoserve::Router::new().route("/", get(routes::index));
        #[cfg(feature = "button-readings")]
        let router = router.route("/buttons", get(routes::buttons));
        let router = router.route("/api/sensors", get(routes::sensors));
        #[cfg(context = "nrf52840")]
        let router = router.route("/api/temp", get(routes::temp));
        router
    }

    let app = make_static!(make_app());

    let config = make_static!(picoserve::Config::new(picoserve::Timeouts {
        start_read_request: Some(Duration::from_secs(5)),
        read_request: Some(Duration::from_secs(1)),
        write: Some(Duration::from_secs(1)),
    }));

    for id in 0..WEB_TASK_POOL_SIZE {
        let app_state = AppState {
            #[cfg(feature = "button-readings")]
            buttons: button_inputs,
        };
        spawner.spawn(web_task(id, app, config, app_state)).unwrap();
    }
}

#[riot_rs::thread]
fn dummy() {
    loop {
        riot_rs::thread::yield_same();
    }
}

#[no_mangle]
fn riot_rs_network_config() -> embassy_net::Config {
    use embassy_net::Ipv4Address;

    embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: embassy_net::Ipv4Cidr::new(Ipv4Address::new(10, 42, 0, 61), 24),
        dns_servers: heapless::Vec::new(),
        gateway: Some(Ipv4Address::new(10, 42, 0, 1)),
    })
}
