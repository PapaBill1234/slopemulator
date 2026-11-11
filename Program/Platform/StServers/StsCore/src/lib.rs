//! Shared core of the Rust StServers port (build 381 parity target).
//! Specs: Docs/Platform/StServers/11–13. Secrets are never stored here;
//! configs use the placeholder `keyhere`.
pub mod conn;
pub mod connect;
pub mod error;
pub mod guid;
pub mod ini;
pub mod route;
pub mod route_client;
pub mod secrets;
pub mod service;
pub mod xml;

pub use error::StsError;
pub use xml::Element;

/// Placeholder for every key, password and secret in the port.
pub const SECRET_PLACEHOLDER: &str = "keyhere";
