//! Provides a sensor abstraction layer.
//!
//! Sensors must implement the [`Sensor`] trait, and must be registered into the
//! [`static@SENSOR_REFS`] [distributed slice](linkme).

#![no_std]
// Required by linkme
#![feature(used_with_arg)]
#![feature(error_in_core)]
#![deny(unused_must_use)]
#![deny(clippy::pedantic)]

pub mod categories;
pub mod physical_unit;
pub mod registry;
pub mod sensor;

pub use physical_unit::PhysicalUnit;
pub use registry::{REGISTRY, SENSOR_REFS};
pub use sensor::{Reading, Sensor};
