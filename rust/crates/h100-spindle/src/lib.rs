//! H100 spindle controller and LinuxCNC realtime component.

#![cfg_attr(not(debug_assertions), no_std)]

mod component;
pub mod sequencer;

pub use component::{rtapi_app_exit, rtapi_app_main};
