import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const targetDir = resolve(projectRoot, "src-tauri/target");
const helperJar = resolve(projectRoot, "src-tauri/binaries/horizon-fabric-bridge.jar");
const probeSource = resolve(projectRoot, "src-tauri/fabric-bridge/test-mod/java/org/opnlauncher/horizonfabricprobe/FabricBridgeProbeMod.java");
const metadata = resolve(projectRoot, "src-tauri/fabric-bridge/test-mod/resources/fabric.mod.json");
const loaderApiStub = resolve(projectRoot, "src-tauri/fabric-bridge/test/java/net/fabricmc/api/ClientModInitializer.java");
const outputJar = resolve(targetDir, "horizon-fabric-bridge-probe.jar");

if (!existsSync(helperJar) || !existsSync(metadata)) {
  console.error("Build OpnLauncher first and verify the bridge helper and probe metadata are present.");
  process.exit(1);
}

const buildDir = mkdtempSync(resolve(targetDir, "fabric-bridge-probe-"));
const classesDir = resolve(buildDir, "classes");
const temporaryJar = resolve(buildDir, "horizon-fabric-bridge-probe.jar");
mkdirSync(classesDir, { recursive: true });
mkdirSync(targetDir, { recursive: true });

const compile = spawnSync(process.env.JAVAC ?? "javac", [
  "--release", "21",
  "-classpath", helperJar,
  "-d", classesDir,
  loaderApiStub,
  probeSource,
], { cwd: projectRoot, stdio: "inherit" });
if (compile.error) throw compile.error;
if (compile.status !== 0) process.exit(compile.status ?? 1);

const archive = spawnSync(process.env.JAR ?? "jar", [
  "--create", "--file", temporaryJar,
  "-C", classesDir, "org/opnlauncher/horizonfabricprobe",
  "-C", resolve(projectRoot, "src-tauri/fabric-bridge/test-mod/resources"), "fabric.mod.json",
], { cwd: projectRoot, stdio: "inherit" });
if (archive.error) throw archive.error;
if (archive.status !== 0) process.exit(archive.status ?? 1);

copyFileSync(temporaryJar, outputJar);
console.log(`Built test-only Fabric bridge probe at ${outputJar}`);
console.log("Copy this JAR into a Horizon profile's mods folder to test the mapped player snapshot and action-bar calls.");
