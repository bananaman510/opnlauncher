# OpnLauncher 0.1.0-dev.41 development preview

This preview tightens detection of unsupported renderer APIs in the experimental Horizon Fabric bridge.

## Changes

- JARs referencing unmapped `com.mojang.blaze3d` rendering APIs are now rejected by the code bridge allowlist. The supported keyboard `InputConstants` types remain accepted.
- The launch status now accurately summarizes the player's input/state snapshots and the bounded gameplay, chat, and HUD actions mapped by the bridge.

## Validation

All 74 Rust library tests, 10 frontend tests, the frontend production build, and the bridge checks with the project-owned probe passed. The Horizon runtime and desktop executable compiled. Tauri's AppImage command failed at `linuxdeploy`, so the AppDir was packaged using an existing project-local Type 2 runtime. The resulting 144.01 MiB AppImage was extracted to verify it contains the desktop executable, Horizon runtime, and Fabric bridge JAR. The GUI and a live Horizon session were not opened, and no third-party JAR was executed.

## Scope

This is still a narrow experimental API bridge. It does not load Mixins or renderer integrations such as Sodium, and it does not provide general Fabric compatibility. The bridge remains optional and disabled by default.
