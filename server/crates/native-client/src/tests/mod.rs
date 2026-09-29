//! In-crate verification for the native client.
//!
//! These tests drive the public API and the private exchange path against
//! real loopback TCP/TLS fixture servers: no handshake, redirect, reset, or
//! timeout outcome is mocked.

mod auth;
mod credentials;
mod images;
mod leakage;
mod logs;
mod networks;
mod operations;
mod password_auth;
mod reads;
mod retry;
mod services;
mod stacks;
mod support;
mod transport;
mod volumes;
