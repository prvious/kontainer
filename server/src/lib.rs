#![forbid(unsafe_code)]

/// Tracing target for audit records. It starts with the crate name so Loco's default log filter,
/// which passes its own whitelist plus `<crate>=<level>`, keeps them without a custom filter.
pub const AUDIT_TARGET: &str = concat!(env!("CARGO_CRATE_NAME"), "::audit");

pub mod app;
pub mod auth;
pub mod containers;
pub mod controllers;
pub mod exec;
pub mod health;
pub mod images;
pub mod logs;
pub mod metrics;
pub mod networks;
pub mod nodes;
pub mod operations;
pub mod rpc;
pub mod server;
pub mod services;
pub mod settings;
pub mod stacks;
pub mod stream_slots;
pub mod swarm;
pub mod tasks;
pub mod volumes;

#[cfg(test)]
pub mod test_support;

#[cfg(test)]
mod audit_tests {
    use loco_rs::app::Hooks;

    #[test]
    fn audit_target_sits_under_the_app_name_locos_default_filter_enables() {
        // Without RUST_LOG or override_filter, Loco enables `<app_name>=<level>` plus its own
        // whitelist; a target outside that prefix is silently dropped.
        let app = crate::app::App::app_name();
        assert!(
            crate::AUDIT_TARGET.starts_with(&format!("{app}::")),
            "{} must start with {app}::",
            crate::AUDIT_TARGET
        );
    }
}
