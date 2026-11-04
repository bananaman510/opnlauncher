import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, chmodSync, readdirSync, rmSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const profile = process.argv[2] === "release" ? "release" : "debug";
const target = process.env.TAURI_ENV_TARGET_TRIPLE;
const targetDir = resolve(projectRoot, "target/horizon-runtime");
const runtimeManifest = resolve(projectRoot, "src-tauri/horizon-runtime/Cargo.toml");
const windowsTarget = target ? target.includes("windows") : process.platform === "win32";
const linuxTarget = target ? target.includes("linux") : process.platform === "linux";
const outputName = windowsTarget ? "horizon-runtime.exe" : "horizon-runtime";
const args = ["build", "--manifest-path", runtimeManifest, "--target-dir", targetDir, "--bin", "horizon-runtime"];

function listJavaSources(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(directory, entry.name);
    return entry.isDirectory() ? listJavaSources(path) : entry.isFile() && entry.name.endsWith(".java") ? [path] : [];
  });
}

const bridgeSource = resolve(projectRoot, "src-tauri/fabric-bridge/java");
const bridgeResources = resolve(projectRoot, "src-tauri/fabric-bridge/resources");
const bridgeTarget = resolve(projectRoot, "src-tauri/target/horizon-fabric-bridge");
const bridgeClasses = resolve(bridgeTarget, "classes");
const bridgeJar = resolve(projectRoot, "src-tauri/binaries/horizon-fabric-bridge.jar");
rmSync(bridgeTarget, { recursive: true, force: true });
mkdirSync(bridgeClasses, { recursive: true });
mkdirSync(dirname(bridgeJar), { recursive: true });
const javacResult = spawnSync(process.env.JAVAC ?? "javac", ["--release", "21", "-d", bridgeClasses, ...listJavaSources(bridgeSource)], {
  cwd: projectRoot,
  stdio: "inherit",
});
if (javacResult.error) throw javacResult.error;
if (javacResult.status !== 0) {
  console.error("Could not compile the optional Fabric-to-Horizon compatibility bridge. Install a JDK 21 or newer and rebuild.");
  process.exit(javacResult.status ?? 1);
}
const jarResult = spawnSync(process.env.JAR ?? "jar", [
  "--create", "--file", bridgeJar,
  "-C", bridgeClasses, "org/opnlauncher/horizonfabric",
  "-C", bridgeClasses, "net/fabricmc/fabric/api",
  "-C", bridgeClasses, "net/fabricmc/fabric/mixin",
  "-C", bridgeClasses, "com/mojang/blaze3d/platform",
  "-C", bridgeClasses, "net/minecraft/client",
  "-C", bridgeClasses, "net/minecraft/core",
  "-C", bridgeClasses, "net/minecraft/resources",
  "-C", bridgeClasses, "net/minecraft/world/entity",
  "-C", bridgeClasses, "net/minecraft/world/level",
  "-C", bridgeClasses, "net/minecraft/world/phys",
  "-C", bridgeClasses, "net/minecraft/network/chat",
  "-C", bridgeResources, "META-INF/services/net.fabricmc.loader.impl.game.GameProvider",
], { cwd: projectRoot, stdio: "inherit" });
if (jarResult.error) throw jarResult.error;
if (jarResult.status !== 0) process.exit(jarResult.status ?? 1);
const verifyJarResult = spawnSync(process.env.JAR ?? "jar", ["--list", "--file", bridgeJar], {
  cwd: projectRoot,
  encoding: "utf8",
  stdio: ["ignore", "pipe", "inherit"],
});
if (verifyJarResult.error) throw verifyJarResult.error;
if (verifyJarResult.status !== 0) process.exit(verifyJarResult.status ?? 1);
const packagedEntries = new Set(verifyJarResult.stdout.split(/\r?\n/));
for (const requiredClass of [
  "net/minecraft/world/phys/Vec3.class",
  "net/minecraft/core/BlockPos.class",
  "net/minecraft/resources/Identifier.class",
  "net/minecraft/world/level/Level.class",
  "net/minecraft/world/level/block/Block.class",
  "net/minecraft/world/level/block/state/BlockState.class",
]) {
  if (!packagedEntries.has(requiredClass)) {
    console.error(`The packaged Fabric bridge is missing its mapped ${requiredClass} facade.`);
    process.exit(1);
  }
}
console.log(`Built experimental Fabric bridge at ${bridgeJar}`);

if (profile === "release") args.push("--release");
if (target) args.push("--target", target);
if (linuxTarget) {
  const hasAlsa = spawnSync("pkg-config", ["--exists", "alsa"], { stdio: "ignore" }).status === 0;
  const hasLibudev = spawnSync("pkg-config", ["--exists", "libudev"], { stdio: "ignore" }).status === 0;
  args.push("--no-default-features");
  if (!hasAlsa) {
    const fedoraHint = process.platform === "linux"
      ? " Fedora: sudo dnf install alsa-lib-devel pkgconf-pkg-config."
      : "";
    console.error(`Cannot build the Linux Horizon runtime without audio support. Install ALSA development files and pkg-config, then rebuild.${fedoraHint}`);
    process.exit(1);
  }
  const features = ["audio"];
  if (hasLibudev) features.push("controllers");
  else console.warn("Horizon controller support is disabled because libudev development files were not found; audio support will still be built.");
  args.push("--features", features.join(","));
}

const result = spawnSync(process.env.CARGO ?? "cargo", args, {
  cwd: projectRoot,
  stdio: "inherit",
  env: { ...process.env, HORIZON_TELEMETRY_URL: "off", CCACHE_DISABLE: "1" },
});
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);

const targetSegment = target ? `${target}/` : "";
const builtRuntime = resolve(targetDir, targetSegment, profile, outputName);
const bundledRuntime = resolve(projectRoot, "src-tauri/binaries/horizon-runtime.exe");
mkdirSync(dirname(bundledRuntime), { recursive: true });
copyFileSync(builtRuntime, bundledRuntime);
if (process.platform !== "win32") chmodSync(bundledRuntime, 0o755);
console.log(`Built Horizon runtime at ${bundledRuntime}`);
