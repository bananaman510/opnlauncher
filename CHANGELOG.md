# Changelog

All notable project changes are recorded here. This project follows Semantic Versioning; versions are not tagged or released automatically.

## [0.1.0-dev.46] - Unreleased

### Added

- Added opt-in local playtime tracking for Minecraft and Horizon game processes, with session recovery after an unclean launcher exit.
- Added an Overview playtime panel with 7-day, 30-day, 90-day, and 12-month charts, session totals, lifetime playtime, and a local tracking control.

### Privacy

- Playtime tracking is disabled by default, stored only in the local SQLite database, and can be paused without deleting existing history.

## [0.1.0-dev.45] - Unreleased

### Added

- Added an **On this device** version library view for locally discovered Minecraft installations, with a clear scan action and named import dialog.
- Added provider-aware compatible mod update checks, individual updates, and sequential Update all for installed mods whose provider project and current release can be verified.
- Added an instance folder action that opens its registered game directory in the system file manager.

### Improved

- Reworked the launcher palette around deep black and stone surfaces with brighter readable text, restrained accent color, and stronger hierarchy.
- Made desktop scrolling use one consistent content area, preserved compact horizontal navigation on small screens, and improved spacing for settings, mod rows, version controls, and responsive layouts.
- Replaced the browser prompt used when adding a discovered installation with an accessible launcher dialog.

### Tests

- Added unit coverage for detecting compatible mod updates without suggesting an update when the current build is unknown or already current.

## [0.1.0-dev.44] - Unreleased

### Added

- Added Minecraft Java/Horizon engine filters and recent/name/version sorting to the instance library; the selected sort and engine filter are kept in local launcher preferences.
- Added installed-mod search, enabled/disabled filters, and bulk enable/disable actions alongside existing safe removal and version switching.
- Added a top-bar account switcher for fast profile changes plus an in-app Microsoft Entra registration link and desktop app setup guide.

### Tests

- Added coverage for engine filtering, instance sorting, installed-mod search, and enabled-state filters.

### Improved

- Reworked the launcher-wide visual system with a coordinated stone-and-biome palette, clearer navigation and page hierarchy, more consistent controls and surfaces, improved responsive layouts, and stronger focus states.

## [0.1.0-dev.43] - Unreleased

### Fixed

- Before Java Minecraft and the optional Horizon Java + Fabric runtime launch, check for missing official Minecraft metadata, client JARs, selected-loader library artifacts, logging configurations, asset indexes, asset objects, and legacy virtual assets. Missing files are restored from Mojang or the loader metadata through the existing bounded, cancellable, checksum-verified download pipeline.
- Resolve an inherited loader profile's base client JAR from its Minecraft version, so Fabric/Forge launches use the correct client file.
- Preserve existing regular instance files during automatic repair; the repair adds missing files without changing mods, settings, saves, or worlds.

### Tests

- Added regression fixtures for missing asset indexes, asset objects, loader libraries, inherited client JARs, and preserving already-present files.

## [0.1.0-dev.42] - Unreleased

### Added

- Added experimental Microsoft account sign-in through the system browser using authorization-code PKCE, Xbox authentication, Java entitlement verification, Minecraft profile lookup, refresh-on-launch, and Linux Secret Service token storage. The flow has not yet been verified with a live account; Windows and macOS credential storage remain unsupported.
- Expanded the experimental Fabric bridge with read-only Minecraft `Level` snapshots for day time, game time, build-height bounds, and rain/thunder state and levels.
- Added bounded read-only block-state snapshots for loaded positions in a 3×3×3 area around the local player, including block identifiers and basic air/collision/render flags.
- Added protocol validation and probe coverage to verify environment and bounded block-state values move from Horizon's native world tick into unchanged Fabric callback code.
- Fixed both the bundled runtime and official-Loader test helper archives to include and verify the `Level`, `Block`, and `BlockState` facade classes.

### Fixed

- Give each Java launch a private native-library extraction directory and remove it after the game exits, preventing concurrent launches from sharing files that one process may still be using.
- Bound native archive extraction by entry count, per-file size, and total expanded size; reject unsafe paths, symbolic links, and duplicate files while allowing valid repeated directory entries.
- Redact the active Minecraft access token from game output before writing launch logs or forwarding output to the interface.

### Known limitations

