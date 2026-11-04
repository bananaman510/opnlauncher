import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { delimiter, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const targetRoot = resolve(projectRoot, "src-tauri/target");
const cacheRoot = resolve(process.env.OPN_FABRIC_BRIDGE_TEST_CACHE ?? resolve(targetRoot, "fabric-real-loader-check"));
const aggregateApiName = "fabric-api-0.165.2+26.3.jar";
const loaderName = "fabric-loader-0.20.1.jar";
const java = process.env.JAVA ?? "java";
const javac = process.env.JAVAC ?? "javac";
const jar = process.env.JAR ?? "jar";
const rustc = process.env.RUSTC ?? "rustc";

const cacheRelativePath = relative(projectRoot, cacheRoot);
if (cacheRelativePath === ".." || cacheRelativePath.startsWith(`..${sep}`) || isAbsolute(cacheRelativePath)) {
  console.error("Keep the Fabric integration-test cache inside this project folder.");
  process.exit(1);
}

function findFile(directory, fileName) {
  if (!existsSync(directory)) return undefined;
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isFile() && entry.name === fileName) return path;
    if (entry.isDirectory()) {
      const found = findFile(path, fileName);
      if (found) return found;
    }
  }
  return undefined;
}

function listJarFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(directory, entry.name);
    return entry.isDirectory()
      ? listJarFiles(path)
      : entry.isFile() && entry.name.endsWith(".jar") ? [path] : [];
  });
}

function listJavaSources(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(directory, entry.name);
    return entry.isDirectory()
      ? listJavaSources(path)
      : entry.isFile() && entry.name.endsWith(".java") ? [path] : [];
  });
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: projectRoot,
    stdio: "inherit",
    ...options,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited with status ${result.status ?? "unknown"}`);
  return result;
}

function toolMajorVersion(output, pattern) {
  const major = output.match(pattern)?.[1];
  return major ? Number.parseInt(major, 10) : undefined;
}

const apiArchive = findFile(cacheRoot, aggregateApiName);
const loaderJar = findFile(cacheRoot, loaderName);
const libraries = resolve(cacheRoot, "libraries");

if (!apiArchive || !loaderJar || !existsSync(libraries)) {
  console.error(
    `The real-Loader integration check needs local official Minecraft 26.3 test artifacts under ${cacheRoot}: `
      + `${aggregateApiName}, ${loaderName}, and a libraries/ folder. Set OPN_FABRIC_BRIDGE_TEST_CACHE to another project-local cache directory if needed.`,
  );
  process.exit(1);
}

mkdirSync(targetRoot, { recursive: true });
const testRoot = mkdtempSync(resolve(targetRoot, "fabric-real-loader-integration-"));
const extractedApi = resolve(testRoot, "api");
const bridgeClasses = resolve(testRoot, "bridge-classes");
const helperJar = resolve(testRoot, "horizon-fabric-bridge.jar");
const classesDir = resolve(testRoot, "probe-classes");
const probeJar = resolve(testRoot, "opnlauncher-fabric-bridge-probe.jar");
const rustHarness = resolve(testRoot, process.platform === "win32" ? "fabric-bridge-real-loader.exe" : "fabric-bridge-real-loader");
const runRoot = resolve(testRoot, "run");
mkdirSync(extractedApi, { recursive: true });
mkdirSync(bridgeClasses, { recursive: true });
mkdirSync(classesDir, { recursive: true });
mkdirSync(runRoot, { recursive: true });

try {
  const version = run(java, ["-version"], { stdio: ["ignore", "pipe", "pipe"], encoding: "utf8" });
  const javaVersion = `${version.stdout ?? ""}\n${version.stderr ?? ""}`;
  const javaMajor = toolMajorVersion(javaVersion, /version\s+"?(\d+)/);
  const compiler = run(javac, ["-version"], { stdio: ["ignore", "pipe", "pipe"], encoding: "utf8" });
  const compilerVersion = `${compiler.stdout ?? ""}\n${compiler.stderr ?? ""}`;
  const compilerMajor = toolMajorVersion(compilerVersion, /javac\s+(\d+)/);
  if (!javaMajor || javaMajor < 25 || !compilerMajor || compilerMajor < 25) {
    console.error("The real Fabric Loader test requires both Java and javac 25 or newer. Set JAVA and JAVAC to that JDK.");
    throw new Error("Java 25 or newer is required for the real Fabric Loader test.");
  }

  // Build the helper JAR from the current bridge sources. Reusing the last
  // packaged JAR can silently test stale facade signatures after source edits.
  const bridgeSource = resolve(projectRoot, "src-tauri/fabric-bridge/java");
  const bridgeResources = resolve(projectRoot, "src-tauri/fabric-bridge/resources");
  run(javac, ["--release", "21", "-d", bridgeClasses, ...listJavaSources(bridgeSource)]);
  run(jar, [
    "--create", "--file", helperJar,
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
  ]);

  run(jar, ["-xf", apiArchive], { cwd: extractedApi });
  const moduleJars = listJarFiles(resolve(extractedApi, "META-INF/jars")).sort();
  const requiredModules = ["fabric-api-base-", "fabric-key-mapping-api-v1-", "fabric-lifecycle-events-v1-"];
  for (const prefix of requiredModules) {
    if (!moduleJars.some((path) => path.split(/[\\/]/).at(-1)?.startsWith(prefix))) {
      console.error(`The published Fabric API archive is missing its ${prefix.slice(0, -1)} module.`);
      throw new Error(`Published Fabric API module ${prefix} is required for the real Loader check.`);
    }
  }

  const probeSource = resolve(projectRoot, "src-tauri/fabric-bridge/test-mod/java/org/opnlauncher/horizonfabricprobe/FabricBridgeProbeMod.java");
  const metadataDir = resolve(projectRoot, "src-tauri/fabric-bridge/test-mod/resources");
  const apiClasspath = [loaderJar, ...moduleJars, helperJar].join(delimiter);
  run(javac, ["--release", "25", "-classpath", apiClasspath, "-d", classesDir, probeSource]);
  run(jar, [
    "--create", "--file", probeJar,
    "-C", classesDir, "org/opnlauncher/horizonfabricprobe",
    "-C", metadataDir, "fabric.mod.json",
  ]);

  const harnessSource = resolve(projectRoot, "src-tauri/fabric-bridge/test-rust/bridge_loopback.rs");
  run(rustc, ["--edition=2024", harnessSource, "-o", rustHarness]);
  run(rustHarness, [java, helperJar, libraries, probeJar, runRoot], { timeout: 90000 });
} finally {
  rmSync(testRoot, { recursive: true, force: true });
}
