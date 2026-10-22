# Security policy

## Reporting a vulnerability

Please do not include live credentials, private game data, or personal information in a report. Until a dedicated private reporting channel is configured, report issues through the private Forgejo project's maintainer contact. Do not publish exploit details before maintainers have had a reasonable opportunity to investigate and prepare a fix.

## Current security boundaries

- The desktop window does not receive unrestricted shell, filesystem, or process plugin access.
- Rust validates version identifiers and local paths used by the launch resolver.
- Native archives are checked against path traversal and symbolic-link extraction.
- Java and Minecraft processes are started directly without a shell.
- Logs remain local and this prototype has no account tokens to record.
- Local offline profiles are clearly unverified and cannot authenticate to online services.

This is an early development build and has not received a complete security audit. Minecraft mods are third-party executable code; install them only from sources you trust when mod support is introduced.
