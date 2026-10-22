# OpnLauncher

OpnLauncher is an independent, local-first launcher for Minecraft Java Edition. It is built with Tauri 2, Rust, React, TypeScript, and SQLite.

This repository is the initial **0.1.0 development prototype**. It focuses on Java discovery, detecting and registering existing Minecraft installations, local instance metadata, and a first launch path for already installed game files. It does not install Minecraft, authenticate Microsoft accounts, install mods, or download game assets yet.

OpnLauncher is not affiliated with, endorsed by, or associated with Mojang Studios or Microsoft. Minecraft is a trademark of Mojang Synergies AB. Minecraft game files are not included or redistributed by this project. Use the official Minecraft launcher or another legitimate source to obtain game files.

## Current features

- Dark desktop interface with Home, Instances, Versions, Mods, Modpacks, Accounts, Downloads, and Settings sections.
- SQLite storage for local instance metadata and development profiles.
- Detection of common Minecraft directories and installed version metadata.
- Manual inspection of another local Minecraft game directory.
- Java runtime discovery through `JAVA_HOME`, `PATH`, and common Linux locations.
- Per-instance Java selection and memory allocation.
- Development-only, visibly unverified local test profiles with stable offline UUIDs.
- Local launch logs and process exit status.
- No telemetry, analytics, ads, launcher account, or automatic diagnostic upload.

The development profile exists for local single-player testing. It does not authenticate with Microsoft or provide online-server access. This prototype does not bypass ownership checks.

## Linux development setup

You need Rust, Node.js with npm, Java for running Minecraft, and Tauri's Linux development libraries. The command below is for Fedora. For other distributions, follow the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) and install the matching GTK/WebKit development packages.

```sh
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
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

The app stores its SQLite database, managed empty instance directories, launch logs, and generated native-library cache in Tauri's application data directory. Registered existing installations continue to point at their existing game directories. OpnLauncher does not overwrite, remove, or download files in those directories in this milestone.

## Development status

The first milestone is still in progress. Microsoft authentication, installation/download repair, assets and library acquisition, mod loaders, mod repositories, backups, and release packaging are planned for later phases. The launch resolver currently expects a complete local installation and supported Mojang version metadata.

See [CHANGELOG.md](CHANGELOG.md), [ARCHITECTURE.md](ARCHITECTURE.md), and [PRIVACY.md](PRIVACY.md) for more detail.

## License

OpnLauncher is licensed under the GNU General Public License, version 3 or (at your option) any later version. See [LICENSE](LICENSE).
