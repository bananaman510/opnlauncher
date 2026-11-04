import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const buildRoot = resolve(projectRoot, "src-tauri/target");
mkdirSync(buildRoot, { recursive: true });
const testRoot = mkdtempSync(resolve(buildRoot, "fabric-bridge-test-"));
const classesDir = resolve(testRoot, "classes");
const probeClassesDir = resolve(testRoot, "probe-classes");
const probeJar = resolve(testRoot, "unchanged-fabric-bridge-probe.jar");
mkdirSync(classesDir, { recursive: true });
mkdirSync(probeClassesDir, { recursive: true });

function listJavaSources(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(directory, entry.name);
    return entry.isDirectory()
      ? listJavaSources(path)
      : entry.isFile() && entry.name.endsWith(".java") ? [path] : [];
  });
}

try {
  const sourceRoot = resolve(projectRoot, "src-tauri/fabric-bridge/java");
  const testSourceRoot = resolve(projectRoot, "src-tauri/fabric-bridge/test/java");
  const probeSourceRoot = resolve(projectRoot, "src-tauri/fabric-bridge/test-mod/java");
  const probeSource = listJavaSources(probeSourceRoot);
  const compile = spawnSync(process.env.JAVAC ?? "javac", [
    "--release", "21", "-d", classesDir,
    ...listJavaSources(sourceRoot),
    ...listJavaSources(testSourceRoot),
  ], { cwd: projectRoot, stdio: "inherit" });
  if (compile.error) throw compile.error;
  if (compile.status !== 0) process.exitCode = compile.status ?? 1;
  else {
    const compileProbe = spawnSync(process.env.JAVAC ?? "javac", [
      "--release", "21", "-classpath", classesDir, "-d", probeClassesDir,
      ...probeSource,
    ], { cwd: projectRoot, stdio: "inherit" });
    if (compileProbe.error) throw compileProbe.error;
    if (compileProbe.status !== 0) process.exitCode = compileProbe.status ?? 1;
    else {
      const archive = spawnSync(process.env.JAR ?? "jar", [
        "--create", "--file", probeJar,
        "-C", probeClassesDir, "org/opnlauncher/horizonfabricprobe",
        "-C", resolve(projectRoot, "src-tauri/fabric-bridge/test-mod/resources"), "fabric.mod.json",
      ], { cwd: projectRoot, stdio: "inherit" });
      if (archive.error) throw archive.error;
      if (archive.status !== 0) process.exitCode = archive.status ?? 1;
    }
  }
  if (process.exitCode === undefined) {
    const run = spawnSync(process.env.JAVA ?? "java", [
      `-Dopn.fabric.test.probe.jar=${probeJar}`,
      "-cp", classesDir,
      "org.opnlauncher.horizonfabric.HorizonFabricBridgeTest",
    ], { cwd: projectRoot, stdio: "inherit", timeout: 15000 });
    if (run.error) throw run.error;
    if (run.status !== 0) process.exitCode = run.status ?? 1;
  }
  if (process.exitCode === undefined) {
    const bridgeModule = resolve(projectRoot, "src-tauri/horizon-upstream/crates/horizon-client/src/fabric_bridge.rs");
    const bridgeTests = resolve(testRoot, process.platform === "win32" ? "fabric-bridge-unit-tests.exe" : "fabric-bridge-unit-tests");
    const compileBridgeTests = spawnSync(process.env.RUSTC ?? "rustc", [
      "--edition", "2021", "--test", bridgeModule, "-o", bridgeTests,
    ], { cwd: projectRoot, stdio: "inherit" });
    if (compileBridgeTests.error) throw compileBridgeTests.error;
    if (compileBridgeTests.status !== 0) process.exitCode = compileBridgeTests.status ?? 1;
    else {
      const runBridgeTests = spawnSync(bridgeTests, [], { cwd: projectRoot, stdio: "inherit", timeout: 15000 });
      if (runBridgeTests.error) throw runBridgeTests.error;
      if (runBridgeTests.status !== 0) process.exitCode = runBridgeTests.status ?? 1;
    }
  }
  if (process.exitCode === undefined) {
    const rustHarness = resolve(projectRoot, "src-tauri/fabric-bridge/test-rust/bridge_loopback.rs");
    const rustBinary = resolve(testRoot, process.platform === "win32" ? "bridge-loopback.exe" : "bridge-loopback");
    const compileRust = spawnSync(process.env.RUSTC ?? "rustc", [
      "--edition", "2024", rustHarness, "-o", rustBinary,
    ], { cwd: projectRoot, stdio: "inherit" });
    if (compileRust.error) throw compileRust.error;
    if (compileRust.status !== 0) process.exitCode = compileRust.status ?? 1;
    else {
      const runRust = spawnSync(rustBinary, [
        process.env.JAVA ?? "java", classesDir, probeJar, projectRoot,
      ], { cwd: projectRoot, stdio: "inherit", timeout: 20000 });
      if (runRust.error) throw runRust.error;
      if (runRust.status !== 0) process.exitCode = runRust.status ?? 1;
    }
  }
} finally {
  rmSync(testRoot, { recursive: true, force: true });
}
