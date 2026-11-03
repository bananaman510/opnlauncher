# OpnLauncher

OpnLauncher is an independent, local-first launcher for Minecraft Java Edition. It is built with Tauri 2, Rust, React, TypeScript, and SQLite.

This repository is an early **development prototype**. It can browse Mojang's official vanilla version list and install compatible releases, snapshots, or historical versions into a new, separate instance directory. Some very old metadata formats still need support. It also detects and registers existing installations, discovers Java, and stores local instance metadata. Microsoft authentication and mod or modpack installation are not implemented yet.

OpnLauncher is not affiliated with, endorsed by, or associated with Mojang Studios or Microsoft. Minecraft is a trademark of Mojang Synergies AB. Minecraft game files are not bundled or redistributed by this project. When requested, OpnLauncher downloads official vanilla game files from URLs in Mojang's version metadata.

## Current features

- Dark desktop interface with Home, Instances, Versions, Mods, Modpacks, Accounts, Downloads, and Settings sections.
- SQLite storage for local instance metadata and development profiles.
- Detection of common Minecraft directories and installed version metadata.
- Manual inspection of another local Minecraft game directory.
- Search and filter Mojang's official releases, snapshots, and older alpha/beta versions.
- Install supported vanilla versions into a new isolated instance folder with bounded parallel downloads, progress, retry, cancellation, and upstream SHA-1/size checks.
- Java runtime discovery through `JAVA_HOME`, `PATH`, and common Linux locations.
- Per-instance Java selection and memory allocation.
- Visibly unverified local developer profiles with stable offline UUIDs, available in development and release builds.
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

The app stores its SQLite database, managed instance directories and downloaded vanilla game files, launch logs, and generated native-library cache in Tauri's application data directory. Registered existing installations continue to point at their existing game directories; downloads go into a separate new instance directory and do not modify those existing files.

## Development status

The first milestone is still in progress. The initial installer handles official vanilla version metadata, libraries, assets, and client files; resume, repair, Microsoft authentication, mod loaders, mod repositories, backups, and release packaging remain planned. The launch resolver expects a complete local installation and supported Mojang version metadata. This is not feature parity with other launcher yet; see the roadmap for the remaining work.

See [CHANGELOG.md](CHANGELOG.md), [ARCHITECTURE.md](ARCHITECTURE.md), and [PRIVACY.md](PRIVACY.md) for more detail.

## License

OpnLauncher is licensed under the GNU General Public License, version 3 or (at your option) any later version. See [LICENSE](LICENSE).
