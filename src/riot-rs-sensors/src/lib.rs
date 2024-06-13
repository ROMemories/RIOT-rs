//! Provides a sensor abstraction layer.
//!
//! Sensors must implement the [`Sensor`] trait
#![no_std]
// Required by linkme
#![feature(used_with_arg)]
#![feature(error_in_core)]
#![feature(trait_upcasting)]
// For watcher tasks
#![feature(type_alias_impl_trait)]
#![deny(unused_must_use)]
#![deny(clippy::pedantic)]

mod category;
mod label;
mod physical_unit;
pub mod registry;
pub mod sensor; // FIXME: this should be move to its own crate
pub mod watcher;

pub use category::Category;
pub use label::Label;
pub use physical_unit::PhysicalUnit;
pub use registry::{REGISTRY, SENSOR_REFS};
pub use sensor::{Reading, Sensor};

pub use riot_rs_macros::measure_sensor as measure;