- The experimental Horizon Fabric code bridge remains opt-in and maps only a small API subset. It is not general Fabric compatibility and has not been verified in a live Horizon session with a third-party mod.
- Horizon `.hnmod` native-code installation, compilation, and launch management are not implemented because the pinned Horizon source in this repository does not include the separate native compiler/build tooling. Existing `.hmod` content mods remain supported.
- Microsoft sign-in needs a live-account test and secure credential-store implementations for Windows and macOS.

## [0.1.0-dev.41] - Unreleased

### Fixed

- The Fabric bridge allowlist now rejects unmapped Mojang Blaze3D rendering classes and descriptors, including APIs used by graphics integrations such as Sodium.
- Horizon's launch status now describes the player/input snapshots and bounded actions that the bridge maps instead of incorrectly reporting them as unmapped.

### Tests

- Added a regression case proving a mod that references `RenderSystem` is reported as unsupported before staging.

## [0.1.0-dev.40] - Unreleased

### Added

- Added regression coverage for the common Fabric end-tick auto-sprint pattern: reading the sprint binding's default key, calling `KeyMapping.set`, and updating it with `setDown` now runs through the unchanged probe and produces a Horizon sprint action.
- Extended the class-reference fixture to cover the exact client tick, player/options, and sprint-key API references used by this pattern.

### Clarified

- A locally cached third-party mod with these references was inspected statically only. The third-party JAR was not executed; the automated behavior check uses the project-owned probe.

## [0.1.0-dev.39] - Unreleased

### Added

- Added immutable `Vec3.add(double, double, double)` support so a mapped mod can calculate a velocity adjustment from Horizon's read-only player snapshot before requesting the bounded Rust velocity update.
- Extended the strict API allowlist and unchanged Fabric probe to exercise vector addition through the official Fabric Loader integration path.

### Clarified

- Other vector methods remain unsupported; this addition does not provide general Minecraft or Fabric API compatibility.

## [0.1.0-dev.38] - Unreleased

### Added

- Added a narrow mapping for vanilla `KeyMapping.set(InputConstants.Key, boolean)` and `KeyMapping.setDown(boolean)` calls that target Horizon's sprint binding. Requests return through the existing bounded bridge and are applied by Horizon's Rust game thread.
- Extended the API allowlist and unchanged Fabric probe to exercise programmatic sprint-key updates through the bridge.

### Clarified

- Other key mappings remain sidecar-local, and arbitrary Fabric/Minecraft APIs, mixins, rendering, and Sodium are still unsupported.

## [0.1.0-dev.37] - Unreleased

### Added

- Added the read-only Minecraft `Entity.blockPosition()` mapping backed by Horizon's player snapshot, including `BlockPos.getX/Y/Z()` and correct flooring for negative coordinates.
- Extended the unchanged Fabric probe, strict Minecraft API scanner, packaged helper-JAR verification, and real-Loader integration path to exercise block-coordinate reads.

### Clarified

- This expands the explicit API subset for simple HUD/position mods. It does not provide world block access, general gameplay compatibility, Mixins, or Sodium support.

## [0.1.0-dev.36] - Unreleased

### Added

- Added a repeatable integration check that compiles the unchanged bridge probe against the published Minecraft 26.3 Fabric API modules, starts official Fabric Loader 0.20.1, and exercises its callbacks through the Rust bridge.
- Added build verification that the packaged helper JAR contains both the `Vec3` and `Identifier` facades required by the mapped movement and key-category APIs.

### Clarified

- Real Loader startup and API linkage are now exercised with synthetic Horizon snapshots. This remains a narrow explicit API mapping; live Horizon behavior, third-party mod compatibility, Sodium, Mixins, and arbitrary Fabric APIs remain unverified or unsupported.

## [0.1.0-dev.35] - Unreleased

### Added

- Added the Minecraft 26.3 `Identifier.fromNamespaceAndPath` and `KeyMapping.Category.register` subset, so unchanged Fabric code can register namespaced key categories and use them with the existing mapped key press/tick bridge.
- Extended the unchanged Java probe and Rust class-reference allowlist to exercise category IDs, namespace/path access, and duplicate-ID rejection.

### Clarified

- Registered categories currently support bridge-side keybind logic only; Horizon does not show them in a controls screen or support rebinding. This remains an explicitly mapped experimental API subset, not general Fabric compatibility.

## [0.1.0-dev.34] - Unreleased

### Added

- Added a read-only `Minecraft.options` facade for vanilla forward, back, left, right, jump, sneak, and sprint key mappings. `KeyMapping.isDown()` reports Horizon's mapped movement state and `consumeClick()` reports press edges.
- Extended the bounded Rust-to-Java tick protocol, static API scanner, unchanged probe JAR, and loopback tests to verify movement state and press-edge delivery.

