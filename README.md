# OpnLauncher — 0.1.0-dev.46

OpnLauncher is an independent, local-first desktop launcher for Minecraft Java Edition. It is built with Tauri 2, Rust, React, TypeScript, and SQLite, with Linux as the current development platform.

> **Development pre-release — not a stable or production release.** Version `0.1.0-dev.46` is shared early to invite feedback and contributions. Features and local data formats may change, and bugs are expected. Back up worlds and important files before testing. This version does not promise a stable API.

The project is an early development prototype. Vanilla installation and launching, Fabric and Forge setup, provider-backed mod management, and a separate experimental Horizon profile path are implemented incrementally. The maintainer has reported successful local AppImage and Minecraft/Horizon launches; this source review does not independently reproduce those desktop sessions or validate the app on every supported distribution. Read [Current status and limitations](#current-status-and-limitations) before relying on it for valuable worlds.

OpnLauncher is not affiliated with, endorsed by, or associated with Mojang Studios or Microsoft. Minecraft is a trademark of Mojang Synergies AB. This project does not bundle or redistribute Minecraft game files. When you request an installation, it fetches official files using Mojang's version metadata.

## What works today

| Area | Available now |
| --- | --- |
| Minecraft versions | Browse Mojang's releases, snapshots, and older alpha/beta entries; install supported vanilla versions into new managed instance folders. The **On this device** view shows locally discovered game folders and versions, with a named add flow for importing one into the library. OpnLauncher checks for missing official files and selected-loader libraries before Java launch and repairs them with verified downloads. Very old metadata formats may not work. |
| Instances | Create an empty profile, install a version, inspect/register a local installation, rename it, favorite it, choose Java and memory, search by name/version/folder, filter by game engine, sort by recent play/name/version, open its game folder in the system file manager, and remove its library entry while preserving its files. |
| Mod loaders | Install Fabric or Forge for a selected instance. Detect existing Fabric, Forge, NeoForge, and Quilt profiles; NeoForge and Quilt installation is not implemented yet. |
| Mods | Browse Modrinth and CurseForge for a selected instance's Minecraft version and loader; install compatible builds and required dependencies; view project art; search and sort installed files; check for provider-linked compatible updates and update one or all; filter enabled/disabled mods; enable or disable a selection; remove individual files or bulk-delete; and switch installed mod versions. Update checks contact the selected provider only when requested. CurseForge requires your own API key for the current session. |
| Modpacks | The page is currently a roadmap placeholder. Modrinth/CurseForge modpack import, update, and export are not implemented yet. |
| Java | Discover Java runtimes, inspect a custom executable, set a default or per-instance runtime, and choose memory for each instance. |
| Accounts | Keep local, unverified test profiles in development and release builds. An experimental Microsoft sign-in flow verifies Java ownership for Java launches; switch profiles from the top bar, and the Accounts page links to Microsoft's app registrations with desktop setup steps. Secure token storage currently works on Linux only and needs a real-account test. |
| Horizon | Create separate Minecraft 26.3 profiles for the native Rust-engine Horizon client, prepare official support files, and manage local `.hmod` content. An optional experimental Fabric code bridge maps client ticks, player snapshots, world time/build-height/weather, basic read-only nearby block states, mapped vanilla movement, custom keyboard bindings, bounded player actions, plain system chat, and simple HUD text; a separate Java + Fabric mode runs the Java game client. |
| Playtime | An opt-in local session counter on Overview tracks launched game processes, with 7-day, 30-day, 90-day, and 12-month charts plus lifetime totals. Tracking starts off; existing history stays local and is kept if tracking is paused. |
| Preferences and diagnostics | Store appearance and launcher preferences locally; view local launch output and process exit status. There is no automatic diagnostic upload. |

The interface includes an overview, instance library, Minecraft version browser, mod browser, Horizon engine settings, downloads view, local profiles, and settings for appearance, Java, loaders, storage, privacy, and about.

## Get OpnLauncher running on Linux

### Requirements

- Linux desktop with GTK 3 and WebKitGTK 4.1 development libraries.
- Rust stable **1.90 or later**, including Cargo.
- Node.js **22.12 or later** and npm. This is required by the current test tooling.
- Java for launching Minecraft. Horizon resource preparation additionally needs a JDK 25 installation, including `javac`.
- An internet connection for downloading the version list, game files, loaders, or mods. Local instance management and settings work without downloading new content.

Install Rust with [rustup](https://rustup.rs/) if it is not already installed. Add the formatting and lint tools used by CI with `rustup component add rustfmt clippy`. Install Node.js and npm using your distribution or the [official Node.js downloads](https://nodejs.org/en/download/).

### Fedora

Install the native Tauri build dependencies. `dbus-devel` and `pkgconf-pkg-config` provide the D-Bus development metadata and `pkg-config`. ALSA development files are required for Horizon audio on Linux; the AppImage build now stops with an install hint if they are missing, instead of producing a silent Horizon runtime. `systemd-devel` provides `libudev` for optional controller support; if that is missing, Horizon audio still builds.

```sh
sudo dnf install webkit2gtk4.1-devel dbus-devel pkgconf-pkg-config openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel alsa-lib-devel systemd-devel
sudo dnf group install "c-development"
```

Horizon resource preparation also needs the JDK 25 development tools, including `javac`. Install Fedora's OpenJDK 25 development package with:

```sh
sudo dnf install java-25-openjdk-devel
```

The package provides the OpenJDK 25 development tools ([Fedora package details](https://packages.fedoraproject.org/pkgs/java-25-openjdk/java-25-openjdk-devel/)). In OpnLauncher, select Java 25 from that JDK under **Settings → Java Manager**; the selected Java installation must contain its matching `bin/javac`.

### Debian or Ubuntu

```sh
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev libdbus-1-dev pkg-config build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev libudev-dev
```

For other distributions, use the matching packages from the [Tauri Linux prerequisites guide](https://v2.tauri.app/start/prerequisites/). Package names differ by distribution.

### Run from the source folder

From the root of the OpnLauncher checkout:

```sh
npm ci
npm run tauri -- dev
```

The first build compiles Rust dependencies and Horizon's native game engine and can take a while. The application window opens when the frontend, desktop backend, and managed Horizon runtime are ready. `npm run dev` starts only the frontend preview; it does not provide the Rust commands needed for installation, local file access, or launching Minecraft.

### Build a Linux AppImage

Build a local AppImage with:

```sh
npm run tauri -- build --bundles appimage
```

Look in `src-tauri/target/release/bundle/appimage/` for the generated `.AppImage`. If you build with a custom `CARGO_TARGET_DIR`, the bundle is created under that target directory instead. Make the file executable and start it:

```sh
chmod +x ./OpnLauncher_VERSION_amd64.AppImage
./OpnLauncher_VERSION_amd64.AppImage
```

Replace the example filename with the generated file's name. This produces a **local development build**. The manual GitHub Actions workflow can also build an AppImage and save it as a temporary workflow artifact; it does not create a tag, GitHub release, or public download automatically. AppImage portability depends on the system it was built on and the target system's libraries and desktop environment. Build and run it on the Linux distributions you intend to support. See Tauri's [AppImage guide](https://v2.tauri.app/distribute/appimage/) for its packaging details and limitations.

## First launch and a basic play setup

1. Open **Accounts** and either create a local developer profile, or configure Microsoft sign-in if you have an Entra public-client application ID. Local profiles remain marked **unverified**.
2. Open **Instances** and choose **Add an instance**.
   - Choose the official Minecraft version browser to create a new managed instance, or
   - inspect and register an existing local Minecraft installation, or
   - create an empty instance and configure it afterward.
3. For a new or empty Java instance, open its **Configure** or **Game setup** control. Select the Minecraft version and Vanilla, Fabric, or Forge setup, then install. These files go into that instance's selected game folder.
4. Select the instance and choose a compatible Java runtime in its settings. Use the runtime version required by that Minecraft version. Set the memory allocation as needed and save.
5. Select the account for the game type. Microsoft accounts are used for Java Minecraft; Horizon continues to use local profiles. Local profiles do not authenticate with Microsoft.
6. Open **Downloads** to review the current install or preparation progress. Launch output and the process exit status are available in the local log view.

Before a Java launch, OpnLauncher checks that the selected profile has its official Minecraft version metadata, client JAR, libraries, logging configuration, asset index, and referenced asset files. If any are missing, it downloads the official Mojang files and the selected Fabric/Forge library artifacts into that instance, verifies downloads, and then starts the game. Repair progress appears in **Downloads** and can be cancelled. Existing regular files, mods, settings, saves, and worlds are left in place. This repair restores missing files; it does not replace files that already exist but have been modified or corrupted, or recreate a missing third-party loader profile.

The **Versions** page can install official vanilla releases, snapshots, and supported historical versions. A completed version download creates a new managed instance. It does not replace the official launcher's files. For modded play, configure Fabric or Forge for that instance, then use the **Mods** page; compatibility follows the selected instance.

## Manage instances

- **Select** an instance from the library or the current-instance selector on pages that provide one.
- **Configure** a Java instance to change its Minecraft version or loader. Existing files that do not match expected upstream integrity data are left alone; saves and worlds are kept in place.
- Choose Java and memory in the instance settings. The Java manager in **Settings** can add and inspect a custom Java executable and set the default for new instances.
- **Rename** or favorite an instance from the library. The Storage settings list detected Minecraft locations and managed instance folders.
- **Remove from library** forgets the launcher record and keeps the game directory on disk. This avoids silently deleting worlds, mods, or configuration files. To delete files, review and remove them yourself in the file manager after confirming the path.

Registering an existing Minecraft folder points an instance at that folder; it does not copy it into an isolated managed directory. A registered folder can therefore be shared with another launcher or instance. Check the selected path before installing loaders or changing mods. New version downloads use separate managed instance folders.

## Use the mod browser

1. Select a Java instance that already has a Minecraft version and, if needed, Fabric or Forge installed.
2. Open **Mods**. The instance context at the top shows which Minecraft and loader versions drive compatibility filtering.
3. Choose **Modrinth** or **CurseForge**, search, and open a project to review its description, artwork, and available builds.
4. Use **Install** on a compatible build. OpnLauncher checks the selected instance's Minecraft and loader context and installs required provider dependencies where available.
5. Use **Installed** to search local mod files, enable or disable them, switch a mod to another compatible release, select multiple files, or remove files after confirmation.

CurseForge requires a personal API key. Enter it in the provider prompt; it stays in app memory for the current run, is sent to CurseForge for its API requests, and is not saved in launcher preferences or the SQLite database. Mod files are third-party executable code. Install only projects you trust. The launcher does not sandbox mods or guarantee that a mod is safe.

Horizon profiles have native `.hmod` content for the Rust client. They also have an **optional Java + Fabric runtime**. It downloads official Minecraft 26.3 files and Fabric Loader into a private per-profile folder, then uses the normal Modrinth/CurseForge mod manager for that Java game directory. Fabric API, Sodium, and other compatible JARs are run unchanged by the Java Minecraft client. This mode is **off until installed and explicitly selected**; turning it off returns Play to the native Rust Horizon client. The Java runtime uses the selected unverified local profile and does not add Microsoft sign-in. This full Java profile is distinct from the experimental native-Horizon bridge below.

The Java + Fabric profile starts a separate Java Minecraft game; it does not use the native-Horizon bridge. The profile's native Rust client and Java runtime have separate game folders, worlds, and mod files. Minecraft 26.3 requires Java 25 or newer.

The earlier static resource adapter remains opt-in for the native Rust client. It only adapts standalone, code-free `assets/` and `data/` files from Fabric JARs into Horizon `.hmod` content; JARs with Java classes, nested JARs, entrypoints, mixins, access wideners, unsupported paths, or other Fabric dependencies are skipped. Some static resource formats may also be rejected by Horizon's validator.

### Experimental Fabric code bridge for Rust Horizon

The Fabric tab on **Horizon → Mods** also has a separate, opt-in **Fabric code bridge**. With native Rust Horizon selected, OpnLauncher scans JAR metadata and class references, then copies only accepted unchanged files to `.opnlauncher/fabric-code-bridge/mods/`. It supplies a small Fabric API and Minecraft facade instead of loading Minecraft's Java client, then starts Fabric Loader in a helper JVM. Horizon captures bounded player position, velocity, look-direction, health, movement-input, and state snapshots, plus world time, build-height, and weather data at the start and end of its native client update and queues the corresponding `ClientTickEvents.START_CLIENT_TICK` and `END_CLIENT_TICK` callbacks in order. The helper JVM processes callbacks off the Horizon game thread, so delivery can lag or callbacks can be dropped if the bounded queue fills. Callback results return to Horizon without making its game thread wait for Java. Mapped calls include player getters (`getDeltaMovement()` returns a read-only velocity vector; `getXRot()` and `getYRot()` read look direction; `blockPosition().getX/Y/Z()` provides the block containing the latest position snapshot, including correct negative-coordinate flooring), `setXRot(float)` and `setYRot(float)` local look updates, `Minecraft.level.getDayTime()` / `getGameTime()`, build-height limits, and rain/thunder state and levels, vanilla movement bindings (`Minecraft.options.keyUp`, `keyDown`, `keyLeft`, `keyRight`, `keyJump`, `keyShift`, and `keySprint`) plus programmatic sprint-key state through `KeyMapping.set(InputConstants.Key, boolean)` and `KeyMapping.setDown(boolean)`, simple HUD text through `Minecraft.gui.hud.setOverlayMessage(Component.literal(text), animate)`, plain system chat through `Entity.sendSystemMessage(Component)`, registered keyboard bindings through Fabric's `KeyMappingHelper`, and `Entity.setSprinting(boolean)` and `Entity.setDeltaMovement(Vec3)`. The immutable `Vec3` facade also supports `add(double, double, double)` for calculating adjusted velocity values. Movement bindings expose held state and queued press edges using Horizon's mapped vanilla actions; they do not add an in-game Minecraft Options screen. Sprint, velocity, and look requests are applied on Horizon's Rust game thread during active play; velocity magnitude is capped at 4 blocks per tick, yaw is limited to ±360,000 degrees, and pitch to −90 through 90 degrees. Look updates affect only the local player; they do not change other entities or authoritative server state. System chat is plain text and follows Horizon's chat visibility setting. This bridge runs unchanged Java mod code in a helper JVM and translates only allowlisted API calls into Rust game actions; it does not transpile arbitrary Java bytecode or emulate general Minecraft APIs.

Only Fabric `main`/`client` initializers, `ClientTickEvents.START_CLIENT_TICK` and `END_CLIENT_TICK`, the listed local-player getters (`getDeltaMovement()`, yaw, pitch, and `blockPosition().getX/Y/Z()`), selected `Level` environment accessors, and `Level.getBlockState(BlockPos)` with `BlockState.getBlock()/isAir()/isSolid()/isSolidRender()` are accepted. Block reads are limited at runtime to loaded cells captured within a 3×3×3 area around the player; querying any other cell throws an explicit unsupported-operation error. The state facade exposes an identifier and basic flags only, without block properties, fluid state, chunk APIs, or world mutations. `sendSystemMessage(Component)`, `setSprinting(boolean)`, `setDeltaMovement(Vec3)`, `setYRot(float)`, `setXRot(float)`, and the listed `Minecraft.options` movement fields with `KeyMapping.isDown()` and `consumeClick()` are also accepted. `KeyMapping.set(InputConstants.Key, boolean)` and `setDown(boolean)` are accepted; only writes to the native sprint binding become Horizon actions, while other writes affect only the helper facade. The `BlockPos` facade exposes only its integer constructor and `getX()`, `getY()`, and `getZ()`; the immutable `Vec3` facade supports its constructor, `x/y/z`, `add(double, double, double)`, length, squared-length, and horizontal-distance operations. Velocity requests with non-finite values or magnitude above 4 blocks per tick are ignored; accepted requests are applied by the Rust game thread only during active play. Rotation requests must have finite yaw within ±360,000 degrees and pitch between −90 and 90 degrees; they update the local player's look on Horizon's game thread during active play. They do not rotate other entities or send authoritative server changes. System messages are plain text and obey Horizon chat visibility. The callback gets a limited `Minecraft` facade, not a real Java game client. The movement fields report Horizon's mapped vanilla movement actions; Key mappings cannot yet be rebound or persisted in a Horizon controls screen; arbitrary key contexts, mouse, and controller mappings are not implemented. The allowlist rejects Mixin configs, access wideners, nested dependencies, unmapped Minecraft class/member use, and other Fabric API classes. Level tick events and other lifecycle events fail with an explicit unsupported-operation message. The Fabric API JAR and Minecraft client JAR are not loaded by the helper. Beyond the captured nearby block states, general block/chunk access, other entities, dimensions, registries, rendering, text styles, and most component operations remain unmapped. Sodium is unsupported. This is an experimental API mapping, not general Fabric support. Probe and loopback tests use synthetic Rust snapshots/input; they do not verify a live Horizon session or a third-party mod.

To try the prototype, install Java 25+ and the profile's Minecraft 26.3 Fabric Loader from **Horizon → Settings**, install a mod from the Horizon **Mods** tab, and review the compatibility status. **Mapped API subset** means the mod's metadata and static references pass the current allowlist; it does not guarantee the mod has a visible effect. Enable **Fabric code bridge** on that profile and leave **Use Java + Fabric for this profile** off so **Play** starts Rust Horizon. The bridge is disabled by default and can be turned off at any time. It copies accepted JARs into its private staging folder; original profile JARs are not rewritten or deleted.

For a known test instead of a third-party mod, build the development probe after building OpnLauncher with `npm run fabric:build:probe`. This creates `src-tauri/target/horizon-fabric-bridge-probe.jar`; copy it into the selected Horizon profile's `mods/` folder and start the profile with the code bridge enabled. It should show a player snapshot (name, position, nearby block ID and flags, velocity, horizontal speed, health, sprint, and ground state) plus world time, build-height, and rain/thunder values in Horizon's action bar when the local player is available. On each end tick with a player snapshot, the probe exercises the common auto-sprint pattern by setting the sprint binding's default key and held state; the bridge sends that change to Horizon. Press and hold **P** to see the held-key message; while moving forward, tap **P** to check that the probe reads the vanilla forward/sneak bindings and consumes the forward press edge, derives a vertical velocity adjustment from the current snapshot with `Vec3.add`, displays an action-bar response, and sends a plain system-chat message. When no player is present, the probe shows a connection message. The probe is a test fixture, not bundled or installed automatically.

`npm run test:fabric-bridge` also builds the probe as a separate JAR, runs six Rust protocol unit tests, and runs the production Rust IPC module with the Java helper over a loopback socket. This checks the Fabric entrypoint, start/end tick ordering, player motion/orientation, world/environment and bounded nearby-block snapshots, held movement state and press-edge mapping through `Minecraft.options`, bounded velocity/look requests, system-chat messages, and action-bar responses together without editing an installed mod. This quick check uses a Fabric Loader stub and synthetic Horizon state.

`npm run test:fabric-bridge:real-loader` runs the unchanged project probe through official Fabric Loader 0.20.1 and the published Minecraft 26.3 Fabric API modules before connecting it to the Rust bridge. The probe exercises Minecraft's programmatic sprint-key calls. It requires Java 25+, `javac`, `jar`, and `rustc`, plus a project-local test cache at `src-tauri/target/fabric-real-loader-check` containing the official Loader libraries and `fabric-api-0.165.2+26.3.jar`. Set `OPN_FABRIC_BRIDGE_TEST_CACHE` to a different cache directory inside this project if necessary. The test compiles the probe against the published API modules, then removes its temporary output from `src-tauri/target`. It validates real Loader startup and the mapped API linkage against synthetic game state; it does not start a live Horizon session or establish compatibility with third-party gameplay mods.

Java mods are executable code running with your user account's permissions. They can read or modify files accessible to your account and can make network requests. The bridge does not sandbox mods. Enable it only for mods you trust; OpnLauncher itself sends no additional request for the event bridge.

### Try Fabric mods on a Horizon profile

1. Open **Horizon**, select the profile, then scroll to **Selected profile settings → Fabric mods on this profile**.
2. Select Java 25 or newer and a Fabric Loader version for Minecraft 26.3, then choose **Install Java + Fabric files**.
3. Turn on **Use Java + Fabric for this profile**. This selects the Java Minecraft backend for this profile.
4. Existing JARs from the profile's `mods/` folder are copied to `.opnlauncher/fabric-java/mods/`; their originals remain untouched. Open **Mods** to install or manage compatible Fabric projects in the isolated folder.
5. Turn off the runtime option to return to native Rust Horizon. The Java installation and mod files are kept so you can switch back later.

## Set up Horizon (experimental)

Horizon is a separate native Rust game engine, not a Java loader. OpnLauncher includes the required Horizon source from upstream commit `fdd290b99b4459cb4803487758ba8015a45aa47f`, builds the game runtime as a separate process, and bundles it with desktop builds. The Horizon resource preparer is called as a Rust library; users do not need to build or configure Horizon executables.

You need:

- a JDK 25 installation, including `javac`, for resource preparation;
- a local OpnLauncher developer profile.

Then:

1. In **Settings → Java manager**, add or select a Java 25 executable for preparation.
2. Open **Versions → Create Horizon profile**, or use **Add an instance** and choose Horizon.
3. In **Horizon**, select a detected Minecraft 26.3 folder or add a folder. OpnLauncher can instead download the official files into its private cache.
4. Choose the Java 25 runtime and select **Download files & prepare Horizon**.
5. Select the Horizon profile and local developer profile, then launch. The client and prepared resource directory are managed automatically.

When reusing a local Minecraft installation, OpnLauncher copies only files that match Mojang's published size and SHA-1 metadata into its private Horizon cache. It does not write into the selected source folder. Missing or invalid files are downloaded from Mojang. Before preparation, OpnLauncher ensures the exact Minecraft 26.3 version manifest pinned by Horizon's included research lock is available in its private preparer cache, fetching it from Mojang on first use and checking its SHA-1 and client/server artifact identities. Preparation also imports Horizon's sounds from the verified Minecraft asset store (about 380 MB) before checking that resources are complete. Horizon profiles keep their worlds and personal settings separate from Java instances. The integration is experimental and fixed to Minecraft 26.3. The maintainer has reported successful local play, but this preparation pass has not independently verified a live game session; see [Horizon integration notes](docs/HORIZON.md). Horizon's MIT license and third-party notices are in `src-tauri/horizon-upstream/` and summarized in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

### Horizon audio and preparation troubleshooting

Linux builds need ALSA development files to include Horizon audio. Fedora users can install them with `sudo dnf install alsa-lib-devel pkgconf-pkg-config`; Debian or Ubuntu users can use `sudo apt install libasound2-dev pkg-config`. Then rebuild the AppImage. The build now stops with an install hint when ALSA is missing instead of silently omitting audio.

If preparation reports `Horizon resources are incomplete: the sounds missing: horizon-prepare sounds`, update to a source version containing the sound-import fix and run **Download files & prepare Horizon** again. OpnLauncher imports the sounds from the verified local Minecraft asset store into its Horizon resources before checking completeness.

## Local accounts and online play

Local developer profiles are available in both development and release builds for offline testing. Their names and stable offline UUIDs are stored locally and are clearly marked unverified. They do **not** sign in to Microsoft, verify Minecraft ownership, provide an authenticated session, or enable authenticated online-server access. Use them for local single-player testing or appropriately configured local development servers only. OpnLauncher does not bypass ownership checks.

### Microsoft sign-in (experimental)

Microsoft sign-in uses the system browser and the authorization-code flow with PKCE. OpnLauncher checks Xbox authentication, Minecraft Java entitlement, and the selected Minecraft profile before saving account metadata. It never asks for a Microsoft password or client secret. A refresh token is kept in the Linux desktop Secret Service keyring and is not sent to the frontend, stored in SQLite, or written to launcher logs. On each Java launch the launcher refreshes the session before building the game process.

This flow currently supports secure token storage on Linux only. Windows and macOS sign-in are not ready. It has not yet been verified with a live account in this development pass; use it only for testing and report any failure message. Horizon still uses a local unverified profile and does not receive a Microsoft token. Microsoft authentication is not an ownership bypass: online authentication depends on Minecraft's own session and server rules.

To configure the app ID:

Open the [Microsoft Entra app registrations page](https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade) to create or manage the registration. Microsoft's [desktop app registration guide](https://learn.microsoft.com/en-us/entra/identity-platform/scenario-desktop-app-registration) has the current portal walkthrough.

1. In Microsoft Entra, create an app registration that accepts **Personal Microsoft accounts only**.
2. Add the **Mobile and desktop applications** platform and register the loopback redirect `http://localhost`. Entra accepts the dynamic local port used by the app.
3. Enable public-client authorization for the registration. Do not create or enter a client secret; desktop apps cannot keep one confidential.
4. Copy the **Application (client) ID**. In OpnLauncher, open **Accounts**, paste it into **Application (client) ID**, then choose **Continue with Microsoft**. Microsoft sign-in opens in your system browser.

See Microsoft's [desktop-app setup](https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-desktop-app-sign-in), [public-client configuration](https://learn.microsoft.com/en-us/entra/identity-platform/scenario-desktop-app-configuration), and [localhost redirect rules](https://learn.microsoft.com/en-us/entra/identity-platform/reply-url). Xbox authentication is part of the flow and sends requests directly to Microsoft/Xbox and Minecraft services.

## Privacy and network activity

OpnLauncher has no analytics, telemetry, ads, tracking, crash-reporting service, launcher account, or automatic diagnostic upload. Settings, launcher logs, profile records, instance metadata, and optional playtime history are local. Playtime tracking is off by default and can be paused from Overview; it records game process start/end times only. No OpnLauncher-operated service is required.

Network requests happen when you use the feature that needs them:

- Mojang's official metadata and file hosts for version browsing, vanilla installation, and Horizon resource preparation;
- Microsoft identity, Xbox authentication, and Minecraft services when you sign in or start a Java game with a Microsoft account;
- Fabric Meta or Forge Maven when browsing or installing those loaders;
- Modrinth or CurseForge when searching for or installing provider content;
- provider image/CDN hosts when viewing project artwork.

The CurseForge key is only sent to CurseForge for the API. Network behavior and stored data are described in [PRIVACY.md](PRIVACY.md). You can manage local appearance, Java, loader, and storage settings in the app. Automatic update checks are not implemented.

## Where files are stored

- The SQLite database, local launcher logs, managed instance folders, and shared download/cache data are placed in Tauri's per-user application data folder for this app (`org.opnlauncher.launcher`). The exact location depends on your operating system.
- **Settings → Storage** shows detected Minecraft folders and registered/managed instance paths.
- New vanilla version installs use a new managed instance directory. Mods are stored in the selected Java instance's `mods` directory. Horizon profiles store worlds and settings in their own instance directories; shared Horizon support files use a separate application-data subfolder.
- Theme, accent, Java defaults, custom Java paths, favorites, playtime chart range, and selected profile/instance preferences are stored in the local desktop WebView. They are not uploaded.
- Opt-in playtime sessions are stored in the local SQLite database. They are not associated with an online reporting service and are preserved when tracking is turned off.
- Registered existing Minecraft folders remain where they already are. Forgetting an instance preserves its files. Mod deletion and bulk deletion remove only selected mod JARs after confirmation.

Keep your own backups of worlds and important configuration. The prototype does not yet provide instance backup and restore.

## Development checks

Install the locked JavaScript dependencies and run the frontend checks:

```sh
npm ci
npm run build
npm run test
npm run test:fabric-bridge
npm run test:fabric-bridge:real-loader
```

The bridge check requires a JDK (including `javac` and `jar`), `rustc`, and permission to open a local loopback socket. The test's temporary classes, JAR, and executable are created under the ignored `src-tauri/target` folder and removed afterward.

Run Rust formatting, library tests, and the desktop backend check:

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo fmt --manifest-path src-tauri/horizon-runtime/Cargo.toml -- --check
cargo test --locked --no-default-features --lib --manifest-path src-tauri/Cargo.toml
npm run horizon:build:release
cargo check --locked --features desktop --manifest-path src-tauri/Cargo.toml
```

GitHub Actions runs the same Linux checks on pushes and pull requests. A separate manually started workflow builds a Linux AppImage artifact for inspection; it does not publish a release. These automated checks do not prove that provider downloads, loader installers, a Tauri window, or a real Minecraft/Horizon session work on every machine. Verify those separately before using important data.

## Repository and contribution policy

Forgejo remains the primary development repository. A GitHub copy is a deliberate public development snapshot for review and community contributions; it is not automatically mirrored, and GitHub does not replace Forgejo as the source of truth. Public releases and tags are created only after maintainer review. See [CONTRIBUTING.md](CONTRIBUTING.md) for the contribution workflow and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community expectations.

The project uses Semantic Versioning and is currently in the `0.1.0` development series. The human-readable history is in [CHANGELOG.md](CHANGELOG.md). Development commits do not imply a release, and this repository does not create release tags automatically.

Useful project references:

- [Architecture](ARCHITECTURE.md)
- [Privacy](PRIVACY.md)
- [Security policy](SECURITY.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)
- [0.1.0-dev.41 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.41.md)
- [0.1.0-dev.40 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.40.md)
- [0.1.0-dev.39 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.39.md)
- [0.1.0-dev.38 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.38.md)
- [0.1.0-dev.37 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.37.md)
- [0.1.0-dev.36 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.36.md)
- [0.1.0-dev.35 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.35.md)
- [0.1.0-dev.32 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.32.md)
- [0.1.0-dev.31 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.31.md)
- [0.1.0-dev.30 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.30.md)
- [0.1.0-dev.29 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.29.md)
- [0.1.0-dev.28 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.28.md)
- [0.1.0-dev.27 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.27.md)
- [0.1.0-dev.26 development preview notes](docs/RELEASE_NOTES_0.1.0-dev.26.md)
- [Feature audit](docs/FEATURE_AUDIT.md)
- [Roadmap](docs/ROADMAP.md)
- [Horizon setup and current limits](docs/HORIZON.md)

## Current status and limitations

OpnLauncher remains an early, incomplete prototype. Microsoft authentication is experimental, Linux-only for secure token storage, and still needs a live-account launch test. Horizon `.hnmod` installation/building is not implemented; its native compiler needs Horizon's separate, pinned build infrastructure. Modpack management, NeoForge and Quilt installation, resource/shader pack support, backups, instance import/export, automatic update management, and Windows/macOS validation remain unavailable. Very old Minecraft metadata may not install correctly. Java selection is currently explicit/manual; downloads do not provide pause/resume, and automatic repair does not replace modified/corrupt files or reconstruct missing third-party loader metadata.

Horizon support is experimental and currently fixed to Minecraft 26.3. The native client accepts its own Rust `.hmod` mods and a small subset of code-free Fabric resources. The optional Fabric code bridge starts a helper JVM and maps client tick callbacks, player position/block-position/velocity/look-direction/health snapshots, world time/build-height/weather, and read-only block states in a bounded 3×3×3 area around the player. It also maps `Minecraft.options` movement snapshots (`isDown()` and `consumeClick()`), namespaced `Identifier`-based key categories, programmatic sprint-key writes, bounded `Vec3.add(double, double, double)` calculations and `setDeltaMovement(Vec3)`, `setYRot(float)` and `setXRot(float)` requests, plain system chat, and simple HUD text. Block queries outside that captured area, block properties, fluids, world mutations, Mixins, graphics APIs, and Sodium remain unsupported; this is not general Fabric gameplay compatibility. Registered key categories are not exposed in a Horizon controls screen and cannot be rebound yet. The separate Java + Fabric runtime remains available as its own game backend. The bridge has automated checks against a test Loader and synthetic game state; a live Horizon session and third-party mod remain unverified. Automated checks cover the Linux development path; Windows/macOS, all loader/provider flows, and a complete security audit remain open. See the [feature audit](docs/FEATURE_AUDIT.md) for evidence and known gaps.

## License

OpnLauncher is licensed under the GNU General Public License, version 3 or (at your option) any later version. See [LICENSE](LICENSE).
