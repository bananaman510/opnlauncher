# OpnLauncher 0.1.0-dev.31 development preview

This is an unreleased development build. It is not a stable release, and the experimental Horizon Fabric bridge does not provide general Fabric mod compatibility.

## Horizon Fabric bridge

- Added `Entity.setDeltaMovement(Vec3)` as a bounded, explicit Rust game-thread action for the active Horizon player.
- Added the mapped `Vec3(double, double, double)` constructor so unchanged mods can form velocity requests using the Minecraft API shape.
- Velocity requests must be finite and have a vector magnitude at most 4 blocks per tick; malformed or out-of-range IPC is rejected, and requests outside active Horizon play are ignored.
- Extended the unchanged probe mod, static class-file allowlist, protocol tests, and synthetic Rust/Java loopback coverage. The optional bridge remains off by default.
- The bridge still executes Java callbacks in a helper JVM and maps only listed calls. It does not translate arbitrary Java bytecode or provide general Fabric/Sodium compatibility.

## Validation scope

The frontend production build, all 10 frontend tests, and all 70 launcher Rust library tests passed. The Java facade contract, five Rust bridge protocol unit tests, and Rust/Java loopback passed, including a velocity request returned to Rust and checks rejecting non-finite and over-limit vectors. Both debug and optimized Horizon runtimes compiled, and Tauri built the `.31` release executable. AppImage packaging reached `appimagetool` but stopped because it could not download the Type 2 runtime while network access was unavailable. No AppImage, live Horizon session, or third-party mod was tested.
