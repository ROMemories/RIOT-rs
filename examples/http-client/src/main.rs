#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(used_with_arg)]

mod pins;

use embassy_time::{Duration, Timer};
use reqwless::{client::HttpClient, request::Method};
use riot_rs::{
    debug::log::info,
    embassy_net::{
        self,
        dns::DnsSocket,
        tcp::client::{TcpClient, TcpClientState},
    },
    network,
};

const MAX_CONCURRENT_CONNECTIONS: usize = 2;
const TCP_BUFFER_SIZE: usize = 1024;
const HTTP_BUFFER_SIZE: usize = 1024;

#[riot_rs::task(autostart)]
async fn main() {
    const URL: &str = env!("URL");

    let stack = network::network_stack().await.unwrap();

    let tcp_client_state =
        TcpClientState::<MAX_CONCURRENT_CONNECTIONS, TCP_BUFFER_SIZE, TCP_BUFFER_SIZE>::new();
    let tcp_client = TcpClient::new(&stack, &tcp_client_state);
    let dns_client = DnsSocket::new(&stack);

    let mut client = HttpClient::new(&tcp_client, &dns_client);
    let mut http_rx_buf = [0; HTTP_BUFFER_SIZE];

    loop {
        if let Ok(mut handle) = client.request(Method::GET, URL).await {
            let response = handle.send(&mut http_rx_buf).await.unwrap();
            info!("Response status: {}", response.status.0);
            let content_type = response.content_type.as_ref().unwrap().as_str();
            info!("Response Content-Type: {}", content_type);
            let body = response.body().read_to_end().await.unwrap();
            info!("{:x}", body);
        }

        // Wait a bit before retrying/making a new request
        Timer::after(Duration::from_secs(3)).await;
    }
}

#[riot_rs::config(network)]
fn network_config() -> embassy_net::Config {
    use embassy_net::Ipv4Address;

    embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: embassy_net::Ipv4Cidr::new(Ipv4Address::new(10, 42, 0, 61), 24),
        dns_servers: heapless::Vec::new(),
        gateway: Some(Ipv4Address::new(10, 42, 0, 1)),
    })
}

#[cfg(capability = "hw/usb-device-port")]
#[riot_rs::config(usb)]
fn usb_config() -> riot_rs::embassy_usb::Config<'static> {
    let mut config = riot_rs::embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("HTTP-over-USB-Ethernet example");
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
