# OpnLauncher 0.1.0-dev.37 development preview

This is an unreleased development build, not a stable or production release. The Horizon Fabric code bridge remains experimental and opt-in.

## Horizon Fabric bridge

- Added an unchanged-mod call path for `Entity.blockPosition()` with `BlockPos.getX()`, `getY()`, and `getZ()` backed by Horizon's current player position snapshot.
- Block coordinates use floor behavior for negative positions, matching Minecraft (for example, `-3.5` maps to `-4`).
- Expanded the strict class-reference scanner, packaged helper JAR, build verification, and test probe for this API.
- This is a read-only position view. It does not expose world block/chunk data or make arbitrary Fabric gameplay mods compatible.

## Validation

- `npm run build` passed.
- `npm run test` passed: 10 frontend tests.
- `cargo test --locked --no-default-features --lib --offline --manifest-path src-tauri/Cargo.toml` passed: 70 tests, including the bridge API allowlist.
- `npm run test:fabric-bridge` passed: Java facade contract, six Rust bridge tests, and Rust/Java localhost loopback with the unchanged probe.
- `npm run horizon:build:release` passed and verified `BlockPos.class` in the packaged helper JAR.
- `npm run tauri -- build --no-bundle` passed and built the optimized Linux desktop executable.
- `npm run test:fabric-bridge:real-loader` could not run: the active Java/Javac toolchain is below the script's Java 25 requirement.
- No AppImage, live Horizon game session, or third-party Fabric mod was tested.
