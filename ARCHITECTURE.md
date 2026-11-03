# Architecture

## Current application shape

- `src/` contains the React and TypeScript interface.
- `src-tauri/src/app.rs` exposes the small set of Tauri commands used by the interface and owns local process/log handling.
- `src-tauri/src/database.rs` owns the SQLite schema and instance/profile persistence.
- `src-tauri/src/java.rs` discovers and inspects installed Java runtimes.
- `src-tauri/src/loaders.rs` retrieves official Fabric/Forge build metadata, installs Fabric profiles with verified libraries, runs checksum-verified Forge client installers for a selected game folder, and scans installed loader profiles.
- `src-tauri/src/minecraft.rs` inspects local game directories and version manifests.
- `src-tauri/src/launch.rs` resolves local version metadata and constructs a Java process plan.
- `src-tauri/src/downloads.rs` reads Mojang's official version manifest, resolves vanilla file metadata, verifies downloads, and installs into a new isolated instance directory. Long network and file operations run on Tauri's blocking worker pool; progress is emitted as events.
- `src-tauri/src/mods.rs` queries the documented Modrinth and CurseForge APIs, filters projects and files by Minecraft version and loader, inspects existing mod JAR metadata, and stages verified mod files before installing them into the selected instance. The CurseForge API key is passed in memory from the UI and is not persisted.
- `src/pages/ModsPage.tsx` provides provider search, category/sort filters, project artwork and galleries, compatible build selection, dependency information, install progress, and local enable/disable/remove controls.
- `src-tauri/capabilities/default.json` grants the window only Tauri's core defaults. The frontend has no unrestricted shell, filesystem, or process plugin.

Core Rust logic can be tested without enabling the Tauri desktop feature by using `cargo test --no-default-features --lib`. The desktop executable is built with the `desktop` feature.

## Local data and boundaries

Tauri's app data directory contains `opnlauncher.sqlite3`, logs, managed empty-instance directories, and generated native libraries. The database stores instance names, game directory paths, version IDs, Java paths, memory settings, timestamps, and unverified local developer profile names and UUIDs. The default desktop build enables developer profiles in both development and release configurations.

Registered Minecraft installation directories are read by Rust and are left untouched by vanilla version downloads. Vanilla downloads create a new managed instance directory, validate relative paths and HTTPS URLs, enforce file and total size limits, verify upstream checksums or declared sizes, write to unique temporary files, and only add an instance record after all files are complete. Mod downloads are placed in a per-install staging directory inside the selected `mods` folder, validated against trusted HTTPS hosts and provider hashes/sizes, and installed without replacing existing files. Fabric libraries are limited to the launcher and Minecraft library hosts, checked against upstream SHA-1 and sizes, and installed without replacing conflicting files. Forge installation uses the official Maven installer artifact after verifying its published SHA-1; it runs as a direct Java process (no shell) with the selected instance directory as its target, and existing launcher profile configuration is restored after the installer exits. Launch arguments are derived from local version JSON. Native archives are checked for unsafe paths and symbolic links before extraction.

## Planned separation

Account providers and mod-loader integrations remain separate from the UI. Microsoft authentication must use supported OAuth and secure OS credential storage. The vanilla installer does not yet support pause/resume, repair, or a persistent download queue. Loader installation currently supports Fabric and Forge; NeoForge and Quilt profiles can be detected and selected when already installed, but their installers are not implemented. Mod-provider requests target documented public APIs, with artwork loaded only from provider CDN hosts.
