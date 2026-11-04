import type { InstalledMod, Instance } from "./types";

export type InstanceFilter = "all" | "favorites";
export type InstanceEngineFilter = "all" | "minecraft" | "horizon";
export type InstanceSort = "recent" | "name" | "version";
export type InstalledModFilter = "all" | "enabled" | "disabled";

export function filterSortInstances(
  instances: Instance[],
  options: {
    query: string;
    filter: InstanceFilter;
    engine: InstanceEngineFilter;
    sort: InstanceSort;
    favorites: string[];
  },
): Instance[] {
  const query = options.query.trim().toLocaleLowerCase();
  const favoriteIds = new Set(options.favorites);
  return [...instances]
    .filter((instance) => options.filter === "all" || favoriteIds.has(instance.id))
    .filter((instance) => options.engine === "all" || instance.engine === options.engine)
    .filter((instance) => !query || `${instance.name} ${instance.versionId ?? ""} ${instance.gameDir}`.toLocaleLowerCase().includes(query))
    .sort((left, right) => {
      if (options.sort === "name") return left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
      if (options.sort === "version") {
        return (right.versionId ?? "").localeCompare(left.versionId ?? "", undefined, { numeric: true })
          || left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
      }
      const favoriteOrder = Number(favoriteIds.has(right.id)) - Number(favoriteIds.has(left.id));
      return favoriteOrder || (right.lastPlayedAt ?? right.createdAt) - (left.lastPlayedAt ?? left.createdAt);
    });
}

export function filterInstalledMods(
  mods: InstalledMod[],
  queryText: string,
  filter: InstalledModFilter,
): InstalledMod[] {
  const query = queryText.trim().toLocaleLowerCase();
  return mods.filter((mod) => {
    const matchesQuery = !query || `${mod.name} ${mod.version ?? ""} ${mod.fileName}`.toLocaleLowerCase().includes(query);
    const matchesState = filter === "all" || (filter === "enabled" ? mod.enabled : !mod.enabled);
    return matchesQuery && matchesState;
  }).sort((left, right) => left.name.localeCompare(right.name, undefined, { sensitivity: "base" }));
}
