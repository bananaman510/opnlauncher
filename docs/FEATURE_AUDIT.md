# OpnLauncher Feature Audit

Audit date: 2026-11-03
Scope: repository source and configuration only. No installed Minecraft directories or saved worlds were inspected.

## Summary

OpnLauncher is a Tauri 2 desktop prototype using Rust, React, TypeScript, Vite, and SQLite. It has an official vanilla version browser and installer plus a first Modrinth and CurseForge mod browser for existing loader-equipped instances. Mod files are checked and staged before installation. Neither a live provider download nor a real Minecraft launch was performed in this audit, and authenticated Microsoft accounts are not implemented.

The implementation is small and can be extended without replacing its architecture. The launch path has useful unit coverage for inheritance, rules, argument expansion, and path validation, but it has not been proven against a real installed Minecraft version in a desktop session. The AppImage packaging step produced an artifact during local validation, but both the AppImage and unbundled binary failed GTK initialization in the available automation session. Treat GUI and game launch as unverified.

## Existing implementation

| Area | Current implementation |
| --- | --- |
| Desktop shell | Tauri 2, Rust backend, React 19, TypeScript, Vite, and a custom CSS dark theme. Linux is the current development target. |
| Frontend | Home and instance views, installation inspection and registration, official version search/filter/install flow, Modrinth/CurseForge mod browsing, compatible build selection, mod file management, download progress/cancellation, per-instance Java and memory controls, profile selection in development builds, log display, and a small read-only settings view. |
| Persistence | SQLite database in Tauri's application data directory; stores instances, development profiles, and a settings table. The connection is serialized through a mutex. |
| Java | Discovers runtimes from `JAVA_HOME`, `PATH`, common Linux paths, and standard executable locations; parses version, vendor, and architecture. Launch planning checks the selected runtime against the version's required Java major. |
| Installation detection | Reads common Minecraft directories, `MINECRAFT_DIR`, and user-selected local paths. Validates version JSON IDs and sorts discovered versions. Registered directories remain separate from managed downloaded instances. |
| Vanilla version installation | Fetches Mojang's version manifest on request, filters release/snapshot/old alpha/beta entries, resolves supported vanilla metadata, libraries, native classifiers, logging configs, and assets, then downloads into a new instance directory with HTTPS, size/SHA-1 checks, retries, bounded parallelism, cancellation, and progress. Very old metadata formats may not work; there is no pause/resume or repair yet. |
| Mod browser and management | Queries Modrinth and CurseForge for mods matched to the instance Minecraft version and loader; shows project images/gallery, descriptions, categories, compatible builds and required dependencies. Installs verified mod files with progress and cancellation; scans JAR metadata and supports enable, disable, and confirmed removal. CurseForge needs the user's own session-only API key. |
| Instance records | Create empty managed directories, register an existing installation/version, rename, forget the launcher record, set Java, and set memory. Existing installations are registered by their shared game-directory path, so they do not yet have independent saves or mods. |
| Launch resolver | Resolves inherited version metadata, libraries, JVM/game arguments, OS rules, classpath, native archives, Java compatibility, and logging configuration from local files. Starts Java directly without a shell and streams output to local logs/events. |
| Archive handling | Native archive extraction rejects unsafe paths and symbolic links. It currently has no expanded-size or entry-count limits. |
| Developer profiles | Unverified local profiles are enabled by the default desktop feature set in development and release builds. They are clearly labeled and do not authenticate accounts or verify ownership. |
| Tauri security | The frontend has only `core:default`; it has no unrestricted shell, filesystem, or process plugin. |
| Privacy | No telemetry, ads, crash-reporting service, or automatic log upload. Mojang, Modrinth, and CurseForge API calls occur only during related feature use. Project images load from approved provider CDN hosts. The CurseForge key exists only in app memory and is sent only to CurseForge. |

## Partial or unavailable features

- Modpacks and Accounts are placeholders. The mod browser manages content for already-installed Fabric, Forge, NeoForge, or Quilt instances; OpnLauncher does not yet install those loaders. Versions browses official vanilla versions; Downloads shows the current or most recent install progress. There is no persistent queue, pause/resume, or repair flow.
- The installer handles official vanilla version metadata and files. Fabric/Quilt/Forge/NeoForge metadata, modpacks, and custom version installation are not supported.
- Microsoft OAuth, ownership checks, account switching, token refresh, and OS credential storage are not implemented.
- The launch resolver supports the metadata forms represented in its code and tests; it has no end-to-end coverage across vanilla releases or mod-loader metadata.
- Java selection is manual per instance. The fallback uses the first discovered runtime, which can be incompatible even when another compatible runtime is installed.
- Instance isolation, JVM arguments, resolution, environment variables, import/export, backups, tags, playtime, and file browsing are not implemented.
- Loader metadata detection and mod-provider integration are implemented, but the loader installers, modpack import/update, resource/shader pack management, and automatic mod updates are not implemented.
- Settings currently display privacy information, detected Java and Minecraft paths, and an update placeholder. Theme, language, download, proxy, memory-default, accessibility, backup, and privacy controls are not implemented.
- There is no CI workflow, release/update verification, or Windows/macOS validation in the repository.

## Tests and validation

- Rust unit tests cover SQLite instance/profile operations, Java version parsing, local installation inspection, version ordering, launch metadata merging, argument rules, path validation, OS matching, Java version defaults, mod-loader detection, CurseForge Minecraft-version filtering, provider query encoding, and safe provider/image/file URLs.
- Frontend tests cover local date/time and Java labels.
- There are no Tauri command integration tests, real-game launch tests, UI automation tests, live installer tests, or cross-platform tests. Installer helper tests validate metadata/path planning but do not contact Mojang.
- `cargo test --offline`: 28 Rust tests passed, including the desktop-only check that developer profiles remain available in release builds. `npm run test`: 3 frontend tests passed. `npm run build`: TypeScript and Vite production build passed. `cargo check --features desktop --offline`: passed.
- The release executable compiled at `src-tauri/target/release/opnlauncher`. AppImage bundling stopped because `linuxdeploy` is not available in the environment. The desktop GUI, live provider API/download, actual mod installation, and Minecraft launch remain unverified.
- `cargo fmt --all -- --check` could not run because the Rust formatting component is not installed; installing it would write to the toolchain outside this project folder.

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
4. Add independently isolated instances, settings, deletion/forget semantics, backups, and import/export.
5. Add Fabric first, then Modrinth discovery and install flows; expand loader and modpack formats incrementally.
6. Complete diagnostics, privacy/security review, Linux distribution validation, Windows/macOS support, CI, and publication preparation only when explicitly requested.

This feature set is an incremental foundation; several planned capabilities remain open. Account authentication, mod loaders, Modrinth integration, modpacks, backups, custom themes, and distribution work remain open roadmap items.
