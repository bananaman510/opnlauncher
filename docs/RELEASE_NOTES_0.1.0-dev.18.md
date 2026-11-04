# OpnLauncher 0.1.0-dev.18

This is an unreleased development build. It adds an optional way to use actual Fabric JAR mods with a Horizon profile while preserving the native Rust Horizon option.

## Experimental Java + Fabric runtime

- Horizon settings can install Minecraft 26.3 and a selected Fabric Loader version into that profile's private `.opnlauncher/fabric-java` directory.
- Existing mod JARs in the Horizon profile are copied into the private runtime if the destination name is free; the originals are left in place.
- Turn on **Use Java + Fabric for this profile** to launch the Java Minecraft client for that profile. The existing local unverified account is used; Microsoft authentication is not added.
- The normal Mods page then targets the isolated Java runtime, so compatible JARs such as Fabric API and Sodium are installed unchanged and executed by Minecraft Java.
- Turn the option off to play the native Rust Horizon client again. The Java runtime and its mod files are preserved.

This is a Java backend compatibility path, not a Rust bytecode translator or Fabric implementation inside Horizon. It is experimental and has not yet been validated in a live AppImage/game session. Java 25 or newer is required for Minecraft 26.3.
