# OpnLauncher Feature Audit

Audit date: 2026-11-03
Scope: repository source and configuration only. No installed Minecraft directories or saved worlds were inspected.

## Summary

OpnLauncher is a Tauri 2 desktop prototype using Rust, React, TypeScript, Vite, and SQLite. It has an official vanilla version browser and installer, Modrinth and CurseForge mod browsing, and Fabric and Forge loader installation for a selected instance. Loader and mod files are checked before installation. This source review did not perform live provider downloads, a Forge or Fabric installation, or a real Minecraft launch. Microsoft authentication is not implemented.

The implementation is small and can be extended without replacing its architecture. The launch path has useful unit coverage for inheritance, rules, argument expansion, and path validation, but it has not been proven against a real installed Minecraft version in a desktop session. The AppImage packaging step produced an artifact during local validation, but both the AppImage and unbundled binary failed GTK initialization in the available automation session. Treat GUI and game launch as unverified.

## Existing implementation

| Area | Current implementation |
| --- | --- |
| Desktop shell | Tauri 2, Rust backend, React 19, TypeScript, Vite, and a custom CSS dark theme. Linux is the current development target. |
| Frontend | Home and instance views, per-instance Minecraft and loader setup, installation inspection and registration, official version search/filter/install flow, Modrinth/CurseForge mod browsing matched to the selected instance, compatible build selection, mod file management, download progress/cancellation, per-instance Java and memory controls, local developer profiles, logs, and categorized settings for defaults, appearance, Java, loaders, storage, privacy, and about. |
| Persistence | SQLite database in Tauri's application data directory; stores instances, development profiles, and a settings table. The connection is serialized through a mutex. |
| Java | Discovers runtimes from `JAVA_HOME`, `PATH`, common Linux paths, and standard executable locations; parses version, vendor, and architecture. Launch planning checks the selected runtime against the version's required Java major. |
| Loader management | Per-instance setup lists compatible Fabric versions from Fabric Meta and Forge builds from Forge Maven, then installs the selected setup into that instance's folder. Fabric profile libraries with Maven coordinates are derived safely and verified with profile or repository SHA-1 values; Forge installers are checked against their published SHA-1 and run directly for the selected game directory. Existing Fabric, Forge, NeoForge, and Quilt profiles can be selected. |
| Installation detection | Reads common Minecraft directories, `MINECRAFT_DIR`, and user-selected local paths. Validates version JSON IDs and sorts discovered versions. Registered directories remain separate from managed downloaded instances. |
| Vanilla version installation | Fetches Mojang's version manifest on request, filters release/snapshot/old alpha/beta entries, resolves supported vanilla metadata, libraries, native classifiers, logging configs, and assets, then downloads into a new instance directory with HTTPS, size/SHA-1 checks, retries, bounded parallelism, cancellation, and progress. Very old metadata formats may not work; there is no pause/resume or repair yet. |
| Mod browser and management | Queries Modrinth and CurseForge for mods matched to the instance Minecraft version and loader; shows project images/gallery, descriptions, categories, compatible builds and required dependencies. Installs verified mod files with progress and cancellation; scans JAR metadata and supports enable, disable, and confirmed removal. CurseForge needs the user's own session-only API key. |
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
- There is no CI workflow, release/update verification, or Windows/macOS validation in the repository.

## Tests and validation

- Rust unit tests cover SQLite instance/profile/version switching, Java version parsing, local installation inspection, version ordering, launch metadata merging, argument rules, path validation, OS matching, Java version defaults, mod-loader detection, loader path/version helpers, CurseForge Minecraft-version filtering, provider query encoding, and safe provider/image/file URLs.
- Frontend tests cover local date/time and Java labels.
- There are no Tauri command integration tests, real-game launch tests, UI automation tests, live installer tests, or cross-platform tests. Installer helper tests validate metadata/path planning but do not contact Mojang.
- `cargo test --lib --offline`: 40 Rust tests passed, including the desktop-only check that developer profiles remain available in release builds. `npm run test`: 6 frontend tests passed. `npm run build`: TypeScript and Vite production build passed. `cargo check --features desktop --offline`: passed. `npm run tauri -- build --no-bundle`: compiled the release desktop executable at `src-tauri/target/release/opnlauncher`.
- The desktop executable was not launched. Live loader API responses, installer execution, GUI behavior, and Minecraft launch remain unverified. AppImage bundling was not attempted in this pass.
- Rust formatting could not be checked because the Rust formatting component is not installed; installing it would write outside the project folder.

## Issues and architectural observations

1. **Development profile UUID compatibility:** corrected for newly created profiles. UUIDs now match Java's `UUID.nameUUIDFromBytes` behavior for the `OfflinePlayer:<name>` UTF-8 identity. Existing database rows are deliberately not rewritten because changing a profile UUID can change the local player's identity in saved worlds.
2. **Java auto-selection:** launch fallback picks the first discovered runtime rather than selecting one that satisfies the installed version's requirement.
3. **Native archive resource limits:** path traversal and links are checked, but a malformed or hostile archive can still consume excessive disk space or processing time.
4. **Managed directory cleanup:** forgetting an instance removes its database row but leaves the managed directory on disk. This avoids deleting user files, but the now-unreferenced directory needs an explicit recovery/cleanup flow.
5. **Schema evolution:** the database creates tables with `CREATE TABLE IF NOT EXISTS` but has no schema-versioned migrations.
6. **UI organization:** most frontend behavior lives in one `App.tsx` file. It is acceptable for the current small prototype, but feature work should extract focused components and services as the UI grows.
7. **Version display:** the UI now reads the displayed version from `package.json`; keep the Rust package metadata synchronized for desktop builds.
8. **Runtime evidence:** no actual installed game files were used for this audit, so successful in-game launch, log capture, and preservation of existing worlds are not established.

## Recommended implementation order

1. Finish and test the local vanilla launch slice: compatible Java selection, accurate offline test identity, clear preflight errors, and a real isolated launch test using user-selected legitimate game files.
2. Exercise a complete vanilla download and launch using legitimate game files, then add installer repair/recovery and compatible Java selection.
3. Implement Microsoft authentication and entitlement verification with OS credential storage before enabling authenticated launch in release builds.
4. Improve instance isolation, deletion/forget semantics, backups, and import/export.
5. Validate Fabric and Forge installations with legitimate game files, then expand modpack and loader formats incrementally.
6. Complete diagnostics, privacy/security review, Linux distribution validation, Windows/macOS support, CI, and publication preparation only when explicitly requested.

This feature set is an incremental foundation; several planned capabilities remain open. Account authentication, modpacks, backup workflows, additional loader installers, advanced instance controls, and distribution work remain open roadmap items.
