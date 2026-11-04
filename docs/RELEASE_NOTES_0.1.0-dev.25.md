# OpnLauncher 0.1.0-dev.25

This is an unreleased development build. The optional Fabric-to-Horizon bridge remains experimental and narrowly mapped.

## Client tick phases

- Added `ClientTickEvents.START_CLIENT_TICK` and retained `END_CLIENT_TICK`, queued in order with player snapshots captured at the corresponding native tick boundaries.
- The helper JVM processes callbacks asynchronously so Horizon's game thread does not wait for Java. Delivery can lag or drop callbacks if the bounded queue fills; action-bar requests return to Horizon's game thread.
- Expanded the test probe to show both phases and verify their ordering against official Fabric Loader 0.20.1 and Fabric API 0.165.2+26.3 classes on Java 25.

Level tick events, Mixins, Sodium, rendering, player/world mutation, and general Fabric API compatibility remain unsupported. Live Horizon gameplay has not been verified.
