import type { ModVersion } from "./types";

const releasePriority: Record<string, number> = {
  release: 3,
  beta: 2,
  alpha: 1,
};

function publishedAt(version: ModVersion): number {
  const timestamp = Date.parse(version.datePublished);
  return Number.isNaN(timestamp) ? 0 : timestamp;
}

export function latestCompatibleModVersion(
  versions: readonly ModVersion[],
  gameVersion: string,
  loader: string,
): ModVersion | null {
  const compatible = versions.filter((version) =>
    version.gameVersions.includes(gameVersion) && version.loaders.includes(loader),
  );

  compatible.sort((left, right) =>
    (releasePriority[right.releaseType] ?? 0) - (releasePriority[left.releaseType] ?? 0)
    || publishedAt(right) - publishedAt(left),
  );

  return compatible[0] ?? null;
}
