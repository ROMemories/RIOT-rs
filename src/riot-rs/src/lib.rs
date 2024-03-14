//! riot-rs
//!
//! This is a meta-package, pulling in the sub-crates of RIOT-rs.

#![no_std]

pub use riot_rs_buildinfo as buildinfo;
pub use riot_rs_embassy::{self as embassy, define_peripherals};
pub use riot_rs_rt as rt;

// Attribute macros
pub use riot_rs_macros::main;
pub use riot_rs_macros::spawner;
#[cfg(feature = "threading")]
pub use riot_rs_macros::thread;

#[cfg(feature = "threading")]
pub use riot_rs_threads as thread;

// These are used by proc-macros we provide
pub use embassy_executor;
pub use linkme;
pub use static_cell;

// ensure this gets linked
use riot_rs_boards as _;
