# Changelog

All notable project changes are recorded here. This project follows Semantic Versioning; versions are not tagged or released automatically.

## [0.1.0-dev.14] - Unreleased

### Added

- Integrated Horizon's Rust client source and resource preparer source into OpnLauncher at upstream commit `fdd290b99b4459cb4803487758ba8015a45aa47f`; the native game runtime is built and bundled with the launcher.
- Replaced user-configured Horizon executable paths with an included engine status and automatic resource-folder management. Resource preparation now calls Horizon's Rust library directly.
- Preserved Horizon's MIT license and third-party notices, and documented the vendored source boundary and upstream revision.
- Removed Horizon executable discovery from `PATH`; the launcher no longer asks users to select a separate client or preparer.

### Improved

- Kept Horizon's game process separate from the Tauri interface, with the bundled runtime forced to disable Horizon telemetry and play pings.
- Continued using only local, unverified profiles for Horizon; no Microsoft sign-in or account tokens are introduced.

## [0.1.0-dev.13] - Unreleased

### Improved

- Reworked the main navigation to keep readable section labels visible on desktop and tablet, with a labeled horizontal navigation on narrow windows.
- Fixed Horizon settings inheriting the standard settings menu's narrow fixed-width column; Horizon setup now fills the available page width and uses a two-column card layout on wide windows.
- Improved Horizon setup spacing, field sizing, and responsive path controls; the sidebar navigation can scroll independently on short windows.
- Replaced the README with a complete Linux setup, AppImage build, first-run, instance, mod, Horizon, privacy, storage, and development guide, including current feature limits.

## [0.1.0-dev.12] - Unreleased

### Improved

- Expanded automatic Minecraft discovery to the official launcher directory and common PrismLauncher, PolyMC, MultiMC, ATLauncher, and GDLauncher instance folders across supported desktop platforms.
- Added Horizon settings to scan for local Minecraft 26.3 folders, add a custom folder, and select which installation can supply reusable files.
- Reuse only Minecraft files that match Mojang's published checksums, copying them into OpnLauncher's private Horizon cache; missing files are downloaded and source installations are left unchanged.
- Discover Horizon client and preparation tools on `PATH` at startup, with detected tool choices and an in-app rescan alongside Java and Minecraft discovery.

## [0.1.0-dev.11] - Unreleased

### Added

- Added a separate Horizon Rust-engine instance flow for Minecraft 26.3, with local profile launches, isolated per-instance settings, and a distinct Horizon content-mod page.
- Added official Minecraft 26.3 file preparation for Horizon, including the server artifact Horizon's preparer requires; existing client, library, and asset files are reused only when their upstream integrity data matches.
- Kept Horizon launches unauthenticated and unverified, and disable Horizon telemetry and play pings for both launch and preparation processes.
- Added Horizon setup documentation covering local executable and JDK 25 requirements, file storage, offline boundaries, and current experimental limitations.

## [0.1.0-dev.10] - Unreleased

### Fixed

- Reuse a dependency already installed in the selected instance when its provider version supports the active Minecraft version and mod loader and its local file still matches upstream integrity data.
- Keep installed dependency mappings and files untouched when reusing them; explain when a dependency is disabled, pinned to a different version, incompatible, or no longer matches its provider checksum.
- Identify older untracked Modrinth dependencies by local SHA-1 lookup when installing a project that requires them.

## [0.1.0-dev.9] - Unreleased

### Added

- Added per-mod version switching with searchable releases, Minecraft version and loader filters, release-type filters, sorting, compatibility checks, checksum-verified downloads, and a confirmation step.
- Added multi-select, select-all, delete-selected, and delete-all controls for the selected instance's mod files, each with an explicit confirmation.
- Disabled Discover and project-detail install buttons for projects already detected in the selected instance.
- Saved provider project and version references for installed mods so later version changes can resolve the correct project.

### Fixed

- Fixed version switching when the provider changes a mod's JAR filename: the new file is staged and verified before the previous file is moved aside, and the previous file is restored if installation fails.
- Improved recognition of pre-existing mods so duplicate installs are less likely when their filenames include a loader and version suffix.

## [0.1.0-dev.8] - Unreleased

### Added

- Added per-instance game setup so each instance can choose a Minecraft version and Vanilla, Fabric, or Forge before installing the selected setup into its game folder.
- Added a setup path for new empty instances and a Configure control on every instance. The selected Minecraft and loader metadata now drives the existing Mods page compatibility filters.
- Kept setup downloads additive: existing files must pass upstream integrity checks before reuse, conflicting files are left unchanged, and existing saves and mods remain in place.

### Fixed

- Fixed Fabric installation for profile JSON that lists Maven coordinates and a repository URL instead of a `downloads.artifact` object. Fabric artifacts now resolve to safe Maven paths and are verified with profile checksums or same-repository SHA-1 sidecars.
- Added compatible instance setup downloads for existing, non-empty game folders without requiring the folder to be cleared first.

