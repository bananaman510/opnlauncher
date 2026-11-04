# OpnLauncher 0.1.0-dev.36 development preview

This is an unreleased development build, not a stable or production release. Horizon's optional Fabric code bridge remains experimental and opt-in.

## Horizon Fabric bridge

- Fixed bridge packaging so the helper JAR includes and verifies both mapped Minecraft `Vec3` and `Identifier` classes.
- Added `npm run test:fabric-bridge:real-loader`, which compiles the unchanged bridge probe against published Fabric API 26.3 modules, starts official Fabric Loader 0.20.1, and verifies the callback, keyboard, HUD, chat, and bounded player-action path through the Rust bridge.
- The real-Loader check uses the official artifact cache in the project's ignored `src-tauri/target/fabric-real-loader-check` directory and removes its temporary test output when finished.

## What this demonstrates

Fabric Loader can start the helper and load this probe against the bridge's explicit shims. Fabric tick events, supported snapshots, key state, simple text output, and bounded local-player actions crossed the Rust/Java bridge in the integration harness. The probe JAR itself was unchanged after being compiled against the published Fabric API.

The harness provides synthetic game snapshots and input. It does not start a live Horizon session, verify an existing third-party mod, or show that Sodium or arbitrary Fabric mods work. The bridge is still a narrow API mapping, not a general bytecode-to-Rust translator. It is not a sandbox; only enable it for mods you trust.

## Validation

- `npm run build` passed.
- `npm run test` passed: 10 frontend tests.
- `npm run test:fabric-bridge` passed: Java facade contract, six Rust protocol tests, and the stub-loader Rust/Java loopback.
- `npm run test:fabric-bridge:real-loader` passed with Fabric Loader 0.20.1, Fabric API 0.165.2+26.3 modules, Java 25, and the production helper JAR.
- `cargo test --locked --no-default-features --lib --offline --manifest-path src-tauri/Cargo.toml` passed: 70 tests.
- `npm run tauri -- build --no-bundle` passed, including the optimized Horizon runtime and Linux desktop executable.
- Rust formatting was not checked because `cargo-fmt` is unavailable in this environment.
- No AppImage or live Horizon game session was produced or tested for this change.
