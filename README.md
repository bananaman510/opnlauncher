# OpnLauncher

OpnLauncher is an independent, local-first desktop launcher for Minecraft Java Edition. It is built with Tauri 2, Rust, React, TypeScript, and SQLite, with Linux as the current development platform.

The project is an **early development prototype**. The vanilla install and launch workflows, Fabric and Forge setup, provider-backed mod management, and a separate experimental Horizon profile path are being developed incrementally. Some important flows have not yet been validated in a live desktop session with real game files. Read [Current status and limitations](#current-status-and-limitations) before relying on it for valuable worlds.

OpnLauncher is not affiliated with, endorsed by, or associated with Mojang Studios or Microsoft. Minecraft is a trademark of Mojang Synergies AB. This project does not bundle or redistribute Minecraft game files. When you request an installation, it fetches official files using Mojang's version metadata.

## What works today

| Area | Available now |
| --- | --- |
| Minecraft versions | Browse Mojang's releases, snapshots, and older alpha/beta entries; install supported vanilla versions into new managed instance folders. Very old metadata formats may not work. |
| Instances | Create an empty profile, install a version, inspect/register a local installation, rename it, favorite it, choose Java and memory, and remove its library entry while preserving its files. |
| Mod loaders | Install Fabric or Forge for a selected instance. Detect existing Fabric, Forge, NeoForge, and Quilt profiles; NeoForge and Quilt installation is not implemented yet. |
| Mods | Browse Modrinth and CurseForge for a selected instance's Minecraft version and loader; install compatible builds and required dependencies; view project art; enable, disable, remove, bulk-delete, and switch installed mod versions. CurseForge requires your own API key for the current session. |
| Java | Discover Java runtimes, inspect a custom executable, set a default or per-instance runtime, and choose memory for each instance. |
| Local profiles | Create clearly labeled, unverified local developer profiles in development and release builds. Microsoft authentication and ownership checks are not implemented. |
| Horizon | Create separate Minecraft 26.3 profiles for the native Rust-engine Horizon client, prepare official support files, and manage local `.hmod` content files. Horizon's Rust source is included and built with OpnLauncher; Java `.jar` mods and Java loaders do not apply. |
| Preferences and diagnostics | Store appearance and launcher preferences locally; view local launch output and process exit status. There is no automatic diagnostic upload. |

The interface includes an overview, instance library, Minecraft version browser, mod browser, Horizon engine settings, downloads view, local profiles, and settings for appearance, Java, loaders, storage, privacy, and about.

## Get OpnLauncher running on Linux

### Requirements

- Linux desktop with GTK 3 and WebKitGTK 4.1 development libraries.
- Rust stable **1.90 or later**, including Cargo.
- Node.js **20 or later** and npm.
- Java for launching Minecraft. Horizon resource preparation additionally needs a JDK 25 installation, including `javac`.
- An internet connection for downloading the version list, game files, loaders, or mods. Local instance management and settings work without downloading new content.

Install Rust with [rustup](https://rustup.rs/) if it is not already installed. Install Node.js and npm using your distribution or the [official Node.js downloads](https://nodejs.org/en/download/).

### Fedora

Install the native Tauri build dependencies. `dbus-devel` and `pkgconf-pkg-config` provide the D-Bus development metadata and `pkg-config`. ALSA and systemd development packages enable Horizon audio and controller support; if either is missing, the build continues without that optional feature.

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

Replace the example filename with the generated file's name. This produces a **local development build**; this repository does not currently provide a published AppImage download. AppImage portability depends on the system it was built on and the target system's libraries and desktop environment. Build and run it on the Linux distributions you intend to support. See Tauri's [AppImage guide](https://v2.tauri.app/distribute/appimage/) for its packaging details and limitations.

## First launch and a basic play setup

1. Open **Accounts** and create a local developer profile. It is visibly marked **unverified**. No Microsoft sign-in is available in the current prototype.
2. Open **Instances** and choose **Add an instance**.
   - Choose the official Minecraft version browser to create a new managed instance, or
   - inspect and register an existing local Minecraft installation, or
   - create an empty instance and configure it afterward.
3. For a new or empty Java instance, open its **Configure** or **Game setup** control. Select the Minecraft version and Vanilla, Fabric, or Forge setup, then install. These files go into that instance's selected game folder.
4. Select the instance and choose a compatible Java runtime in its settings. Use the runtime version required by that Minecraft version. Set the memory allocation as needed and save.
5. Select your local profile, return to the overview or instance page, and press **Play**.
6. Open **Downloads** to review the current install or preparation progress. Launch output and the process exit status are available in the local log view.

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

Horizon profiles use the separate Horizon Mods page. It manages local `.hmod` content files only. Java `.jar` mods and Java mod loaders do not work with Horizon; native `.hnmod` code mods are not managed by OpnLauncher.

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

When reusing a local Minecraft installation, OpnLauncher copies only files that match Mojang's published size and SHA-1 metadata into its private Horizon cache. It does not write into the selected source folder. Missing or invalid files are downloaded from Mojang. Horizon profiles keep their worlds and personal settings separate from Java instances. The integration is experimental and has not yet been validated in a live Horizon game session; see [Horizon integration notes](docs/HORIZON.md). Horizon's MIT license and third-party notices are in `src-tauri/horizon-upstream/`.

## Local accounts and online play

Local developer profiles are available in both development and release builds for offline testing. Their names and stable offline UUIDs are stored locally and are clearly marked unverified. They do **not** sign in to Microsoft, verify Minecraft ownership, provide an authenticated session, or enable authenticated online-server access. Use them for local single-player testing or appropriately configured local development servers only. OpnLauncher does not bypass ownership checks.

Microsoft OAuth, ownership/entitlement verification, secure token storage, account switching, and authenticated online launch are not implemented. Do not enter a Microsoft password or token into OpnLauncher.

## Privacy and network activity

OpnLauncher has no analytics, telemetry, ads, tracking, crash-reporting service, launcher account, or automatic diagnostic upload. Settings, launcher logs, profile records, and instance metadata are local. No OpnLauncher-operated service is required.

Network requests happen when you use the feature that needs them:

- Mojang's official metadata and file hosts for version browsing, vanilla installation, and Horizon resource preparation;
- Fabric Meta or Forge Maven when browsing or installing those loaders;
- Modrinth or CurseForge when searching for or installing provider content;
- provider image/CDN hosts when viewing project artwork.

The CurseForge key is only sent to CurseForge for the API. Network behavior and stored data are described in [PRIVACY.md](PRIVACY.md). You can manage local appearance, Java, loader, and storage settings in the app. Automatic update checks and Microsoft authentication are not implemented.

## Where files are stored

- The SQLite database, local launcher logs, managed instance folders, and shared download/cache data are placed in Tauri's per-user application data folder for this app (`org.opnlauncher.launcher`). The exact location depends on your operating system.
- **Settings → Storage** shows detected Minecraft folders and registered/managed instance paths.
- New vanilla version installs use a new managed instance directory. Mods are stored in the selected Java instance's `mods` directory. Horizon profiles store worlds and settings in their own instance directories; shared Horizon support files use a separate application-data subfolder.
- Theme, accent, Java defaults, custom Java paths, favorites, and selected profile/instance preferences are stored in the local desktop WebView. They are not uploaded.
- Registered existing Minecraft folders remain where they already are. Forgetting an instance preserves its files. Mod deletion and bulk deletion remove only selected mod JARs after confirmation.

Keep your own backups of worlds and important configuration. The prototype does not yet provide instance backup and restore.

## Development checks

Run the frontend checks:

```sh
npm run build
npm run test
```

Run the Rust library tests and desktop backend check:

```sh
cargo test --offline --no-default-features --lib --manifest-path src-tauri/Cargo.toml
cargo check --offline --features desktop --manifest-path src-tauri/Cargo.toml
```

These checks do not prove that provider downloads, loader installers, a Tauri window, or a real Minecraft/Horizon session work on a particular machine. Verify those separately before using important data.

## Repository and contribution policy

Development currently uses a private, self-hosted Forgejo repository. Forgejo is the primary development remote; there is no automatic GitHub mirror or publication. GitHub publication is planned after the project is sufficiently stable and has completed its release-preparation review. See [CONTRIBUTING.md](CONTRIBUTING.md) for the project workflow and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community expectations.

The project uses Semantic Versioning and is currently in the `0.1.0` development series. The human-readable history is in [CHANGELOG.md](CHANGELOG.md). Development commits do not imply a release, and this repository does not create release tags automatically.

Useful project references:

- [Architecture](ARCHITECTURE.md)
- [Privacy](PRIVACY.md)
- [Security policy](SECURITY.md)
- [Feature audit](docs/FEATURE_AUDIT.md)
- [Roadmap](docs/ROADMAP.md)
- [Horizon setup and current limits](docs/HORIZON.md)

## Current status and limitations

OpnLauncher is not yet feature-complete. Microsoft authentication, modpack management, NeoForge and Quilt installation, resource/shader pack support, backups, instance import/export, automatic update management, and cross-platform validation remain planned or unavailable. Very old Minecraft metadata may not install correctly. Java selection is currently explicit/manual, and version installation does not provide pause/resume or a complete repair workflow.

Horizon support is experimental and has not been validated with a live game session. The project has no CI workflow or published release packages at this stage. See the [feature audit](docs/FEATURE_AUDIT.md) for the source-reviewed details and known gaps.

## License

OpnLauncher is licensed under the GNU General Public License, version 3 or (at your option) any later version. See [LICENSE](LICENSE).
