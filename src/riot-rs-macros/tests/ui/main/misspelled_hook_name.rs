#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(used_with_arg)]

// As the macro will fail, this import will not get used
#[allow(unused_imports)]
use riot_rs::embassy::usb::UsbBuilderHook;

// FAIL: misspelled hook name
#[riot_rs::main(usb_builder_hooook)]
async fn main() {}
