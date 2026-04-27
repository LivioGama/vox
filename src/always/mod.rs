//! Always-on voice activation.

pub mod audio;
pub mod config;
pub mod daemon;
pub mod event_loop;
pub mod filter;
pub mod log;
pub mod notify;
pub mod paste;
pub mod text;
pub mod vad;
pub mod context_vocab;
pub mod postprocess;

pub use config::AlwaysConfig;
pub use event_loop::run;
