pub mod database;
pub mod downloads;
pub mod error;
pub mod java;
pub mod horizon;
pub mod launch;
pub mod loaders;
pub mod minecraft;
pub mod mods;
pub mod models;
pub mod microsoft_auth;

#[cfg(feature = "desktop")]
mod app;

#[cfg(feature = "desktop")]
pub fn run() {
    app::run();
}
