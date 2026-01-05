//! ZRC Agent - Host agent for remote control
//!
//! This crate provides the host-side agent that enables remote desktop control.

pub mod audit;
pub mod capture;
pub mod clipboard;
pub mod config;
pub mod consent;
pub mod file_transfer;
pub mod identity;
pub mod input;
pub mod media_transport;
pub mod pairing;
pub mod policy;
pub mod replay;
pub mod service;
pub mod session;
pub mod signaling;
