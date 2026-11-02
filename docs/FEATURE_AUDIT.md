# OpnLauncher Feature Audit

Audit date: 2026-11-02
Scope: repository source and configuration only. No installed Minecraft directories or saved worlds were inspected.

## Summary

OpnLauncher is an existing Tauri 2 desktop prototype using Rust, React, TypeScript, Vite, and SQLite. Its strongest working slice is local discovery and registration of existing Minecraft installations, Java discovery, instance metadata, and a launch-plan resolver for complete local version files. The prototype does not download or install game files and does not implement authenticated Microsoft accounts.

The implementation is small and can be extended without replacing its architecture. The launch path has useful unit coverage for inheritance, rules, argument expansion, and path validation, but it has not been proven against a real installed Minecraft version in a desktop session. The AppImage packaging step produced an artifact during local validation, but both the AppImage and unbundled binary failed GTK initialization in the available automation session. Treat GUI and game launch as unverified.

## Existing implementation

| Area | Current implementation |
| --- | --- |
| Desktop shell | Tauri 2, Rust backend, React 19, TypeScript, Vite, and a custom CSS dark theme. Linux is the current development target. |
| Frontend | Home and instance views, installation inspection and registration, per-instance Java and memory controls, profile selection in development builds, log display, and a small read-only settings view. |
| Persistence | SQLite database in Tauri's application data directory; stores instances, development profiles, and a settings table. The connection is serialized through a mutex. |
| Java | Discovers runtimes from `JAVA_HOME`, `PATH`, common Linux paths, and standard executable locations; parses version, vendor, and architecture. Launch planning checks the selected runtime against the version's required Java major. |
| Installation detection | Reads common Minecraft directories, `MINECRAFT_DIR`, and user-selected local paths. Validates version JSON IDs and sorts discovered versions. It does not download or repair files. |
| Instance records | Create empty managed directories, register an existing installation/version, rename, forget the launcher record, set Java, and set memory. Existing installations are registered by their shared game-directory path, so they do not yet have independent saves or mods. |
| Launch resolver | Resolves inherited version metadata, libraries, JVM/game arguments, OS rules, classpath, native archives, Java compatibility, and logging configuration from local files. Starts Java directly without a shell and streams output to local logs/events. |
| Archive handling | Native archive extraction rejects unsafe paths and symbolic links. It currently has no expanded-size or entry-count limits. |
| Development profiles | Unverified local profiles are available only when both the development feature and Rust debug assertions are enabled. The release AppImage therefore does not expose this profile path. |
| Tauri security | The frontend has only `core:default`; it has no unrestricted shell, filesystem, or process plugin. |
| Privacy | No application network requests, telemetry, ads, crash-reporting service, or automatic log upload are implemented in this prototype. |

## Partial or unavailable features

- The navigation includes Versions, Mods, Modpacks, Accounts, and Downloads, but these destinations are placeholders.
- Minecraft version metadata is read from existing files only. There is no official manifest fetch, library or asset download, integrity verification, queue, resume, repair, or cancellation flow.
- Microsoft OAuth, ownership checks, account switching, token refresh, and OS credential storage are not implemented.
- The launch resolver supports the metadata forms represented in its code and tests; it has no end-to-end coverage across vanilla releases or mod-loader metadata.
- Java selection is manual per instance. The fallback uses the first discovered runtime, which can be incompatible even when another compatible runtime is installed.
- Instance isolation, JVM arguments, resolution, environment variables, import/export, backups, tags, playtime, and file browsing are not implemented.
- Fabric, Forge, NeoForge, Quilt, Modrinth, CurseForge, mod installation, modpacks, and resource/shader packs are not implemented.
- Settings currently display privacy information, detected Java and Minecraft paths, and an update placeholder. Theme, language, download, proxy, memory-default, accessibility, backup, and privacy controls are not implemented.
- There is no CI workflow, release/update verification, or Windows/macOS validation in the repository.

## Tests and validation

- Rust unit tests cover SQLite instance/profile operations, Java version parsing, local installation inspection, version ordering, launch metadata merging, argument rules, path validation, OS matching, and Java version defaults.
- Frontend tests cover local date/time and Java labels.
- There are no Tauri command integration tests, real-game launch tests, UI automation tests, installer/download tests, or cross-platform tests.
- This audit's checks passed: all 13 Rust unit tests, all 3 frontend tests, and the TypeScript/Vite production build.
- An earlier local AppImage build completed, but GUI startup and Minecraft launch remain unverified in the available session.

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
2. Implement official version metadata and a safe installation/download manager with hashes, atomic writes, retries, cancellation, and repair.
3. Implement Microsoft authentication and entitlement verification with OS credential storage before enabling authenticated launch in release builds.
4. Add independently isolated instances, settings, deletion/forget semantics, backups, and import/export.
5. Add Fabric first, then Modrinth discovery and install flows; expand loader and modpack formats incrementally.
6. Complete diagnostics, privacy/security review, Linux distribution validation, Windows/macOS support, CI, and publication preparation only when explicitly requested.
