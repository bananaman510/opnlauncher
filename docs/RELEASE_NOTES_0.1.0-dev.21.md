# OpnLauncher 0.1.0-dev.21

This is an unreleased development build. It adds a real but deliberately small Fabric API mapping for native Rust Horizon.

## Fabric callback and HUD mapping

- Fabric Loader still runs selected, unchanged mod JARs in an optional helper JVM beside Horizon.
- The existing `ClientTickEvents.END_CLIENT_TICK` event now receives a limited Minecraft facade rather than `null`.
- `Minecraft.getInstance().gui.hud.setOverlayMessage(Component.literal(text), animate)` sends plain text through a bounded local response and displays it in Horizon's Rust action bar on Horizon's game thread.
- The Java helper does not load the Minecraft client JAR. Original installed mod files remain unchanged; only accepted copies are staged privately.
- The scanner recognizes only the exact class and member subset needed for this mapping. Unmapped APIs, Mixins, access wideners, nested JARs, and other Fabric API references remain rejected.
- Added a development-only probe mod, built with `npm run fabric:build:probe`, so the action-bar path can be tried in a native Horizon profile without depending on a third-party mod.

## Limits and validation

This is not general Fabric support and it does not translate arbitrary Java bytecode to Rust. It is not enough for Fabric API, Sodium, or ordinary gameplay mods that read or modify the world, player, entities, input, registries, or rendering. The Java contract test uses a test loader stub; a real Fabric Loader installation, an unchanged test mod, visible action-bar output in Horizon, and a packaged AppImage still require desktop validation.

The bridge is opt-in and runs mod code with the user's normal operating-system permissions. It is not a Java sandbox.
