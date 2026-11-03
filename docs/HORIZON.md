# Horizon integration

OpnLauncher includes Horizon's Rust Minecraft 26.3 client and the Rust resource preparer. Horizon is a separate game engine: it has its own instance folders, worlds, settings, local player data, and `.hmod` content mods. Java `.jar` mods and Fabric, Forge, NeoForge, or Quilt do not work with the native Horizon engine. OpnLauncher does not manage Horizon `.hnmod` code mods.

## Included source and licenses

The vendored source is from Horizon commit `fdd290b99b4459cb4803487758ba8015a45aa47f`. The game runtime is built as a separate process and bundled by Tauri; resource preparation calls the included `horizon-prepare` Rust library. You do not need a separate Horizon checkout, client executable, or preparer executable.

Horizon source is MIT licensed. The upstream license and third-party notices remain in `src-tauri/horizon-upstream/LICENSE` and `src-tauri/horizon-upstream/THIRD_PARTY_NOTICES.md`. OpnLauncher remains GPL-3.0-or-later. The vendored source contains no Minecraft jars, assets, prepared resources, user data, or Horizon Git metadata. The launcher downloads the official game files only when you request preparation.

## Requirements

- A local OpnLauncher developer profile. These profiles are unverified and do not sign in to Microsoft.
- JDK 25, including `javac`, when preparing Horizon resources.
- Internet access when official Minecraft files need downloading or repair.
- For Linux development and AppImage builds, the system libraries required by Tauri. ALSA development files are required for Horizon audio; the build stops with an install hint if they are missing. `libudev` development files add optional controller support, but are not required for audio.

## Prepare and launch

1. In **Settings → Java manager**, add or choose a JDK 25 runtime. Horizon's exporter tools need both `java` and `javac`.
2. Create a Horizon instance from **Versions** or **Add an instance**.
3. Open **Horizon** and choose an existing Minecraft 26.3 folder, add a folder, or leave the source unset to download files into OpnLauncher's private cache.
4. Select **Download files & prepare Horizon**. OpnLauncher reuses only files verified against Mojang metadata, fetches missing official files over HTTPS, and calls the bundled Rust preparer. It then copies the pinned sound assets into Horizon's resources and checks that preparation is complete. Sound import copies about 380 MB from the verified local Minecraft asset store. OpnLauncher does not modify the selected Minecraft source folder.
5. Choose the Horizon instance and a local developer profile, then press **Play**.

The client, preparer source inputs, downloaded Minecraft files, prepared resources, and logs are managed separately from normal Java instances. Worlds and local settings remain in each Horizon instance. Preparing or launching Horizon does not change standard Minecraft profiles.

## Offline and privacy behavior

Horizon launches use local, unverified profiles. OpnLauncher does not pass authentication tokens, implement Microsoft sign-in, or verify ownership for these profiles. The Horizon child process receives `HORIZON_TELEMETRY_URL=off`, disabling Horizon telemetry and its play ping. Resource preparation contacts Mojang's official metadata and file hosts only when that feature is requested. No OpnLauncher-operated server is used.

Local profiles are for offline single-player development and appropriately configured local testing. They do not authenticate to online servers or bypass Minecraft ownership checks. Downloaded Minecraft files remain local and are not bundled or redistributed.

## Current limits

Horizon support is experimental and fixed to Minecraft 26.3, matching the vendored upstream source. The maintainer has reported successful local play. This repository review does not independently reproduce that session or establish compatibility across machines. Java mods, Modrinth/CurseForge browsing for Horizon, and `.hnmod` code-mod management are not supported. The Horizon Mods page handles local `.hmod` content files only.
