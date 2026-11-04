# OpnLauncher 0.1.0-dev.30 development preview

This is an unreleased development build. It is not a stable release, and the experimental Horizon Fabric bridge does not provide general Fabric mod compatibility.

## Horizon Fabric bridge

- Added a read-only `Entity.getDeltaMovement()` mapping backed by Horizon's real local-player velocity snapshot.
- Added an immutable Java `Vec3` facade with explicit `x`, `y`, `z`, length, squared-length, and horizontal-distance operations.
- Extended finite-value checks, class-file allowlisting, the unchanged probe mod, and the Rust/Java loopback harness to cover motion snapshots.
- Unsupported vector calls, writes, Mixins, and other unmapped APIs remain rejected. The bridge runs Java callbacks in a helper JVM; it does not translate arbitrary bytecode into Rust.

## Validation scope

The frontend production build, all 10 frontend tests, all 70 launcher Rust library tests, the Java facade contract, and Rust-to-Java loopback harness passed. The harness checked velocity coordinates, full and horizontal vector lengths (including squared values), callback ordering, keyboard input, sprint requests, HUD responses, and rejection of non-finite velocity. Development and optimized Horizon runtimes built, and `npm run tauri -- build --no-bundle` built the Linux desktop executable. No AppImage or live Horizon session was tested; the velocity mapping has not been verified with a third-party mod.