### Clarified

- Movement input is reported only while Horizon is actively playing. Java-side Options editing, arbitrary key contexts, mapping conflicts, mouse/controller bindings, and general Fabric mod compatibility remain unsupported.

## [0.1.0-dev.33] - Unreleased

### Added

- Mapped Fabric's `Entity.setYRot(float)` and `setXRot(float)` calls to bounded local-player look updates applied by Horizon's Rust game thread during active play.
- Extended the unchanged probe, Java facade, class-reference allowlist, Rust IPC parser, and loopback harness to verify yaw/pitch writes and reject invalid angles.
- Included the `Vec3` facade in the packaged bridge helper JAR so the development probe and unchanged mods using mapped velocity APIs can resolve it.

### Clarified

- Look updates are limited to finite yaw values within ±360,000 degrees and pitch values from −90 through 90 degrees. They affect only the local Horizon player's view/body; arbitrary entity and authoritative server changes remain unsupported.
- The bridge still maps only explicitly listed APIs. General Fabric gameplay mod compatibility, Mixins, Sodium, and a live in-game third-party mod test remain outstanding.

## [0.1.0-dev.32] - Unreleased

### Added

- Mapped `Entity.sendSystemMessage(Component)` to Horizon's native system chat, honoring the local chat visibility setting.
- Updated the unchanged probe and Rust/Java bridge checks to exercise a Fabric-style keybinding callback that sends system chat alongside an action-bar message and bounded player actions.

### Clarified

- System messages are delivered as plain text; Minecraft text styles, click events, and other component features are not translated.

## [0.1.0-dev.31] - Unreleased

### Added

- Mapped `Entity.setDeltaMovement(Vec3)` to a bounded Rust game-thread velocity request for the active Horizon player.
- Added an explicit four-blocks-per-tick speed limit, finite-value validation on both sides of bridge IPC, and probe/loopback coverage for the unchanged Java mod calling the mapped setter.

### Clarified

- The optional bridge can apply this one explicit movement write while native Horizon is playing. Other player/world changes, Mixins, Sodium, and general Fabric compatibility remain unsupported.

## [0.1.0-dev.30] - Unreleased

### Added

- Mapped Fabric's `Entity.getDeltaMovement()` to Horizon's real local-player velocity snapshot, with a narrow immutable `Vec3` facade for coordinates and length calculations.
- Extended bridge protocol validation, class scanning, and the unchanged probe/loopback tests to cover the vector snapshot and horizontal-speed calculation.

### Clarified

- Velocity is read-only snapshot data. Unsupported Minecraft methods and vector operations remain rejected; this does not add general Fabric or Sodium compatibility.

## [0.1.0-dev.29] - Unreleased

### Added

- Mapped Fabric's `Entity.getYRot()` and `getXRot()` to bounded local-player rotation snapshots captured by Horizon's Rust game loop. Unchanged allowlisted mods can now read the player's yaw and pitch in Fabric client tick callbacks.
- Extended the Fabric bridge protocol, static scanner, probe, and loopback harness to carry and verify finite orientation values; rotation setters remain rejected.

### Clarified

- The bridge still executes Java code in its helper JVM and maps only explicit APIs. Rotation reads do not add general Fabric or renderer compatibility.

## [0.1.0-dev.28] - Unreleased

### Added

- Mapped the local player's `Entity.setSprinting(boolean)` call to Horizon's native sprint-key state. Requests return through bounded bridge IPC and are applied on the Rust game thread only during active play.
- Updated the unchanged probe so pressing its default **P** key toggles Horizon sprint and displays the resulting state in the Rust HUD.
- Added scanner, bridge protocol, Fabric Loader harness, and native sprint-state regression coverage for this explicit write mapping.

### Clarified

- The bridge supports one specific player-state write, sprint control. Other player and world mutations, Mixins, Sodium, and general Fabric compatibility remain unsupported.

## [0.1.0-dev.27] - Unreleased

### Added

- Mapped Fabric 26.3 keyboard key registration to Horizon's native input snapshots, translating physical key names into Minecraft's SDL/HID key values. Supported mods can register default keyboard bindings and poll `KeyMapping.isDown()` or consume press edges through `KeyMapping.consumeClick()` from client tick callbacks.
- Extended the Fabric class/API allowlist and bridge probe to cover `KeyMappingHelper.registerKeyMapping`, `getBoundKeyOf`, `InputConstants`, key categories, and the mapped key-query methods. Keyboard input is bounded and forwarded only for the active Horizon game; unsupported key categories and APIs remain rejected.

