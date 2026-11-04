# OpnLauncher 0.1.0-dev.34 development preview

This is an unreleased development build, not a stable release. Horizon's optional Fabric code bridge remains an experimental, narrowly mapped API layer. It does not promise general compatibility with Fabric gameplay mods.

## Horizon Fabric bridge

- Added read-only `Minecraft.options` movement mappings for forward, back, left, right, jump, sneak, and sprint.
- `KeyMapping.isDown()` reports Horizon's mapped vanilla movement state; `consumeClick()` reports a press edge. These values are available during active play and follow the movement actions Horizon currently tracks.
- Extended the bounded Rust-to-Java tick message, class-reference allowlist, unchanged development probe, and loopback harness. The probe uses forward and sneak state to request sprint, and checks the forward press edge.
- Kept the bridge opt-in and separate from the normal Rust launch path. Existing mod JARs are copied unchanged into the bridge staging folder.

## Current limits

- This adds API mappings; it does not translate arbitrary Java bytecode to Rust. Fabric Mixins, Sodium, rendering, world/block APIs, most Fabric APIs, and unsupported Minecraft calls remain rejected or unmapped.
- The Java `Options` facade is read-only and does not implement options-screen editing, arbitrary key conflicts, mouse/controller mappings, or mod-specific input contexts.
- Automated checks use a development probe and synthetic Horizon state. No AppImage, live Horizon game session, or third-party Fabric mod was tested for this development build.

## Validation

- `npm run build` passed.
- `npm test` passed: 10 frontend tests.
- `cargo test --locked --no-default-features --lib --offline --manifest-path src-tauri/Cargo.toml` passed: 70 launcher Rust tests.
- `npm run test:fabric-bridge` passed: Java facade contract, six bridge protocol tests, and Rust/Java loopback with the unchanged probe JAR.
- `npm run horizon:build:dev` passed; the development Horizon runtime compiled.
- `npm run tauri -- build --no-bundle` passed; the optimized Horizon runtime and Linux desktop executable compiled.
- Rust formatting could not be checked because `cargo fmt` is not installed in this environment.
