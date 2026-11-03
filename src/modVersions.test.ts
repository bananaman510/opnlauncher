import { describe, expect, it } from "vitest";
import type { InstalledMod, ModProject, ModVersion } from "./types";
import { filterAndSortModVersions, latestCompatibleModVersion, matchingInstalledMod } from "./modVersions";

function version(
  id: string,
  options: Partial<ModVersion> = {},
): ModVersion {
  return {
    provider: "modrinth",
    id,
    projectId: "test-project",
    name: id,
    versionNumber: id,
    releaseType: "release",
    datePublished: "2026-01-01T00:00:00Z",
    gameVersions: ["1.21.1"],
    loaders: ["fabric"],
    files: [],
    dependencies: [],
    changelog: null,
    ...options,
  };
}

describe("latestCompatibleModVersion", () => {
  it("selects the newest stable build for the exact game version and loader", () => {
    const result = latestCompatibleModVersion([
      version("wrong-game", { gameVersions: ["1.20.1"] }),
      version("wrong-loader", { loaders: ["forge"] }),
      version("older-release", { datePublished: "2025-12-01T00:00:00Z" }),
      version("newer-beta", { releaseType: "beta", datePublished: "2026-02-01T00:00:00Z" }),
      version("newer-release", { datePublished: "2026-01-15T00:00:00Z" }),
    ], "1.21.1", "fabric");

    expect(result?.id).toBe("newer-release");
  });

  it("falls back to the newest compatible beta when no stable build exists", () => {
    const result = latestCompatibleModVersion([
      version("older-beta", { releaseType: "beta", datePublished: "2026-01-01T00:00:00Z" }),
      version("alpha", { releaseType: "alpha", datePublished: "2026-02-01T00:00:00Z" }),
      version("newer-beta", { releaseType: "beta", datePublished: "2026-01-15T00:00:00Z" }),
    ], "1.21.1", "fabric");

    expect(result?.id).toBe("newer-beta");
  });

  it("returns no version when the project has no exact compatibility match", () => {
    expect(latestCompatibleModVersion([
      version("wrong-game", { gameVersions: ["1.20.1"] }),
    ], "1.21.1", "fabric")).toBeNull();
  });
});

describe("matchingInstalledMod", () => {
  const project: ModProject = {
    provider: "modrinth",
    id: "opaque-project-id",
    title: "Sodium",
    author: "Author",
    summary: "A rendering optimization mod.",
    iconUrl: null,
    downloads: 1,
    categories: ["optimization"],
    gameVersions: ["1.21.1"],
    loaders: ["fabric"],
    projectUrl: "https://modrinth.com/mod/sodium",
    gallery: [],
  };

  it("recognizes a previously installed project by its tracked provider identity", () => {
    const installed: InstalledMod = {
      fileName: "sodium-old.jar",
      name: "Unknown module",
      version: null,
      modId: null,
      source: { fileName: "sodium-old.jar", provider: "modrinth", projectId: "opaque-project-id", versionId: "old" },
      enabled: true,
      size: 10,
    };

    expect(matchingInstalledMod(project, [installed])).toBe(installed);
  });

  it("recognizes common loader-suffixed filenames without matching similarly named projects", () => {
    const similar: InstalledMod = { fileName: "sodium-extra-fabric.jar", name: "Unknown module", version: null, modId: null, source: null, enabled: true, size: 10 };
    const same: InstalledMod = { fileName: "sodium-fabric-0.9.2+mc1.21.jar", name: "Unknown module", version: null, modId: null, source: null, enabled: true, size: 10 };
    const versioned: InstalledMod = { fileName: "sodium-1.21.1.jar", name: "Unknown module", version: null, modId: null, source: null, enabled: true, size: 10 };

    expect(matchingInstalledMod(project, [similar, same])).toBe(same);
    expect(matchingInstalledMod(project, [versioned])).toBe(versioned);
  });
});

describe("filterAndSortModVersions", () => {
  it("filters by Minecraft version, loader, release type, and search text", () => {
    const versions = [
      version("sodium-fabric", { name: "Sodium Fabric", releaseType: "release" }),
      version("sodium-forge", { name: "Sodium Forge", loaders: ["forge"], releaseType: "release" }),
      version("sodium-beta", { name: "Sodium Preview", releaseType: "beta" }),
      version("other", { name: "Other Mod" }),
    ];

    expect(filterAndSortModVersions(versions, {
      query: "sodium",
      gameVersion: "1.21.1",
      loader: "fabric",
      releaseType: "release",
      sort: "newest",
    }).map((entry) => entry.id)).toEqual(["sodium-fabric"]);
  });

  it("sorts version labels numerically in either direction", () => {
    const versions = [version("v10", { versionNumber: "2.10.0" }), version("v2", { versionNumber: "2.2.0" })];
    const common = { query: "", gameVersion: "", loader: "", releaseType: "all" as const };

    expect(filterAndSortModVersions(versions, { ...common, sort: "version-az" }).map((entry) => entry.id)).toEqual(["v2", "v10"]);
    expect(filterAndSortModVersions(versions, { ...common, sort: "version-za" }).map((entry) => entry.id)).toEqual(["v10", "v2"]);
  });
});
