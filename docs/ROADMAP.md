# OpnLauncher Roadmap

This roadmap tracks implementation against the project specification. Work is incremental; a milestone is complete only after its behavior is implemented and validated. No milestone implies a public release.

## Current foundation — 0.1.0-dev.1

The repository contains a Tauri 2 desktop shell, React/TypeScript interface, Rust backend, SQLite persistence, Java discovery, existing-installation detection, instance records, development-only unverified profiles, and a local launch-plan resolver. The current launcher expects complete, legitimately installed game files. A real desktop session and in-game launch still need validation.

Completed in this development version:

- Corrected offline test profile UUID generation to match Java's name-based UUID behavior. Existing saved UUIDs remain unchanged.

Remaining follow-up priorities:

- Select a compatible discovered Java runtime when an instance has no explicit Java choice.
- Exercise the launch pipeline against a user-selected installation and capture process exit status and logs.
- Add resource limits to native archive extraction before accepting downloaded or mod-provided archives.
- Add schema-versioned SQLite migrations before the data model grows.

## Planned milestones

### 0.2.0 — Official version installation

- Read and filter the official Minecraft version manifest.
- Resolve version metadata, libraries, natives, and assets for installation.
- Download with bounded concurrency, progress, cancellation, retry, and atomic replacement.
- Verify upstream checksums and safely resume or repair incomplete installations.
- Keep existing working files intact when an installation fails.

### 0.3.0 — Official account authentication

- Implement Microsoft sign-in through the system browser and the supported Xbox/Minecraft authentication flow.
- Verify Minecraft ownership and retrieve the Java profile.
- Store tokens in operating-system credential storage where available; never log or send them to an OpnLauncher server.
- Keep development-only unverified profiles distinct from authenticated accounts.

### 0.4.0 — Isolated instances and Fabric

- Give instances independent game directories and configurable Java, memory, JVM arguments, resolution, and launch environment.
- Add explicit delete confirmation and a preserve-worlds choice; keep forget-record behavior non-destructive.
- Install Fabric incrementally and manage local mods with compatibility information.
- Add a provider boundary for Modrinth search and compatible project installation, including attribution and rate limits.

### 0.5.0 — Modpacks and additional loaders

- Import and export supported Modrinth modpacks with manifest validation and dependency checks.
- Detect local changes and back up before updates; never silently overwrite user data.
- Add Quilt, NeoForge, and Forge in separate, testable steps.
- Add mod enable/disable, update, dependency, and loader-compatibility workflows.

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
