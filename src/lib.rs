pub mod accounts;
pub mod adapters;
pub mod admin;
mod atomic_file;
pub mod auth;
/// Benchmark-only entry points into crate-private paths (issue #554). Gated so
/// a normal build's public API is unchanged; `cargo bench --features bench`
/// turns it on. See the module docs for why a facade rather than wider `pub`.
#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench_support;
pub mod blueprints;
pub mod codex_analytics;
pub mod codex_endpoint;
pub(crate) mod compression;
pub(crate) mod concurrency;
pub mod config;
pub mod count_tokens;
pub mod dashboard;
pub mod discovery;
pub mod error;
pub mod gateway;
pub mod headers;
pub(crate) mod http_tuning;
pub mod import;
pub mod init;
pub mod keepalive;
pub mod metrics;
pub mod model;
pub mod models_catalog;
pub mod models_prefs;
pub mod oauth_usage;
pub(crate) mod observability;
pub(crate) mod offload;
#[cfg(feature = "tui")]
pub mod opencode_status;
pub mod pool;
pub mod protocol;
pub mod proxy;
pub mod reload;
pub(crate) mod request;
pub mod rerank;
pub mod retry;
pub mod routes;
pub mod routing;
pub mod server;
pub mod state_persist;
pub mod status_poll;
pub mod stream_metrics;
pub mod telemetry;
#[cfg(feature = "tui")]
pub mod tui;
pub mod upstream_status;
pub(crate) mod upstream_timeout;
pub mod usage;
pub mod usage_poll;
pub(crate) mod window_primer;

#[cfg(test)]
mod testutil;
