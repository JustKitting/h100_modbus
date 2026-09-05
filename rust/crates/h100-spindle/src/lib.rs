//! H100 spindle controller and LinuxCNC realtime component.

#![cfg_attr(all(not(debug_assertions), feature = "realtime-component"), no_std)]

#[cfg(feature = "realtime-component")]
mod component;
pub mod sequencer;

#[cfg(feature = "realtime-component")]
pub use component::{rtapi_app_exit, rtapi_app_main};
