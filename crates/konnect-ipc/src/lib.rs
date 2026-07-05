#[allow(clippy::all, warnings)]
pub mod gen;

pub mod board;
pub mod builders;
pub mod client;
pub mod types;

pub use client::KiCadIpcClient;
pub use types::*;