## [0.1.0-dev.7] - Unreleased

### Added

- Reorganized settings into clear General, Appearance, Java manager, Mod loaders, Storage, Privacy, and About sections, with a compact icon navigation rail and a charcoal-and-violet default theme.
- Added local defaults for new instances, including preferred Java runtime and memory allocation, plus validation and registration for custom Java executables.
- Added compatible Fabric and Forge build lists, Fabric profile installation, Forge installer checksum verification and per-instance installation, and switching between loader profiles already installed in a game folder.
- Added tests for loader profile detection, path validation, version ordering, and instance version switching.

### Security and privacy

- Fabric library downloads are restricted to approved HTTPS hosts, checked against upstream SHA-1 values and sizes, and never overwrite a different existing library.
- Forge installers are fetched from Forge's Maven repository, checked against the published SHA-1 before execution, run for the selected game folder without a shell, and do not retain changes to `launcher_profiles.json`.
- Documented the new loader metadata and installer network requests.

## [0.1.0-dev.6] - Unreleased

### Improved

- Added one-click installation to mod cards. OpnLauncher selects the newest compatible stable build for the active instance's Minecraft version and loader, falling back to a compatible preview if needed.
- Added per-card version lookup, install progress, and success or error feedback while reusing dependency handling, integrity checks, and no-overwrite safeguards.
- Added tests for exact Minecraft and loader compatibility plus stable and preview version selection.

## [0.1.0-dev.5] - Unreleased

### Improved

- Increased small interface text sizes across the launcher for easier reading.
- Made mod project screenshots more prominent in search cards and project icons visible as large artwork when a gallery image is unavailable.

### Fixed

- Parsed Modrinth search gallery URLs, which are returned as strings, so project screenshots now appear in search results. Featured screenshots are shown first while image-host validation remains enforced.

## [0.1.0-dev.4] - Unreleased

### Added

- Added a dedicated Modrinth and CurseForge mod browser with Minecraft version and Fabric, Forge, NeoForge, or Quilt filtering, search, category chips, sorting, project art, and screenshot galleries.
- Added compatible build details, required dependency installation, staged downloads, progress and cancellation, provider checksum/size verification, and no-overwrite installation.
- Added local mod scanning plus enable, disable, and confirmed removal controls for the selected instance.
- Kept the CurseForge API key session-only and added provider network/image disclosures.
- Added mod-loader detection from installed version metadata and regression tests for loader, URL, and file-name validation.

## [0.1.0-dev.3] - Unreleased

### Added

- Rebuilt the desktop interface with a responsive overview, instance library, clearer navigation, and dedicated account and settings pages.
- Added local theme and accent preferences, instance favorites, recent-instance sorting, grid and list layouts, and a keyboard shortcut for instance search.
- Added a local profile manager that keeps development profiles clearly marked as unverified and available in release builds.
- Replaced browser-native instance dialogs with in-app rename, removal, and creation flows.
- Improved Java runtime selection with readable labels and the executable path shown separately.

## [0.1.0-dev.2] - Unreleased

### Added

- Initial Tauri 2, React, TypeScript, and Rust application scaffold.
- Dark local-first desktop interface and primary navigation.
- SQLite persistence for instances and visibly unverified local developer profiles.
- Common Minecraft directory discovery and local version metadata inspection.
- Java runtime discovery from environment and standard Linux paths.
- Instance registration, empty managed instance creation, rename, Java selection, and memory settings.
- Initial local launch argument resolution for installed Minecraft metadata, including version inheritance, library classpaths, native extraction safeguards, and process logs.
- Basic Rust and frontend unit tests.
- Linux development, privacy, security, and architecture documentation.
- Corrected development profile UUIDs to match Java's name-based UUID behavior for new profiles.
- Added a feature audit and an incremental implementation roadmap.
- Added a searchable official Minecraft version browser for releases, snapshots, and old alpha/beta versions.
- Added isolated vanilla version installation with metadata and asset resolution, bounded parallel downloads, SHA-1/size verification, retries, cancellation, and progress reporting.
- Kept downloads in a new managed instance directory and create the instance record only after its files finish successfully.
- Moved version-list and installation work onto Tauri's blocking worker pool so network transfers do not block the desktop UI.
- Corrected versioned Maven library paths and launch asset substitutions used by downloaded versions.
- Updated privacy, architecture, Linux setup, and roadmap documentation for the new Mojang network requests.
- Fixed version manifest validation for official IDs containing spaces or Unicode while keeping unsafe path and Windows device names rejected.
- Fixed a race when parallel downloads create shared parent directories.
- Enabled local developer profiles in default release builds for single-player and local testing.
