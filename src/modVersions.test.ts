import { describe, expect, it } from "vitest";
import type { ModVersion } from "./types";
import { latestCompatibleModVersion } from "./modVersions";

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
