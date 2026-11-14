// source: ProxyAuthK8S@8fe220b (libs/server/api/src/lib.rs), trimmed.
pub mod handlers;
pub mod openapi;
#[cfg(feature = "server")]
pub mod scalar; // templates/crates/web/src/scalar.rs, shared with actix

pub use openapi::*;
#[cfg(feature = "server")]
pub use scalar::*;
