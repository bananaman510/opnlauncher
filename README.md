# OpnLauncher

OpnLauncher is an independent, local-first launcher for Minecraft Java Edition, with an experimental separate profile path for the Rust-powered Horizon Minecraft client. It is built with Tauri 2, Rust, React, TypeScript, and SQLite.

This repository is an early **development prototype**. It can browse Mojang's official vanilla version list and install compatible releases, snapshots, or historical versions into a new, separate instance directory. It also includes Modrinth mod discovery and compatible mod installation, plus CurseForge discovery when you provide your own API key. Fabric and Forge loader builds can be installed into a selected instance, and existing Fabric, Forge, NeoForge, and Quilt profiles can be selected from the loader manager. Modpacks are still planned.

OpnLauncher is not affiliated with, endorsed by, or associated with Mojang Studios or Microsoft. Minecraft is a trademark of Mojang Synergies AB. Minecraft game files are not bundled or redistributed by this project. When requested, OpnLauncher downloads official vanilla game files from URLs in Mojang's version metadata.

## Current features

- Desktop interface with a compact icon rail, instance library, versions, mods, modpacks, accounts, downloads, and categorized settings.
- Optional light theme, four accent colors, saved instance favorites, grid or list layouts, recent-instance sorting, and Ctrl/Cmd+K instance search.
- SQLite storage for local instance metadata and development profiles.
- Detection of common Minecraft directories and installed version metadata.
- Manual inspection of another local Minecraft game directory.
- Search and filter Mojang's official releases, snapshots, and older alpha/beta versions.
- Install supported vanilla versions into a new isolated instance folder with bounded parallel downloads, progress, retry, cancellation, and upstream SHA-1/size checks.
- Search Modrinth and CurseForge mods by Minecraft version and loader, inspect project images and build details, and filter results by category and popularity.
- Install compatible mod builds with required dependencies, checksum/size verification, progress and cancellation; manage existing JARs by enabling, disabling, removing one or many, and switching versions.
- Compare a mod's available releases with sortable and filterable Minecraft version, loader, and release-type controls. Switching replaces the selected mod only after the new file passes its integrity check.
- Disable duplicate installs in Discover when the project is already present in the selected instance, including common Fabric JAR filename patterns.
- Use CurseForge's official API with your own session-only key. The key stays in app memory, is sent only to CurseForge, and is not stored in the app database or preferences.
- Detect Fabric, Forge, NeoForge, and Quilt from each installed version's metadata so search filters and compatible builds follow the selected instance.
- Java runtime discovery through `JAVA_HOME`, `PATH`, and common Linux locations.
- Java manager with custom executable validation, a preferred Java runtime, and per-instance Java selection.
- Saved defaults for new instance Java selection and memory allocation.
- Fabric and Forge version browsing and installation for the selected Minecraft version; installed loader profiles can be selected without removing worlds or mods.
- Per-instance game setup for a chosen Minecraft version and Vanilla, Fabric, or Forge, followed by mod search filtered to that instance's installed setup.
- Visibly unverified local developer profiles with stable offline UUIDs, available in development and release builds.
- Separate Horizon 26.3 Rust-engine profiles with their own worlds, local settings, player files, and `.hmod` content-mod list.
- Horizon resource preparation from official Mojang Minecraft 26.3 client, library, asset, and server files, with upstream integrity verification.
- Saved local profile and instance selection for convenient testing between launches.
- Local launch logs and process exit status.
- No telemetry, analytics, ads, launcher account, or automatic diagnostic upload.

Developer profiles are available in release builds for local single-player testing and are always marked unverified. They do not authenticate with Microsoft, verify game ownership, or provide authenticated online-server access. Use them only with local single-player worlds or appropriately configured test servers. This feature does not bypass ownership checks.

## Linux development setup

You need Rust, Node.js with npm, Java for running Minecraft, and Tauri's Linux development libraries. The command below is for Fedora. For other distributions, follow the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) and install the matching GTK/WebKit development packages.

```sh
sudo dnf install webkit2gtk4.1-devel dbus-devel pkgconf-pkg-config openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel
sudo dnf group install "c-development"
```

From the repository root:

```sh
npm install
npm run tauri -- dev
```

The Vite frontend can also be opened in a browser with `npm run dev`, but local filesystem and process commands are available only in the desktop app.

Useful checks:

```sh
npm run build
npm run test
cargo test --no-default-features --lib --manifest-path src-tauri/Cargo.toml
```

`cargo tauri dev` is an alternative after installing the Tauri CLI. The npm script uses the project-local CLI.

## Local data

The app stores its SQLite database, managed instance directories and downloaded vanilla game files, launch logs, and generated native-library cache in Tauri's application data directory. Downloads from the general version browser go into a separate new instance directory. When you choose Game setup for a specific instance, verified Minecraft and loader files are added to that instance's selected game folder; existing files that do not match the expected integrity data are left untouched. A registered local folder may be shared with another instance, and the app warns when it detects this. Horizon keeps its shared Minecraft 26.3 support files and prepared resources under an `horizon` subfolder in the same application data directory; each Horizon profile's worlds and personal files remain inside its own instance directory.

Interface and default preferences, including theme, accent color, default Java, default memory, custom Java paths, instance favorites, and selected instance and local profile, are stored locally in the desktop WebView. They are not uploaded. Mod files and their provider project/version references are stored in the selected instance's `mods` folder and SQLite database. Mod deletion is limited to the confirmed JAR files selected in the Installed view. A version switch stages and verifies the replacement before removing the previous mod file. Fabric libraries and loader profiles, or Forge installer output, are written to the selected game folder after you choose Install. Existing worlds are kept in place.

## Development status

The first milestone is still in progress. The vanilla installer handles official Minecraft version metadata, libraries, assets, and client files. Per-instance setup selects Minecraft, Fabric, or Forge before installation, and the Mods page follows the selected instance. Horizon integration currently requires local `horizon-client` and `horizon-prepare` executables, plus a JDK 25 for resource preparation; OpnLauncher does not build or bundle Horizon. Horizon's launch API and resource preparation have not yet been verified against a live Horizon binary or in-game session. See [Horizon setup](docs/HORIZON.md) for the current scope and requirements. Modpacks, Microsoft authentication, backups, and release packaging remain planned. Some very old Minecraft metadata formats may need additional support. See the roadmap for the remaining work.

See [CHANGELOG.md](CHANGELOG.md), [ARCHITECTURE.md](ARCHITECTURE.md), and [PRIVACY.md](PRIVACY.md) for more detail.

## License

OpnLauncher is licensed under the GNU General Public License, version 3 or (at your option) any later version. See [LICENSE](LICENSE).
