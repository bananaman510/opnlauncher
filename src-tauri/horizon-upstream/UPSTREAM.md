# Vendored Horizon source

This directory contains the source needed to build the Horizon Minecraft 26.3 client and prepare its runtime resources. It was copied from the Horizon repository at commit fdd290b99b4459cb4803487758ba8015a45aa47f on 2026-11-03.

The Horizon source is MIT licensed; see LICENSE. Patched upstream dependencies and other third-party notices are documented in THIRD_PARTY_NOTICES.md. The OpnLauncher project remains GPL-3.0-or-later.

Only source files are included. No Minecraft jars, assets, compiled exporter classes, prepared resources, user data, or Horizon Git metadata are included.

The workspace lists only the transitive Rust crates needed by horizon-client and horizon-prepare; third_party/wgpu-hal and third_party/naga are retained because Horizon patches them. mods/horizon/src/test/java/dev/horizonlab/parity contains source code used to prepare the player's own Minecraft files.
