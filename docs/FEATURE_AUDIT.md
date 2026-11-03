# OpnLauncher Feature Audit

Audit date: 2026-11-03
Scope: repository source and configuration only. No installed Minecraft directories or saved worlds were inspected.

## Summary

OpnLauncher is a Tauri 2 desktop prototype using Rust, React, TypeScript, Vite, and SQLite. It has an official vanilla version browser and installer, Modrinth and CurseForge mod browsing, Fabric and Forge loader installation for a selected instance, and an experimental integrated Horizon engine. Loader and mod files are checked before installation. The maintainer has reported successful local Java and Horizon launches; this source review does not independently reproduce those sessions or perform live provider downloads. Microsoft authentication is not implemented.

The implementation is small and can be extended without replacing its architecture. The launch path has useful unit coverage for inheritance, rules, argument expansion, and path validation, but the current automated checks do not exercise a real installed Minecraft version in a desktop session. A prior automation session could not initialize GTK, while the maintainer later reported a working local AppImage and Horizon launch. Treat those as maintainer-reported results rather than an independently reproduced or cross-platform guarantee.

## Existing implementation

| Area | Current implementation |
| --- | --- |
| Desktop shell | Tauri 2, Rust backend, React 19, TypeScript, Vite, and a custom CSS dark theme. Linux is the current development target. |
| Frontend | Home and instance views, per-instance Minecraft and loader setup, installation inspection and registration, official version search/filter/install flow, Modrinth/CurseForge mod browsing matched to the selected instance, compatible build selection, mod file management, download progress/cancellation, per-instance Java and memory controls, local developer profiles, logs, and categorized settings for defaults, appearance, Java, loaders, storage, privacy, and about. |
| Persistence | SQLite database in Tauri's application data directory; stores instances, development profiles, mod provider/version references, and a settings table. The connection is serialized through a mutex. |
| Java | Discovers runtimes from `JAVA_HOME`, `PATH`, common Linux paths, and standard executable locations; parses version, vendor, and architecture. Launch planning checks the selected runtime against the version's required Java major. |
| Loader management | Per-instance setup lists compatible Fabric versions from Fabric Meta and Forge builds from Forge Maven, then installs the selected setup into that instance's folder. Fabric profile libraries with Maven coordinates are derived safely and verified with profile or repository SHA-1 values; Forge installers are checked against their published SHA-1 and run directly for the selected game directory. Existing Fabric, Forge, NeoForge, and Quilt profiles can be selected. |
| Installation detection | Reads common Minecraft directories, `MINECRAFT_DIR`, and user-selected local paths. Validates version JSON IDs and sorts discovered versions. Registered directories remain separate from managed downloaded instances. |
| Vanilla version installation | Fetches Mojang's version manifest on request, filters release/snapshot/old alpha/beta entries, resolves supported vanilla metadata, libraries, native classifiers, logging configs, and assets, then downloads into a new instance directory with HTTPS, size/SHA-1 checks, retries, bounded parallelism, cancellation, and progress. Very old metadata formats may not work; there is no pause/resume or repair yet. |
| Mod browser and management | Queries Modrinth and CurseForge for mods matched to the instance Minecraft version and loader; shows project images/gallery, descriptions, categories, compatible builds and required dependencies. Installs verified mod files with progress and cancellation; scans JAR metadata and supports duplicate-aware install buttons, enable/disable, sortable/filterable per-mod version switching, and confirmed single or bulk removal. CurseForge needs the user's own session-only API key. |
| Instance records | Create empty managed directories, register an existing installation/version, rename, forget the launcher record, set Java, and set memory. Existing installations are registered by their shared game-directory path, so they do not yet have independent saves or mods. |
| Launch resolver | Resolves inherited version metadata, libraries, JVM/game arguments, OS rules, classpath, native archives, Java compatibility, and logging configuration from local files. Starts Java directly without a shell and streams output to local logs/events. |
| Archive handling | Native archive extraction rejects unsafe paths and symbolic links. It currently has no expanded-size or entry-count limits. |
| Developer profiles | Unverified local profiles are enabled by the default desktop feature set in development and release builds. They are clearly labeled and do not authenticate accounts or verify ownership. |
| Tauri security | The frontend has only `core:default`; it has no unrestricted shell, filesystem, or process plugin. |
| Privacy | No telemetry, ads, crash-reporting service, or automatic log upload. Mojang, Fabric, Forge, Modrinth, and CurseForge calls occur only during related feature use. Project images load from approved provider CDN hosts. The CurseForge key exists only in app memory and is sent only to CurseForge. |

## Partial or unavailable features

