#![forbid(unsafe_code)]

pub mod actions;
pub mod credentials;
pub mod domain;
pub mod entities;

#[cfg(test)]
mod postgres_tests;
