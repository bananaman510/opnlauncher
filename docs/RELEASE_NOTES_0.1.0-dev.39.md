# OpnLauncher 0.1.0-dev.39 development preview

This development build extends the optional Horizon Fabric code bridge with one immutable Minecraft vector operation.

## Changes

- Map `Vec3.add(double, double, double)` so unchanged mod code can derive an adjusted velocity from Horizon's player snapshot.
- Pass the calculated vector to the existing bounded `Entity.setDeltaMovement(Vec3)` Rust action.
- Extend the strict API allowlist, Java facade contract, Rust scanner tests, and unchanged Fabric probe. Mixin metadata and Mixin bytecode are explicitly rejected in tests.

## Validation

The project-owned unchanged probe passed through official Fabric Loader 0.20.1 with published Fabric API 0.165.2+26.3 modules and synthetic Horizon state. Frontend tests/build, 74 Rust tests, the Java facade contract, six bridge protocol tests, Rust/Java loopback, and optimized desktop/runtime builds passed. The `.39` AppImage was packaged with an existing local Type 2 runtime after Tauri could not download one. The AppImage GUI was not opened; no live Horizon session or third-party mod is covered by these checks. See the [feature audit](FEATURE_AUDIT.md) for the detailed validation record.

## Scope

This adds only vector addition. Other unsupported Minecraft/Fabric calls and Mixins remain rejected; this does not provide general Fabric compatibility or Sodium support. The bridge remains optional and off by default.
