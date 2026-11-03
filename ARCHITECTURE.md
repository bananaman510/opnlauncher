# Architecture

## Current application shape

- `src/` contains the React and TypeScript interface.
- `src-tauri/src/app.rs` exposes the small set of Tauri commands used by the interface and owns local process/log handling.
- `src-tauri/src/database.rs` owns the SQLite schema and instance/profile persistence, including per-instance provider/version references for installed mod files.
- `src-tauri/src/java.rs` discovers and inspects installed Java runtimes.
- `src-tauri/src/loaders.rs` retrieves official Fabric/Forge build metadata, installs Fabric profiles with verified libraries, runs checksum-verified Forge client installers for a selected game folder, and scans installed loader profiles.
- `src-tauri/src/minecraft.rs` inspects local game directories and version manifests.
- `src-tauri/src/launch.rs` resolves local version metadata and constructs a Java process plan.
- `src-tauri/src/downloads.rs` reads Mojang's official version manifest, resolves vanilla file metadata, verifies downloads, and installs into a new isolated instance directory. Long network and file operations run on Tauri's blocking worker pool; progress is emitted as events.
- `src-tauri/src/horizon.rs` validates the local Horizon Launch API, prepares an offline client process, and manages per-instance `.hmod` content files. Horizon resource preparation reuses the verified Mojang downloader and stages prepared resources before replacing its shared cache.
- `src-tauri/src/mods.rs` queries the documented Modrinth and CurseForge APIs, filters projects and files by Minecraft version and loader, inspects existing mod JAR metadata, identifies older Modrinth installs by SHA-1 lookup on demand, verifies saved dependency versions and local integrity before reuse, and stages verified installs or replacements before changing files. The CurseForge API key is passed in memory from the UI and is not persisted.
- `src/pages/ModsPage.tsx` provides provider search, category/sort filters, project artwork and galleries, duplicate-install recognition, compatible build selection, dependency information, install progress, and local enable/disable, version-switch, multi-select, and confirmed bulk-removal controls.
- `src/pages/ModVersionSwitchDialog.tsx` lists project releases with search, Minecraft version, loader, and release-type filters plus date/version sorting; it blocks incompatible selections for the active instance.
- `src-tauri/capabilities/default.json` grants the window only Tauri's core defaults. The frontend has no unrestricted shell, filesystem, or process plugin.

Core Rust logic can be tested without enabling the Tauri desktop feature by using `cargo test --no-default-features --lib`. The desktop executable is built with the `desktop` feature.

## Local data and boundaries

Tauri's app data directory contains `opnlauncher.sqlite3`, logs, managed instance directories, generated native libraries, and Horizon's shared Minecraft 26.3 files and prepared resources. The database stores instance names, engine kind, game directory paths, version IDs, Java paths, memory settings, timestamps, and unverified local developer profile names and UUIDs. The default desktop build enables developer profiles in both development and release configurations.

Registered Minecraft installation directories are read by Rust and are left untouched by downloads from the general vanilla version browser. Per-instance setup can add a selected official Minecraft version into the chosen game folder, validating relative paths and HTTPS URLs, enforcing file and total size limits, verifying upstream checksums or declared sizes, and refusing to replace conflicting files. Mod downloads are placed in a per-install staging directory inside the selected `mods` folder, validated against trusted HTTPS hosts and provider hashes/sizes, and installed without replacing existing files. Mod version switches stage and verify the new build first, move the old root JAR into that staging directory, restore it if adding the replacement fails, and then update the local provider/version mapping. Bulk removal validates every selected JAR before removing any. Fabric libraries are resolved from either profile artifact records or safe Maven coordinates, restricted to launcher and Minecraft library hosts, checked against profile or Maven SHA-1 values, size bounded, and installed without replacing conflicting files. Forge installation uses the official Maven installer artifact after verifying its published SHA-1; it runs as a direct Java process (no shell) with the selected instance directory as its target, and existing launcher profile configuration is restored after the installer exits. The instance database stores the active version profile ID; Minecraft and loader versions are resolved from that profile's metadata, avoiding duplicate version settings. Launch arguments are derived from local version JSON. Native archives are checked for unsafe paths and symbolic links before extraction.

## Planned separation

Account providers and mod-loader integrations remain separate from the UI. Microsoft authentication must use supported OAuth and secure OS credential storage. The vanilla installer does not yet support pause/resume, repair, or a persistent download queue. Loader installation currently supports Fabric and Forge; NeoForge and Quilt profiles can be detected and selected when already installed, but their installers are not implemented. Mod-provider requests target documented public APIs, with artwork loaded only from provider CDN hosts.
