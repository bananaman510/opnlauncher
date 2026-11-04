# OpnLauncher 0.1.0-dev.17 — development preview

This is an unstable development build, shared for testing and feedback. It is not a stable release, and features or local data formats may change.

## Horizon Fabric resource bridge

Horizon profiles now include an optional Fabric tab in the mod workspace. The existing Modrinth and CurseForge browser can find and install provider builds tagged for Minecraft 26.3 and Fabric. The feature is off by default and is configured separately for each Horizon profile.

When enabled, OpnLauncher inspects installed Fabric JARs immediately before launching Horizon. It adapts only standalone JARs with validated, code-free `assets/` and `data/` content into Horizon `.hmod` packages. Those packages are written into the selected profile's `horizon-mods/` folder. Files are checked against Horizon's own manifest and content validators, and conflicting namespaces are skipped.

This is an experimental resource adapter, not Fabric support in the usual sense. Horizon does not load Fabric Loader or execute Java bytecode. JARs containing classes, entrypoints, mixins, access wideners, nested JARs, unsupported paths, or additional Fabric dependencies are skipped. Most Fabric gameplay, library, and API mods will not work through it. Some otherwise code-free resources may still be incompatible with Horizon's supported content formats.

Turning the bridge off disables packages it manages but keeps downloaded JARs and generated `.hmod` files. Removing a source JAR disables its generated package on the next Horizon launch. Other Java instances and their normal Fabric/Forge mod workflows are unchanged.

## Validation status

The frontend production build, all 62 Rust library tests, all 10 frontend tests, the Tauri desktop-feature check, and the optimized Linux desktop executable build passed. AppImage bundling did not complete because Tauri's `linuxdeploy` step failed. The bridge has not yet been exercised with real provider JARs in a live Horizon session, so the candidate labels and conversion remain experimental.
