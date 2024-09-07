#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(used_with_arg)]

mod pins;

use embassy_time::{Duration, Timer};
use reqwless::{
    client::HttpClient,
    headers::ContentType,
    request::{Method, RequestBuilder},
};
use riot_rs::{
    debug::log::debug,
    embassy_net::{
        self,
        dns::DnsSocket,
        tcp::client::{TcpClient, TcpClientState},
    },
    network,
};

const MAX_CONCURRENT_CONNECTIONS: usize = 2;

#[riot_rs::task(autostart)]
async fn main() {
    const URL: &str = env!("URL");

    let stack = network::network_stack().await.unwrap();

    // Wait for the network connect to be up (hopefully).
    // FIXME: do something smarter
    Timer::after(Duration::from_secs(5)).await;

    let tcp_client_state = TcpClientState::<MAX_CONCURRENT_CONNECTIONS, 1024, 1024>::new();
    let tcp_client = TcpClient::new(&stack, &tcp_client_state);
    let dns_client = DnsSocket::new(&stack);

    let mut client = HttpClient::new(&tcp_client, &dns_client);
    let mut rx_buf = [0; 1024];
    let response = client
        .request(Method::POST, URL)
        .await
        .unwrap()
        .body(b"PING".as_slice())
        .content_type(ContentType::TextPlain)
        .send(&mut rx_buf)
        .await
        .unwrap();
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
