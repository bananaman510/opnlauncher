# OpnLauncher 0.1.0-dev.38 development preview

This development build extends the optional Horizon Fabric code bridge with programmatic sprint-key mapping.

## Changes

- Map `KeyMapping.set(InputConstants.Key, boolean)` and `KeyMapping.setDown(boolean)` when used on Horizon's native sprint binding. The request travels through the bridge and Horizon applies it on the Rust game thread.
- Extend the strict JAR API allowlist and project-owned probe to cover those exact calls.
- Keep normal Rust Horizon launches and the separate Java + Fabric profile unchanged.

## Validation

The project-owned probe passed through Fabric Loader 0.20.1 and the published Minecraft 26.3 Fabric API modules with synthetic Horizon player state. This confirms Loader startup, method linkage, and a sprint action reaching the Rust bridge. It does not confirm a visible effect in a live Horizon session or compatibility with a third-party gameplay mod. Third-party JARs remain executable code and are not sandboxed. The 144.92 MiB Linux AppImage was packaged with a project-local cached Type 2 runtime because the packaging environment could not download one.

## Scope

This is one additional API mapping, not general Fabric compatibility. Mixins, Sodium, rendering, world access, and unmapped Minecraft/Fabric calls remain unsupported. The bridge is still optional and off by default.