- Modpacks and authenticated Accounts are placeholders. The loader manager installs Fabric and Forge and recognizes installed NeoForge or Quilt profiles; those additional loader installers are not implemented. Downloads shows the current or most recent install progress. There is no persistent queue, pause/resume, or repair flow.
- The vanilla installer handles official Minecraft metadata and files. Fabric and Forge installer flows have not been tested against live metadata or game files in this source-only validation.
- Microsoft OAuth, ownership checks, account switching, token refresh, and OS credential storage are not implemented.
- The launch resolver supports the metadata forms represented in its code and tests; it has no end-to-end coverage across vanilla releases or mod-loader metadata.
- Java selection is manual per instance. The fallback uses the first discovered runtime, which can be incompatible even when another compatible runtime is installed.
- Instance isolation, JVM arguments, resolution, environment variables, import/export, backups, tags, playtime, and file browsing are not implemented.
- Fabric and Forge installation are implemented; modpack import/update, NeoForge and Quilt installation, resource/shader pack management, and automatic mod updates are not implemented.
- Settings include theme/accent, preferred and custom Java runtimes, new-instance memory defaults, loader management, local storage summaries, privacy details, and about. Language, download concurrency, proxy, accessibility, backup, and update controls are not implemented.
- Linux CI, dependency audit workflows, and a manual AppImage artifact workflow are configured. They do not publish Git tags or GitHub releases. There is no application updater or Windows/macOS validation.

## Tests and validation

- Rust unit tests cover SQLite instance/profile/version switching and mod source records, Java version parsing, local installation inspection, version ordering, launch metadata merging, argument rules, path validation, OS matching, Java version defaults, mod-loader detection, loader path/version helpers, CurseForge Minecraft-version filtering, provider query encoding, safe provider/image/file URLs, and replacement when a mod version uses a new filename.
- Frontend tests cover local date/time and Java labels, compatible mod-version selection, and duplicate mod detection.
- There are no Tauri command integration tests, real-game launch tests, UI automation tests, live installer tests, or cross-platform tests. Installer helper tests validate metadata/path planning but do not contact Mojang.
- On the preparation branch, the frontend production build passed and all 10 frontend tests passed with Vitest 5.1.0. `npm audit --audit-level=low` reported zero vulnerabilities after updating the development test runner.
- The Rust library suite passed all 59 tests, the Linux Tauri desktop backend check passed, and the bundled Horizon runtime built with Linux audio enabled. The Linux AppImage was produced and its bundled license/attribution resources were inspected; the application was not launched from that artifact during this review.
- The CI, security-audit, and manual AppImage workflow files parse as YAML. GitHub Actions have not run because this preparation branch is not connected to a GitHub repository. Rust formatting and Clippy were not run locally because their Rust components are unavailable; CI installs them. The RustSec `cargo audit` job is configured but has not run in GitHub.

## Issues and architectural observations

1. **Development profile UUID compatibility:** corrected for newly created profiles. UUIDs now match Java's `UUID.nameUUIDFromBytes` behavior for the `OfflinePlayer:<name>` UTF-8 identity. Existing database rows are deliberately not rewritten because changing a profile UUID can change the local player's identity in saved worlds.
2. **Java auto-selection:** launch fallback picks the first discovered runtime rather than selecting one that satisfies the installed version's requirement.
3. **Native archive resource limits:** path traversal and links are checked, but a malformed or hostile archive can still consume excessive disk space or processing time.
4. **Managed directory cleanup:** forgetting an instance removes its database row but leaves the managed directory on disk. This avoids deleting user files, but the now-unreferenced directory needs an explicit recovery/cleanup flow.
5. **Schema evolution:** the database creates tables with `CREATE TABLE IF NOT EXISTS` but has no schema-versioned migrations.
6. **UI organization:** most frontend behavior lives in one `App.tsx` file. It is acceptable for the current small prototype, but feature work should extract focused components and services as the UI grows.
7. **Version display:** the UI now reads the displayed version from `package.json`; keep the Rust package metadata synchronized for desktop builds.
8. **Runtime evidence:** automated validation did not use an installed game or user worlds. Successful in-game launch, log capture, and preservation of real-world data are maintainer-reported and are not established by these tests.

## Recommended implementation order

1. Finish and test the local vanilla launch slice: compatible Java selection, accurate offline test identity, clear preflight errors, and a real isolated launch test using user-selected legitimate game files.
2. Exercise a complete vanilla download and launch using legitimate game files, then add installer repair/recovery and compatible Java selection.
3. Implement Microsoft authentication and entitlement verification with OS credential storage before enabling authenticated launch in release builds.
4. Improve instance isolation, deletion/forget semantics, backups, and import/export.
5. Validate Fabric and Forge installations with legitimate game files, then expand modpack and loader formats incrementally.
6. Complete diagnostics, privacy/security review, Linux distribution validation, Windows/macOS support, and the remaining publication checklist before a stable release.

This feature set is an incremental foundation. Account authentication, modpacks, backup workflows, additional loader installers, advanced instance controls, and distribution work remain open roadmap items.
