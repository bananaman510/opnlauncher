import type { InstalledMod, ModProject, ModVersion } from "./types";

const releasePriority: Record<string, number> = {
  release: 3,
  beta: 2,
  alpha: 1,
};

export type ModVersionSort = "newest" | "oldest" | "version-az" | "version-za";
export type ModReleaseFilter = "all" | "release" | "beta" | "alpha";

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

export function latestCompatibleModUpdate(
  versions: readonly ModVersion[],
  installedVersionId: string | null | undefined,
  gameVersion: string,
  loader: string,
): ModVersion | null {
  if (!installedVersionId) return null;
  const latest = latestCompatibleModVersion(versions, gameVersion, loader);
  return latest && latest.id !== installedVersionId ? latest : null;
}

function normalizedIdentity(value: string): string {
  return value.toLocaleLowerCase().replace(/[^a-z0-9]/g, "");
}

export function matchingInstalledMod(
  project: ModProject,
  installedMods: readonly InstalledMod[],
): InstalledMod | null {
  const projectSlug = project.projectUrl?.split("/").filter(Boolean).at(-1) ?? "";
  const normalizedTitle = normalizedIdentity(project.title);
  const normalizedSlug = normalizedIdentity(projectSlug);
  const filenameSlug = projectSlug.toLocaleLowerCase();

  return installedMods.find((mod) => {
    if (mod.source?.provider === project.provider && mod.source.projectId === project.id) return true;
    if (normalizedTitle && normalizedIdentity(mod.name) === normalizedTitle) return true;
    if (project.provider === "modrinth" && mod.modId && normalizedSlug && normalizedIdentity(mod.modId) === normalizedSlug) return true;

    if (!filenameSlug) return false;
    const stem = mod.fileName.toLocaleLowerCase()
      .replace(/\.disabled$/i, "")
      .replace(/\.jar$/i, "");
    if (stem === filenameSlug) return true;
    if (!stem.startsWith(`${filenameSlug}-`)) return false;

    // Common provider filenames append the loader or Minecraft version to the project slug.
    const suffix = stem.slice(filenameSlug.length + 1);
    return /^(?:(?:fabric|forge|neoforge|quilt)(?:-|$)|mc[0-9]|minecraft-?[0-9]|[0-9])/i.test(suffix);
  }) ?? null;
}

export function filterAndSortModVersions(
  versions: readonly ModVersion[],
  options: {
    query: string;
    gameVersion: string;
    loader: string;
    releaseType: ModReleaseFilter;
    sort: ModVersionSort;
  },
): ModVersion[] {
  const query = options.query.trim().toLocaleLowerCase();
  return versions.filter((version) =>
    (!options.gameVersion || version.gameVersions.includes(options.gameVersion))
    && (!options.loader || version.loaders.includes(options.loader))
    && (options.releaseType === "all" || version.releaseType === options.releaseType)
    && (!query || `${version.name} ${version.versionNumber} ${version.id}`.toLocaleLowerCase().includes(query)),
  ).sort((left, right) => {
    if (options.sort === "version-az") return left.versionNumber.localeCompare(right.versionNumber, undefined, { numeric: true });
    if (options.sort === "version-za") return right.versionNumber.localeCompare(left.versionNumber, undefined, { numeric: true });
    const leftDate = publishedAt(left);
    const rightDate = publishedAt(right);
    return options.sort === "oldest" ? leftDate - rightDate : rightDate - leftDate;
  });
}
