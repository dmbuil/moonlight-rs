//! One module per page or family of pages (Horizon's panels and detail pages). Pages load
//! through `#[server]` fns in `api.rs`, render with `components/`, and act through
//! `components/ops`; their routes are in `app.rs`.

pub mod admin;
pub mod alarms;
pub mod details;
pub mod firewall;
pub mod floating_ip;
pub mod heat_catalog;
pub mod identity;
pub mod instance_detail;
pub mod instances;
pub mod key_manager;
pub mod lists;
pub mod load_balancers;
pub mod login;
pub mod overview;
pub mod qos;
pub mod quotas;
pub mod stack_launch;
pub mod stacks;
pub mod template_generator;
pub mod topology;
pub mod trunks;
pub mod volume_groups;
