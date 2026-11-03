pub mod database;
pub mod downloads;
pub mod error;
pub mod java;
pub mod launch;
pub mod minecraft;
pub mod mods;
pub mod models;

#[cfg(feature = "desktop")]
mod app;

#[cfg(feature = "desktop")]
pub fn run() {
    app::run();
}
