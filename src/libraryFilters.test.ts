import { describe, expect, it } from "vitest";
import type { InstalledMod, Instance } from "./types";
import { filterInstalledMods, filterSortInstances } from "./libraryFilters";

function instance(id: string, name: string, engine: Instance["engine"], versionId: string | null, lastPlayedAt: number | null): Instance {
  return {
    id, name, engine, versionId, lastPlayedAt, createdAt: 1, gameDir: `/games/${id}`,
    javaPath: null, horizonFabricEnabled: false, horizonFabricVersionId: null,
    horizonFabricJavaPath: null, memoryMb: 2048,
  };
}

function mod(fileName: string, name: string, enabled: boolean, version: string | null = null): InstalledMod {
  return { fileName, name, enabled, version, modId: null, source: null, size: 100 };
}

describe("filterSortInstances", () => {
  const instances = [
    instance("java-old", "zeta", "minecraft", "1.20.1", 10),
    instance("java-new", "Alpha", "minecraft", "1.21.1", 30),
    instance("horizon", "Horizon test", "horizon", "26.3", null),
  ];

  it("filters by engine and searches names, versions, and folders", () => {
    expect(filterSortInstances(instances, { query: "1.20", filter: "all", engine: "minecraft", sort: "recent", favorites: [] }).map((item) => item.id)).toEqual(["java-old"]);
    expect(filterSortInstances(instances, { query: "horizon", filter: "all", engine: "all", sort: "recent", favorites: [] }).map((item) => item.id)).toEqual(["horizon"]);
  });

  it("keeps favorites first for recent order and sorts versions numerically", () => {
    expect(filterSortInstances(instances, { query: "", filter: "favorites", engine: "all", sort: "recent", favorites: ["java-old"] }).map((item) => item.id)).toEqual(["java-old"]);
    expect(filterSortInstances(instances, { query: "", filter: "all", engine: "minecraft", sort: "version", favorites: [] }).map((item) => item.versionId)).toEqual(["1.21.1", "1.20.1"]);
  });
});

describe("filterInstalledMods", () => {
  const mods = [mod("zeta.jar", "Zeta", true, "1.0"), mod("sodium.jar", "Sodium", true, "0.6"), mod("old-api.jar.disabled", "Fabric API", false, "0.5")];

  it("searches mod metadata and filenames, then sorts by name", () => {
    expect(filterInstalledMods(mods, "sodium.jar", "all").map((item) => item.name)).toEqual(["Sodium"]);
    expect(filterInstalledMods(mods, "", "all").map((item) => item.name)).toEqual(["Fabric API", "Sodium", "Zeta"]);
  });

  it("filters enabled and disabled files", () => {
    expect(filterInstalledMods(mods, "", "enabled").map((item) => item.name)).toEqual(["Sodium", "Zeta"]);
    expect(filterInstalledMods(mods, "", "disabled").map((item) => item.name)).toEqual(["Fabric API"]);
  });
});
