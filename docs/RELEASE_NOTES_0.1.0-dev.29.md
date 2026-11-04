# OpnLauncher 0.1.0-dev.29 development preview

This is an unreleased development build. It is not a stable release, and the experimental Horizon Fabric bridge does not provide general Fabric mod compatibility.

## Horizon Fabric bridge

- Added explicit mappings for Fabric callbacks that read `Entity.getYRot()` and `Entity.getXRot()`. They return the local player's yaw and pitch from the Rust game snapshot.
- Extended protocol validation, class scanning, the unchanged probe mod, and the local bridge harness for these values.
- Rotation setters and other unmapped APIs remain unsupported. The Java code executes in a helper JVM; this feature does not translate arbitrary bytecode into Rust.

## Validation scope

The frontend production build, all 10 frontend tests, all 70 launcher Rust library tests, the Java facade contract, and the Rust-to-Java loopback harness passed. Both development and optimized Horizon runtimes built, and `npm run tauri -- build --no-bundle` built the Linux desktop executable. No AppImage or live game session was tested. The bridge checks use a probe and synthetic game snapshots; they do not prove a third-party mod works in a live Horizon game.
