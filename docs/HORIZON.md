# Horizon integration

OpnLauncher treats Horizon as a separate Minecraft 26.3 engine. Horizon instances have their own instance folder, local player file, worlds, settings, and data-only `.hmod` content mods. Java `.jar` mods, Fabric, Forge, NeoForge, and Quilt are not compatible with the native Horizon engine. Native `.hnmod` code mods are not managed by OpnLauncher.

## Local requirements

- A locally built `horizon-client` executable that reports Launch API `0.1.x` and targets Minecraft 26.3.
- A locally built `horizon-prepare` executable compatible with that client.
- JDK 25, including `javac`, for resource preparation.
- A local, unverified OpnLauncher developer profile.

OpnLauncher searches `PATH` for the client and preparer when it opens. You can choose a detected tool or enter its path on the Horizon page. Add a Java 25 executable under Settings → Java Manager, then select it as the preparation JDK.

OpnLauncher also searches the standard Minecraft folder and common PrismLauncher, PolyMC, MultiMC, ATLauncher, and GDLauncher instance folders. You can rescan or add another Minecraft folder from Horizon settings. If a selected installation contains Minecraft 26.3, OpnLauncher copies only files that match Mojang's published SHA-1 and size metadata into its private Horizon cache. It never writes into the selected source folder. Missing or invalid source files are downloaded from Mojang into the private cache; if no source is selected, the complete set is fetched there. The required server file is included. Horizon's preparer then builds the Rust-engine resources in a separate support folder. Repeating preparation verifies and reuses valid cached files and safely replaces the prepared resource directory after a successful run.

The Java and Horizon files are separate installations. Preparing Horizon does not replace or modify a standard Java instance, its mods, saves, or settings.

## Offline and privacy behavior

OpnLauncher passes the selected unverified local profile to Horizon without an authentication token. Microsoft sign-in is not part of this integration. Horizon processes are started with `HORIZON_TELEMETRY_URL=off`, which disables Horizon telemetry and its play ping. Resource preparation contacts Mojang's official metadata and file hosts when files need to be installed or repaired. No OpnLauncher-operated server is involved.

Local profiles do not verify Minecraft ownership or authenticate to online servers. Use them for local single-player development and appropriately configured local testing only. Downloaded game files remain local and are not bundled or redistributed by OpnLauncher.

## Current limits

Horizon support is experimental and currently limited to Minecraft 26.3. The Horizon client and preparer must be built or obtained separately; OpnLauncher currently launches and invokes the configured local tools but does not build or bundle Horizon. The launch API, resource preparation, `.hmod` behavior, and game session have not yet been validated against a live Horizon build. The Horizon Mods page manages local `.hmod` files only; it does not browse Modrinth or CurseForge, and it does not execute `.hnmod` code.
