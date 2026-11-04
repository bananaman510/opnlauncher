# OpnLauncher 0.1.0-dev.24

This is an unreleased development build. The experimental Fabric-to-Horizon code bridge remains opt-in and limited to explicitly mapped calls.

## Fabric Loader compatibility check

- Fixed the Horizon Fabric game provider to initialize and retain Fabric Loader's game transformer.
- Started official Fabric Loader 0.20.1 on Java 25.0.4.1 with the production bridge helper.
- Compiled the unchanged test probe against the published Fabric API 0.165.2+26.3 lifecycle and base modules, then loaded that JAR through Fabric Loader.
- Verified a Rust-to-Java `END_CLIENT_TICK` callback using a synthetic local-player snapshot and the mapped action-bar response back through Rust.

This check verifies Loader startup and the event/API bridge contract. It does not run inside the live Horizon game, and it does not establish compatibility with third-party mods, Mixins, Sodium, rendering, world mutation, or arbitrary Fabric APIs.
