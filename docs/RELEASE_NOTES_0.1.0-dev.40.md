# OpnLauncher 0.1.0-dev.40 development preview

This preview adds a regression-tested path for a common Fabric auto-sprint mod pattern.

## Changes

- The unchanged project-owned probe calls `KeyMapping.set` with the sprint binding's default key and calls `setDown(true)` during `END_CLIENT_TICK`.
- The Rust bridge test confirms that these calls become a native Horizon sprint action.
- The class-reference scanner fixture now covers the end-tick event, local-player/options fields, and sprint-key methods used by this pattern.

## Validation

All 74 Rust library tests, the Java facade contract, six bridge protocol tests, Rust/Java loopback, and the real Fabric Loader 0.20.1 integration check with published Fabric API 0.165.2+26.3 passed. The integration check uses synthetic Horizon state and a project-owned probe. A locally cached third-party mod with the same API references was inspected statically but was not executed. No live Horizon session was tested.

## Scope

This validates one narrow API sequence. It does not add general Fabric compatibility, support Mixin-based mods, or make Sodium usable in Rust Horizon. The bridge remains optional and disabled by default.
