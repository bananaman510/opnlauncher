# OpnLauncher Roadmap

This roadmap tracks implementation against the project specification. Work is incremental; a milestone is complete only after its behavior is implemented and validated. No milestone implies a public release.

## Current foundation — 0.1.0-dev.8

The repository contains a Tauri 2 desktop shell, React/TypeScript interface, Rust backend, SQLite persistence, Java discovery, existing-installation detection, instance records, development-only unverified profiles, a local launch-plan resolver, and an initial official vanilla version installer. The installer has not yet been exercised against live Mojang files, and a real desktop session and in-game launch still need validation.

Completed in this development version:

- Corrected offline test profile UUID generation to match Java's name-based UUID behavior. Existing saved UUIDs remain unchanged.
- Added the official Minecraft version browser and isolated vanilla version downloads with progress, cancellation, retries, and upstream integrity checks.
- Moved network and file transfer work off the UI command thread.
- Updated app data and privacy documentation for version downloads.
- Reorganized desktop navigation and added categorized appearance, Java, loader, storage, privacy, and about settings.
- Added validated custom Java paths and saved defaults for the Java runtime and memory used by new instances.
- Added compatible Fabric and Forge version browsing, Fabric profile installation, verified Forge installer execution, and local profile switching.
- Kept loader profile switching additive: worlds and mods remain in the selected game folder.
- Added per-instance Minecraft and loader selection followed by additive setup installation; the Mods page uses each selected instance's detected metadata.
- Fixed Fabric profile installation for coordinate-and-repository metadata by resolving Maven artifacts and checking their SHA-1 sidecars.

Remaining follow-up priorities:

- Select a compatible discovered Java runtime when an instance has no explicit Java choice.
- Validate a complete vanilla download and launch using legitimate game files and a development profile.
- Add clear launch messaging and compatible Java selection for the chosen version.
- Add resource limits to native archive extraction before accepting downloaded or mod-provided archives.
- Add schema-versioned SQLite migrations before the data model grows.

## Planned milestones

### 0.2.0 — Installation management

- Harden the initial vanilla installer with live-download validation, repair, and recovery from interrupted installs.
- Add a persistent download queue, configurable concurrency, and safe pause/resume where supported.
- Add support and tests for historical version metadata formats that the initial installer does not handle.
- Keep existing working files intact when an installation fails.

### 0.3.0 — Official account authentication

- Implement Microsoft sign-in through the system browser and the supported Xbox/Minecraft authentication flow.
- Verify Minecraft ownership and retrieve the Java profile.
- Store tokens in operating-system credential storage where available; never log or send them to an OpnLauncher server.
- Keep development-only unverified profiles distinct from authenticated accounts.

### 0.4.0 — Isolated instances and loader validation

- Give instances independent game directories and configurable Java, memory, JVM arguments, resolution, and launch environment.
- Add explicit delete confirmation and a preserve-worlds choice; keep forget-record behavior non-destructive.
- Validate Fabric and Forge installation and mod compatibility against live Minecraft launches; improve repair and recovery paths.
- Complete the Modrinth provider workflow with rate-limit handling, compatible project installation, and attribution.

### 0.5.0 — Modpacks and additional loaders

- Import and export supported Modrinth modpacks with manifest validation and dependency checks.
- Detect local changes and back up before updates; never silently overwrite user data.
- Add Quilt and NeoForge in separate, testable steps.
- Add mod update, dependency, and loader-compatibility workflows.

### 0.6.0 — Recovery and customization

- Add manual instance, world, and configuration backups with integrity checks and restore previews.
- Add safe instance import/export and orphaned-directory recovery.
- Add editable appearance, language, Java defaults, network, accessibility, and backup settings.
- Improve local log viewing and redact credentials from manually exported diagnostics.

### 0.7.0–0.9.0 — Distribution readiness

- Add automated formatting, lint, type-check, unit/integration test, and Linux build workflows.
- Validate Windows and macOS behavior and packaging as practical.
- Validate Linux AppImage and Flatpak behavior on supported desktop environments.
- Review Tauri capabilities, archive handling, dependencies, privacy behavior, documentation, and reproducibility.
- Prepare contribution, security, privacy, license, and Minecraft disclaimer materials before any public publication.

### 1.0.0 — Stable release

Only consider a stable release after authenticated launching, reliable installation and instance management, tested distribution packages, security review, complete documentation, and no known critical defects. Stable version selection, release tags, and publication require explicit approval.

## Release and repository policy

- Use semantic versions and keep the package, Rust, and displayed application versions aligned.
- Maintain the changelog as features land.
- The private Forgejo repository remains the development remote. Do not mirror or publish to GitHub automatically.
- Do not create release tags or publish a release without explicit authorization.