### Clarified

- Documented that keyboard mappings use their declared default key only. Rebinding, mouse/controller input, key conflict handling, Mixins, Sodium, and arbitrary Java-to-Rust translation remain unsupported.

## [0.1.0-dev.26] - Unreleased

### Added

- Mapped `Minecraft.level.getOverworldClockTime()` to a read-only time snapshot from Horizon's native world state.
- Extended the unchanged Fabric probe, class-reference allowlist, and Rust/Java loopback test to verify world-clock snapshots and rejection of unmapped clock methods.

### Clarified

- Updated Horizon documentation to reflect the official Fabric Loader check and distinguish it from an in-game or third-party-mod test.
- The bridge remains an explicit, limited API mapping; unsupported APIs and Mixins are rejected before the helper runs.

## [0.1.0-dev.25] - Unreleased

### Added

- Mapped Fabric `START_CLIENT_TICK` and `END_CLIENT_TICK` to snapshots captured at Horizon's corresponding client-tick boundaries. The helper processes callbacks asynchronously and returns HUD actions to the Rust game thread; a full bounded queue may drop callbacks.
- Added regression coverage for tick ordering, snapshot delivery in both phases, and rejection of unsupported level tick events.

### Verified

- Rebuilt the bridge and checked both tick phases with official Fabric Loader 0.20.1 on Java 25 and an unchanged probe compiled against the official Fabric API 26.3 classes.

### Clarified

- The bridge still maps only selected events and simple getters/action-bar calls. It does not support Mixins, Sodium, rendering, world mutation, or arbitrary Fabric mods; live Horizon gameplay remains unverified.

## [0.1.0-dev.24] - Unreleased

### Fixed

- Initialize and retain Fabric Loader's game transformer in the Horizon compatibility provider so the real Loader can transform its own entrypoint classes before launching the bridge.

### Verified

- Started official Fabric Loader 0.20.1 on Java 25.0.4.1 with the production helper and an unchanged probe JAR compiled against the published Fabric API 26.3 lifecycle classes. The probe received a synthetic Rust player snapshot and returned an action-bar message through the production loopback bridge.

### Clarified

- This verifies Loader startup, the Fabric event ABI, and Rust/Java bridge IPC; it does not verify a live Horizon game session or a third-party mod. General Fabric compatibility, Mixins, Sodium, rendering, and world/player mutation remain unsupported.

## [0.1.0-dev.23] - Unreleased

### Improved

- Strengthened the Fabric bridge check to load the probe from a separate, unchanged JAR and run the production Rust IPC module against the Java helper over loopback.
- Verified the end-to-end test path from a Fabric `END_CLIENT_TICK` callback through the Rust player snapshot to Horizon's action-bar response.

### Clarified

- The automated loopback test uses a Fabric Loader stub and does not prove compatibility in a real Horizon game session.

## [0.1.0-dev.22] - Unreleased

### Added

- Added read-only local-player snapshots to the optional Fabric-to-Horizon code bridge: player name, position, health, sprint state, and ground state are available during `END_CLIENT_TICK` callbacks.
- Added exact class/member allowlist entries for the local-player facade while continuing to reject writes and unmapped Minecraft APIs.
- Expanded the development probe to show the mapped player snapshot in Horizon's action bar.

## [0.1.0-dev.21] - Unreleased

### Added

- Added an explicit Fabric-to-Horizon mapping for Fabric end-client ticks and `Minecraft.gui.hud.setOverlayMessage(Component.literal(text), animate)`. Simple action-bar messages now enter Horizon's native Rust HUD on the game thread.
- Added a bounded Java-to-Rust action response and tightened scanner coverage for the supported facade members while continuing to reject unmapped game APIs.
- Added a development-only Fabric probe mod and build command for checking the mapped action bar in a Horizon profile.

### Clarified

- The callback receives a limited Minecraft facade; it is not the Java Minecraft client. World, player, input, renderer, Mixins, Sodium, and general Fabric gameplay compatibility remain unsupported and need real-loader/in-game validation.

## [0.1.0-dev.20] - Unreleased

### Improved

