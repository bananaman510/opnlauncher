# OpnLauncher 0.1.0-dev.16 — development preview

**This is an early development pre-release, not a stable or production release.** It is shared to make the work visible, invite feedback, and welcome potential contributors. Expect missing features, rough edges, and changes to behavior or local data. Back up important worlds before trying it.

## Included

- Tauri 2 desktop launcher built with Rust, React, TypeScript, and SQLite.
- Official Minecraft version browsing and installation into managed instance folders.
- Per-instance vanilla, Fabric, or Forge setup, with Java and memory settings.
- Modrinth and CurseForge browsing for the selected Minecraft version and loader, including compatible build selection, project artwork, dependency handling, and local mod management.
- Local developer profiles in development and release builds. They are clearly unverified and do not authenticate or verify game ownership.
- Experimental Horizon Rust-engine profiles for Minecraft 26.3, with local profiles, official resource preparation, separate worlds/settings, and `.hmod` content management.
- Local-only settings, logs, and diagnostics; no analytics, ads, telemetry, automatic diagnostic uploads, or OpnLauncher account.
- Linux AppImage build support. Automated workflows run checks, and an explicitly started AppImage workflow saves a temporary artifact without creating a Git tag or release.
- Updated the development test tooling to a patched Vitest release after the initial npm audit found critical and high advisories in the prior test-tool dependency tree.

## Important limits

- Microsoft OAuth, Minecraft ownership/entitlement checks, authenticated online play, and secure account-token storage are not implemented. Local profiles are for offline single-player and appropriately configured local testing only.
- Horizon is a separate native engine for Minecraft 26.3. Java `.jar` mods and Fabric/Forge loaders do not run on Horizon; Java-to-Horizon mod compatibility is not included.
- Modpack management, NeoForge and Quilt installation, resource/shader packs, backups, instance import/export, automatic updates, pause/resume, and a complete repair flow are unavailable.
- Linux is the current development target. Windows/macOS builds and behavior have not been validated.
- This pre-release has not received a complete security audit. Minecraft mods are third-party executable code; only install content you trust.
- Linux-target Rust license metadata was reviewed; Windows/macOS target-specific dependency metadata still needs review before a stable release.

## Privacy and network activity

The launcher has no telemetry or OpnLauncher-operated service. When the corresponding feature is used, it contacts Mojang for official metadata/files, Fabric or Forge for loader data, and Modrinth or CurseForge for mod content. CurseForge requires the user's own API key, held in app memory for the current session. See [PRIVACY.md](../PRIVACY.md) for details.

## Getting started

Use the Linux setup and first-launch instructions in [README.md](../README.md). Back up worlds and review the selected instance path before installing files or mods.
