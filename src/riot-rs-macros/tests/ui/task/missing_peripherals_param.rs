#![no_main]
#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]
#![feature(used_with_arg)]

// FAIL: the `peripherals` parameter is required in this case
#[riot_rs::task(autostart)]
async fn main(_peripherals: Peripherals) {}

struct Peripherals;
