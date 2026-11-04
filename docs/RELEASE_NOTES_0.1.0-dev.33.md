# OpnLauncher 0.1.0-dev.33 development preview

This is an unreleased development build, not a stable release. Horizon's experimental Fabric code bridge remains a limited, opt-in API mapping.

## Horizon Fabric bridge

- Mapped `Entity.setYRot(float)` and `Entity.setXRot(float)` to local-player look updates in Horizon.
- The unchanged development probe's **P** key now queues a 90-degree yaw change and a 15-degree pitch change alongside its existing sprint, velocity, chat, and HUD actions.
- Java validates finite angles; Rust independently checks the IPC action and applies it to Horizon's local camera/player on the game thread during active play. Yaw is limited to ±360,000 degrees and pitch to −90 through 90 degrees.
- Fixed the packaged helper JAR to include the `Vec3` facade required by the probe and mapped player-velocity APIs.
- Updated the static JAR allowlist and regression checks. Other entity rotations, server-authoritative changes, arbitrary Fabric APIs, Mixins, and Sodium remain unsupported.

## Validation

- `npm run build` passed.
- `npm test` passed: 10 frontend tests.
- `cargo test --locked --no-default-features --lib --manifest-path src-tauri/Cargo.toml --offline` passed: 70 launcher Rust tests.
- `npm run test:fabric-bridge` passed: the unchanged probe facade contract, five Rust protocol tests, and Rust/Java loopback, including valid rotation writes and invalid-angle rejection.
- `npm run fabric:build:probe` passed and produced the test-only probe JAR. The packaged helper JAR now includes `Vec3.class`, and runtime builds verify it is present.
- `npm run horizon:build:dev` and `npm run tauri -- build --no-bundle` passed; debug and optimized Horizon runtimes and the optimized Linux desktop executable compiled.
- Rust formatting could not be checked because this environment does not have the `cargo fmt` component installed.
- No AppImage, live Horizon session, or third-party Fabric mod was tested.
- Synthetic probe and loopback checks do not establish live Horizon or third-party mod compatibility.