- Added a minimal Fabric API 26.3 lifecycle facade and an automated Java contract check for entrypoint invocation, `END_CLIENT_TICK` delivery, and rejection of unmapped lifecycle events.
- Restricted code-bridge staging to unchanged JAR copies that pass bounded class/API inspection, entrypoint metadata checks, duplicate-id checks, and the explicit tick-event allowlist. Original mod files remain untouched.
- Moved native Horizon tick forwarding until after the Rust world tick completes and hardened staging against checksum changes, symlinks, and unmanaged files.

### Clarified

- The bridge runs Java code beside Horizon; it does not translate arbitrary Java bytecode or Minecraft calls into Rust. The callback receives `null` for the Minecraft object, so ordinary gameplay mods, Mixins, Sodium, and mods that access game state are not supported. This remains an experimental API-mapping proof, not general Fabric mod support.

## [0.1.0-dev.19] - Unreleased

### Added

- Added a per-profile, opt-in Horizon Fabric code bridge. Fabric Loader runs in a helper JVM beside Rust Horizon, and Horizon's native client ticks are forwarded to Fabric's `END_CLIENT_TICK` callback through a bounded local queue.
- Added explicit launch checks for the profile's installed Minecraft 26.3 Fabric Loader and Java 25+ runtime. The setting defaults to off, leaves the installed JARs unchanged, and remains separate from the optional Java Minecraft backend and static-resource adapter.

### Limited

- This is a first API mapping, not a Java-to-Rust compiler. Fabric entrypoints and the end-tick callback can run, but receive an empty Minecraft client shell. Minecraft state, renderer APIs, Mixins, and Sodium are unsupported; real-mod and in-game validation remains outstanding.
- Documented that enabled Java mods execute with the user's account permissions and are not sandboxed.

## [0.1.0-dev.18] - Unreleased

### Added

- Added an opt-in Java Minecraft + Fabric runtime for Horizon profiles. It installs Minecraft 26.3 and Fabric Loader into a profile-private folder and leaves the native Rust Horizon runtime and mod directory separate.
- Existing Horizon-profile mod JARs are copied into the isolated Java runtime without moving, replacing, or deleting the originals.
- Routed enabled compatibility profiles through the existing offline Java launch pipeline and the standard Modrinth/CurseForge mod manager, so compatible Fabric JARs run unchanged on the Java game client.
- Added per-profile runtime selection, Java 25+ validation, Fabric profile validation, and SQLite migration coverage for the isolated runtime settings.

### Clarified

- Documented that this compatibility mode runs the Java Minecraft client; it does not translate Fabric mods or execute their bytecode inside Horizon's Rust engine.
- Kept the static resource adapter as a separate, opt-in path for code-free content in native Horizon.

## [0.1.0-dev.17] - Unreleased

### Added

- Added an opt-in Horizon Fabric resource bridge with Modrinth and CurseForge browsing filtered to Minecraft 26.3 Fabric builds.
- Added startup-time inspection and adaptation for standalone Fabric JARs that contain only safe, code-free `assets/` and `data/` resources. Adapted files are validated as Horizon content packages before they are enabled.
- Added per-profile bridge status, candidate/skipped explanations, enable/disable controls, launch-log summaries, size/path limits, and namespace conflict checks.

### Limited

- The bridge does not run Fabric Loader, Java bytecode, entrypoints, mixins, access wideners, nested JARs, or additional Fabric dependencies. Most Fabric mods remain unsupported by Horizon.
- The bridge is disabled by default. Disabling it preserves downloaded JARs and generated Horizon packages while turning off packages managed by the adapter.

## [0.1.0-dev.16] - Unreleased

### Fixed

- Complete Horizon preparation now imports the verified Minecraft sound assets before checking resource completeness, matching Horizon preparer's `--complete` behavior.
- Linux Horizon runtime builds now require ALSA development files so AppImages cannot silently omit game audio; missing `libudev` only disables optional controller support.
- Updated the development test runner to the supported, patched Vitest 5 line after the dependency audit identified critical advisories in the previous Vitest release.

## [0.1.0-dev.15] - Unreleased

### Fixed

- Horizon preparation now stages Mojang's SHA-1-pinned Minecraft 26.3 manifest in its private preparer cache, avoiding failures when a game installation contains a republished or launcher-rewritten version JSON.
- Reject modified cached Horizon manifests instead of silently replacing them.

### Improved

- Reworked the home and Horizon setup surfaces with original voxel landscape artwork, clearer color accents, and a balanced responsive card layout.
- Added Minecraft landscape artwork to the Horizon profile card in the version browser.

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
