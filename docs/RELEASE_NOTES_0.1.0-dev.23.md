# OpnLauncher 0.1.0-dev.23

This is an unreleased development build. The experimental Fabric-to-Horizon bridge remains opt-in and narrowly mapped.

## Bridge verification

- `npm run test:fabric-bridge` now builds an unchanged probe mod into a separate JAR, loads it through an isolated class loader, and runs the production Rust IPC source with the Java helper over loopback.
- The test verifies Fabric client entrypoints, `END_CLIENT_TICK`, a synthetic read-only player snapshot, and the action-bar response flowing back through Rust.
- It uses a Fabric Loader stub. It does not replace a test with the real Fabric Loader in a live Horizon game.

The bridge maps only the explicitly listed read-only player getters and one action-bar operation. It does not support general Fabric mods, Mixins, world mutation, rendering, or Sodium.
