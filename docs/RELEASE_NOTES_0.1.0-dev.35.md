# OpnLauncher 0.1.0-dev.35 development preview

This is an unreleased development build, not a stable or production release. Horizon's optional Fabric code bridge remains experimental and supports only explicitly mapped API calls.

## Horizon Fabric bridge

- Added the Minecraft 26.3 `Identifier.fromNamespaceAndPath` facade and the `KeyMapping.Category` record and `register(Identifier)` behavior.
- Unchanged Fabric code can now create a namespaced category, register a key in that category, inspect its identifier, and receive its press events through the existing mapped Horizon tick and keyboard bridge.
- The probe checks namespace/path parsing, duplicate category rejection, and the custom-category key binding without modifying the probe JAR after compilation.
- Existing native Rust Horizon launches, Java + Fabric profiles, and profile-local mod files are unchanged.

## Current limits

- This is an explicit Java API-to-Horizon mapping executed in a helper JVM. It does not translate arbitrary Java bytecode into Rust or provide the full Minecraft client to mods.
- Custom key categories are not shown in a Horizon controls screen and cannot be rebound. Mouse/controller mappings, most game APIs, Mixins, rendering, Sodium, and arbitrary Fabric APIs remain unsupported.
- The verification uses a probe mod and synthetic Horizon state. It is not a test of a third-party mod in a live Horizon session, and no AppImage was built for this change.

## Validation

- `npm run test:fabric-bridge` passed: Java facade contract, six Rust protocol tests, and Rust/Java loopback.
- `npm test` passed: 10 frontend tests. `npm run build` passed.
- `cargo test --locked --no-default-features --lib --offline --manifest-path src-tauri/Cargo.toml` passed: 70 Rust tests.
- `npm run tauri -- build --no-bundle` compiled the optimized Horizon runtime and Linux desktop executable.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` could not run because the `cargo-fmt` subcommand is not installed.
