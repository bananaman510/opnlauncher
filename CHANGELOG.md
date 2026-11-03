# Changelog

All notable project changes are recorded here. This project follows Semantic Versioning; versions are not tagged or released automatically.

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
