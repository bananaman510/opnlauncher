# Architecture

## Current application shape

- `src/` contains the React and TypeScript interface.
- `src-tauri/src/app.rs` exposes the small set of Tauri commands used by the interface and owns local process/log handling.
- `src-tauri/src/database.rs` owns the SQLite schema and instance/profile persistence.
- `src-tauri/src/java.rs` discovers and inspects installed Java runtimes.
- `src-tauri/src/minecraft.rs` inspects local game directories and version manifests.
- `src-tauri/src/launch.rs` resolves local version metadata and constructs a Java process plan.
- `src-tauri/src/downloads.rs` reads Mojang's official version manifest, resolves vanilla file metadata, verifies downloads, and installs into a new isolated instance directory. Long network and file operations run on Tauri's blocking worker pool; progress is emitted as events.
- `src-tauri/capabilities/default.json` grants the window only Tauri's core defaults. The frontend has no unrestricted shell, filesystem, or process plugin.

Core Rust logic can be tested without enabling the Tauri desktop feature by using `cargo test --no-default-features --lib`. The desktop executable is built with the `desktop` feature.

## Local data and boundaries

Tauri's app data directory contains `opnlauncher.sqlite3`, logs, managed empty-instance directories, and generated native libraries. The database stores instance names, game directory paths, version IDs, Java paths, memory settings, timestamps, and unverified local developer profile names and UUIDs. The default desktop build enables developer profiles in both development and release configurations.

Registered Minecraft installation directories are read by Rust and are left untouched by version downloads. Downloads create a new managed instance directory, validate relative paths and HTTPS URLs, enforce file and total size limits, verify upstream checksums or declared sizes, write to unique temporary files, and only add an instance record after all files are complete. Launch arguments are derived from local version JSON; OpnLauncher does not invoke a shell or accept arbitrary frontend-provided command strings. Native archives are checked for unsafe paths and symbolic links before extraction.

## Planned separation

Account providers and mod-loader integrations should remain separate from the UI. Microsoft authentication must use supported OAuth and secure OS credential storage. The first vanilla installer does not yet support pause/resume, repair, mod-loader metadata, or a persistent download queue. Mod-provider requests must target documented public APIs, with attribution where required.
