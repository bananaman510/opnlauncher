import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  ArrowDownToLine,
  ArrowDownUp,
  ArrowUpRight,
  Boxes,
  Check,
  ChevronDown,
  CircleAlert,
  Clock3,
  Download,
  ExternalLink,
  Filter,
  Layers3,
  LoaderCircle,
  PackageCheck,
  RefreshCw,
  Search,
  ShieldAlert,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import { latestCompatibleModUpdate, latestCompatibleModVersion, matchingInstalledMod } from "../modVersions";
import { filterInstalledMods, type InstalledModFilter } from "../libraryFilters";
import { ModVersionSwitchDialog } from "./ModVersionSwitchDialog";
import type {
  InstalledMod,
  Instance,
  ModInstallSource,
  ModDownloadProgress,
  ModInstanceContext,
  ModProject,
  ModProvider,
  ModSearchResult,
  ModVersion,
} from "../types";

interface ModsPageProps {
  desktop: boolean;
  instances: Instance[];
  selectedInstanceId: string | null;
  onSelectInstance: (instanceId: string) => void;
  onOpenInstances: () => void;
  curseForgeKey: string;
  onCurseForgeKeyChange: (value: string) => void;
  horizonFabricMode?: boolean;
  hidePageHeading?: boolean;
  hideInstancePicker?: boolean;
  onModsChanged?: () => void;
}

type LibraryTab = "discover" | "installed";
type ModSort = "relevance" | "downloads" | "updated" | "newest";
type LoaderFilter = "fabric" | "forge" | "neoforge" | "quilt";
type CardInstallFeedback = { projectKey: string; instanceId: string; kind: "success" | "error"; message: string };
type ModUpdateCandidate = { mod: InstalledMod; source: ModInstallSource; version: ModVersion };

const loaders: { id: LoaderFilter; name: string }[] = [
  { id: "fabric", name: "Fabric" },
  { id: "forge", name: "Forge" },
  { id: "neoforge", name: "NeoForge" },
  { id: "quilt", name: "Quilt" },
];

function formatDownloads(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}m`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}k`;
  return value.toString();
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(0)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function loaderName(loader: string | null | undefined): string {
  return loaders.find((item) => item.id === loader)?.name ?? "No loader detected";
}

function versionDate(value: string): string {
  if (!value) return "Date unavailable";
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value.slice(0, 10) : parsed.toLocaleDateString();
}

function modProjectKey(project: Pick<ModProject, "provider" | "id">): string {
  return `${project.provider}:${project.id}`;
}

export function ModsPage({ desktop, instances, selectedInstanceId, onSelectInstance, onOpenInstances, curseForgeKey, onCurseForgeKeyChange, horizonFabricMode = false, hidePageHeading = false, hideInstancePicker = false, onModsChanged }: ModsPageProps) {
  const [libraryTab, setLibraryTab] = useState<LibraryTab>("discover");
  const [provider, setProvider] = useState<ModProvider>("modrinth");
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [sort, setSort] = useState<ModSort>("downloads");
  const [loaderFilter, setLoaderFilter] = useState<LoaderFilter>("fabric");
  const [instanceContext, setInstanceContext] = useState<ModInstanceContext | null>(null);
  const [projects, setProjects] = useState<ModProject[]>([]);
  const [totalHits, setTotalHits] = useState(0);
  const [searchOffset, setSearchOffset] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState("");
  const [installedMods, setInstalledMods] = useState<InstalledMod[]>([]);
  const [installedError, setInstalledError] = useState("");
  const [installedQuery, setInstalledQuery] = useState("");
  const [installedFilter, setInstalledFilter] = useState<InstalledModFilter>("all");
  const [installedSort, setInstalledSort] = useState<"name" | "size" | "enabled">("name");
  const [availableUpdates, setAvailableUpdates] = useState<ModUpdateCandidate[]>([]);
  const [isCheckingUpdates, setIsCheckingUpdates] = useState(false);
  const [isUpdatingAllMods, setIsUpdatingAllMods] = useState(false);
  const [updateStatus, setUpdateStatus] = useState("");
  const [updateError, setUpdateError] = useState("");
  const [selectedProject, setSelectedProject] = useState<ModProject | null>(null);
  const [projectVersions, setProjectVersions] = useState<ModVersion[]>([]);
  const [selectedVersionId, setSelectedVersionId] = useState("");
  const [isLoadingDetails, setIsLoadingDetails] = useState(false);
  const [detailError, setDetailError] = useState("");
  const [isInstalling, setIsInstalling] = useState(false);
  const [installingProjectKey, setInstallingProjectKey] = useState<string | null>(null);
  const [cardInstallFeedback, setCardInstallFeedback] = useState<CardInstallFeedback | null>(null);
  const [downloadId, setDownloadId] = useState("");
  const [downloadProgress, setDownloadProgress] = useState<ModDownloadProgress | null>(null);
  const [installError, setInstallError] = useState("");
  const [confirmInstall, setConfirmInstall] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<InstalledMod | null>(null);
  const [switchTarget, setSwitchTarget] = useState<InstalledMod | null>(null);
  const [selectedModFiles, setSelectedModFiles] = useState<string[]>([]);
  const [bulkRemoveTarget, setBulkRemoveTarget] = useState<string[] | null>(null);
  const [bulkRemoveError, setBulkRemoveError] = useState("");
  const [isRemovingMods, setIsRemovingMods] = useState(false);
  const [isBulkUpdatingMods, setIsBulkUpdatingMods] = useState(false);
  const [isSwitchingVersion, setIsSwitchingVersion] = useState(false);
  const [busyFile, setBusyFile] = useState("");
  const [imageIndex, setImageIndex] = useState(0);
  const [showKeyInput, setShowKeyInput] = useState(false);
  const [keyDraft, setKeyDraft] = useState(curseForgeKey);
  const activeDownloadId = useRef("");
  const selectedInstanceIdRef = useRef(selectedInstanceId);
  const instanceContextRef = useRef(instanceContext);
  selectedInstanceIdRef.current = selectedInstanceId;
  instanceContextRef.current = instanceContext;

  const selectedVersion = projectVersions.find((version) => version.id === selectedVersionId) ?? projectVersions[0] ?? null;
  const selectedInstance = instances.find((instance) => instance.id === selectedInstanceId) ?? null;
  const requiredDependencyCount = selectedVersion?.dependencies.filter((dependency) => dependency.dependencyType === "required").length ?? 0;
  const selectedProjectIsInstalled = Boolean(selectedProject && matchingInstalledMod(selectedProject, installedMods));
  const visibleInstalledMods = useMemo(() => filterInstalledMods(installedMods, installedQuery, installedFilter).sort((left, right) => {
    if (installedSort === "size") return right.size - left.size || left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
    if (installedSort === "enabled") return Number(right.enabled) - Number(left.enabled) || left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
    return left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
  }), [installedFilter, installedMods, installedQuery, installedSort]);
  const categories = useMemo(() => [...new Set(projects.flatMap((project) => project.categories))].slice(0, 12), [projects]);
  const [categoryFilter, setCategoryFilter] = useState("");
  const filteredProjects = useMemo(() => categoryFilter ? projects.filter((project) => project.categories.includes(categoryFilter)) : projects, [categoryFilter, projects]);
  const canInstall = Boolean(
    desktop
    && selectedProject
    && selectedVersion
    && instanceContext?.instanceId === selectedInstanceId
    && instanceContext.minecraftVersion
    && instanceContext.loader
    && instanceContext.loader === loaderFilter
    && selectedVersion.gameVersions.includes(instanceContext.minecraftVersion)
    && selectedVersion.loaders.includes(instanceContext.loader)
    && !selectedProjectIsInstalled,
  );
  const canInstallFromCard = Boolean(
    desktop
    && selectedInstanceId
    && instanceContext?.instanceId === selectedInstanceId
    && instanceContext.minecraftVersion
    && instanceContext.loader
    && instanceContext.loader === loaderFilter
    && (provider !== "curseforge" || curseForgeKey.trim()),
  );
  const cardInstallDisabledReason = !desktop
    ? "Open the desktop app to install mods."
    : !selectedInstanceId || instanceContext?.instanceId !== selectedInstanceId
      ? "Select an instance to install a compatible mod build."
      : !instanceContext?.minecraftVersion
        ? "The selected instance needs readable Minecraft version metadata."
        : !instanceContext.loader
          ? "Install Fabric or Forge for this instance to enable mod installation."
          : instanceContext.loader !== loaderFilter
            ? "The selected mod loader does not match this instance."
            : provider === "curseforge" && !curseForgeKey.trim()
              ? "Enter a CurseForge API key to install this project."
              : "";
  const progressPercent = downloadProgress
    ? downloadProgress.totalBytes > 0
      ? Math.min(100, Math.round((downloadProgress.downloadedBytes / downloadProgress.totalBytes) * 100))
      : downloadProgress.totalFiles > 0 ? Math.round((downloadProgress.completedFiles / downloadProgress.totalFiles) * 100) : 0
    : 0;

  useEffect(() => {
    const timeout = window.setTimeout(() => setDebouncedQuery(query.trim()), 280);
    return () => window.clearTimeout(timeout);
  }, [query]);

  useEffect(() => {
    if (!desktop || !selectedInstanceId) {
      setInstanceContext(null);
      setInstalledMods([]);
      setAvailableUpdates([]);
      setUpdateStatus("");
      setUpdateError("");
      setSelectedModFiles([]);
      setSwitchTarget(null);
      setRemoveTarget(null);
      setBulkRemoveTarget(null);
      setBulkRemoveError("");
      return;
    }
    let cancelled = false;
    setInstanceContext(null);
    setSelectedModFiles([]);
    setInstalledError("");
    void Promise.all([
      invoke<ModInstanceContext>("inspect_instance_mod_context", { instanceId: selectedInstanceId }),
      invoke<InstalledMod[]>("list_installed_mods", { instanceId: selectedInstanceId }),
    ]).then(([context, installed]) => {
      if (cancelled) return;
      setInstanceContext(context);
      setInstalledMods(installed);
      setAvailableUpdates([]);
      setUpdateStatus("");
      setUpdateError("");
      if (context.loader && loaders.some((loader) => loader.id === context.loader)) setLoaderFilter(context.loader as LoaderFilter);
    }).catch((reason) => {
      if (!cancelled) setInstalledError(String(reason));
    });
    return () => { cancelled = true; };
  }, [desktop, selectedInstanceId]);

  useEffect(() => {
    if (provider === "curseforge" && !curseForgeKey.trim()) {
      setProjects([]);
      setTotalHits(0);
      setIsLoading(false);
      setError("");
      return;
    }
    if (!desktop || !selectedInstanceId || !instanceContext?.minecraftVersion) return;
    if (!loaders.some((loader) => loader.id === loaderFilter)) return;
    let cancelled = false;
    setIsLoading(true);
    setError("");
    if (searchOffset === 0) setProjects([]);
    void invoke<ModSearchResult>("search_mod_projects", {
      provider,
      query: debouncedQuery,
      gameVersion: instanceContext.minecraftVersion,
      loader: loaderFilter,
      sort,
      offset: searchOffset,
      apiKey: provider === "curseforge" ? curseForgeKey || null : null,
    }).then((result) => {
      if (cancelled) return;
      setProjects((current) => searchOffset === 0 ? result.projects : [...current, ...result.projects]);
      setTotalHits(result.totalHits);
    }).catch((reason) => {
      if (!cancelled) {
        setError(String(reason));
        if (searchOffset === 0) setProjects([]);
      }
    }).finally(() => {
      if (!cancelled) setIsLoading(false);
    });
    return () => { cancelled = true; };
  }, [curseForgeKey, debouncedQuery, desktop, instanceContext?.minecraftVersion, loaderFilter, provider, searchOffset, selectedInstanceId, sort]);

  useEffect(() => {
    if (!desktop) return;
    let unlisten: UnlistenFn | undefined;
    void listen<ModDownloadProgress>("mod-download-progress", (event) => {
      if (event.payload.downloadId === activeDownloadId.current) setDownloadProgress(event.payload);
    }).then((stop) => { unlisten = stop; });
    return () => { unlisten?.(); };
  }, [desktop]);

  async function refreshInstalled() {
    if (!desktop || !selectedInstanceId) return;
    try {
      const installed = await invoke<InstalledMod[]>("list_installed_mods", { instanceId: selectedInstanceId });
      setInstalledMods(installed);
      setAvailableUpdates((current) => current.filter((candidate) => installed.some((mod) => mod.fileName === candidate.mod.fileName)));
      setSelectedModFiles((current) => current.filter((fileName) => installed.some((mod) => mod.fileName === fileName)));
      setInstalledError("");
    } catch (reason) {
      setInstalledError(String(reason));
    }
  }

  async function checkForUpdates() {
    const context = instanceContext;
    const targetInstanceId = selectedInstanceId;
    if (!desktop || !targetInstanceId || !context?.minecraftVersion || !context.loader || isCheckingUpdates || isUpdatingAllMods) return;
    setIsCheckingUpdates(true);
    setAvailableUpdates([]);
    setUpdateStatus("Checking installed mods with their original providers…");
    setUpdateError("");
    setInstalledError("");
    const candidates: ModUpdateCandidate[] = [];
    let untracked = Math.max(0, installedMods.length - 100);
    const checkableMods = installedMods.slice(0, 100);
    try {
      for (let index = 0; index < checkableMods.length; index += 1) {
        if (selectedInstanceIdRef.current !== targetInstanceId) return;
        const mod = checkableMods[index];
        setUpdateStatus(`Checking ${index + 1} of ${checkableMods.length}: ${mod.name}`);
        try {
          const source = mod.source ?? await invoke<ModInstallSource | null>("resolve_installed_mod_source", {
            instanceId: targetInstanceId,
            fileName: mod.fileName,
          });
          if (!source?.versionId) {
            untracked += 1;
            continue;
          }
          const versions = await invoke<ModVersion[]>("get_all_mod_project_versions", {
            provider: source.provider,
            projectId: source.projectId,
            apiKey: source.provider === "curseforge" ? curseForgeKey || null : null,
          });
          const version = latestCompatibleModUpdate(versions, source.versionId, context.minecraftVersion, context.loader);
          if (version) candidates.push({ mod, source, version });
        } catch {
          untracked += 1;
        }
      }
      if (selectedInstanceIdRef.current !== targetInstanceId) return;
      setAvailableUpdates(candidates);
      const skippedMessage = untracked > 0 ? ` ${untracked} file${untracked === 1 ? " was" : "s were"} unlinked or could not be checked.` : "";
      setUpdateStatus(candidates.length ? `${candidates.length} compatible update${candidates.length === 1 ? "" : "s"} found.${skippedMessage}` : `No compatible updates found.${skippedMessage}`);
    } finally {
      setIsCheckingUpdates(false);
    }
  }

  async function updateAllMods() {
    if (!availableUpdates.length || isUpdatingAllMods || !selectedInstanceId) return;
    const targetInstanceId = selectedInstanceId;
    const pending = [...availableUpdates];
    const failed: string[] = [];
    let succeeded = 0;
    setIsUpdatingAllMods(true);
    setUpdateError("");
    setUpdateStatus(`Updating 0 of ${pending.length} mods…`);
    for (let index = 0; index < pending.length; index += 1) {
      const candidate = pending[index];
      if (selectedInstanceIdRef.current !== targetInstanceId) {
        failed.push(`${candidate.mod.name}: stopped because the selected instance changed`);
        break;
      }
      setUpdateStatus(`Updating ${index + 1} of ${pending.length}: ${candidate.mod.name}`);
      try {
        const updatedMods = await switchInstalledModVersion(candidate.mod, candidate.source, candidate.version);
        succeeded += 1;
        setAvailableUpdates((current) => current.filter((item) => item.mod.fileName !== candidate.mod.fileName));
        for (let nextIndex = index + 1; nextIndex < pending.length; nextIndex += 1) {
          const linked = updatedMods.find((mod) => mod.source?.provider === pending[nextIndex].source.provider && mod.source.projectId === pending[nextIndex].source.projectId);
          if (linked) pending[nextIndex] = { ...pending[nextIndex], mod: linked };
        }
      } catch (reason) {
        failed.push(`${candidate.mod.name}: ${reason instanceof Error ? reason.message : String(reason)}`);
      }
    }
    setIsUpdatingAllMods(false);
    if (failed.length) setUpdateStatus(`${succeeded} updated; ${failed.length} could not be updated. Review the error below and rescan.`);
    else setUpdateStatus(`Updated ${pending.length} mod${pending.length === 1 ? "" : "s"} for Minecraft ${instanceContext?.minecraftVersion} · ${loaderName(instanceContext?.loader)}.`);
    if (failed.length) setUpdateError(failed.join("\n"));
  }

  function restartSearch() {
    setSearchOffset(0);
    setCategoryFilter("");
  }

  async function openProject(project: ModProject) {
    if (isInstalling) return;
    setSelectedProject(project);
    setProjectVersions([]);
    setSelectedVersionId("");
    setDetailError("");
    setInstallError("");
    setImageIndex(0);
    setIsLoadingDetails(true);
    try {
      const [details, versions] = await Promise.all([
          invoke<ModProject>("get_mod_project", { provider: project.provider, projectId: project.id, apiKey: project.provider === "curseforge" ? curseForgeKey || null : null }).catch(() => project),
        invoke<ModVersion[]>("get_mod_project_versions", {
          provider: project.provider,
          projectId: project.id,
          gameVersion: instanceContext?.minecraftVersion ?? "",
          loader: loaderFilter,
          apiKey: project.provider === "curseforge" ? curseForgeKey || null : null,
        }),
      ]);
      setSelectedProject(details);
      setProjectVersions(versions);
      setSelectedVersionId(versions[0]?.id ?? "");
      if (versions.length === 0) setDetailError("No compatible versions were found for this Minecraft version and loader.");
    } catch (reason) {
      setDetailError(String(reason));
    } finally {
      setIsLoadingDetails(false);
    }
  }

  async function startInstall() {
    if (!selectedProject || !selectedVersion || !selectedInstanceId || !canInstall || isInstalling) return;
    const targetInstanceId = selectedInstanceId;
    const targetProject = selectedProject;
    const targetVersion = selectedVersion;
    const targetApiKey = targetProject.provider === "curseforge" ? curseForgeKey || null : null;
    const id = crypto.randomUUID();
    activeDownloadId.current = id;
    setDownloadId(id);
    setDownloadProgress({ downloadId: id, projectTitle: selectedProject.title, phase: "Preparing download", currentFile: "", completedFiles: 0, totalFiles: 0, downloadedBytes: 0, totalBytes: 0 });
    setIsInstalling(true);
    setInstallError("");
    setConfirmInstall(false);
    try {
      const installed = await invoke<InstalledMod[]>("install_mod_version", {
        instanceId: targetInstanceId,
        provider: targetProject.provider,
        projectId: targetProject.id,
        versionId: targetVersion.id,
        downloadId: id,
        apiKey: targetApiKey,
      });
      if (selectedInstanceIdRef.current === targetInstanceId) {
        setInstalledMods(installed);
        setInstalledError("");
        setLibraryTab("installed");
        setSelectedProject(null);
      }
      onModsChanged?.();
      setDownloadProgress(null);
    } catch (reason) {
      setInstallError(String(reason));
    } finally {
      activeDownloadId.current = "";
      setDownloadId("");
      setIsInstalling(false);
    }
  }

  async function installProjectFromCard(project: ModProject) {
    const targetInstanceId = selectedInstanceId;
    const targetContext = instanceContext;
    const projectKey = modProjectKey(project);
    if (!canInstallFromCard || !targetInstanceId || !targetContext?.minecraftVersion || !targetContext.loader || isInstalling) return;

    setInstallingProjectKey(projectKey);
    setCardInstallFeedback(null);
    setInstallError("");
    setIsInstalling(true);
    try {
      const versions = await invoke<ModVersion[]>("get_mod_project_versions", {
        provider: project.provider,
        projectId: project.id,
        gameVersion: targetContext.minecraftVersion,
        loader: targetContext.loader,
        apiKey: project.provider === "curseforge" ? curseForgeKey || null : null,
      });
      const version = latestCompatibleModVersion(versions, targetContext.minecraftVersion, targetContext.loader);
      if (!version) {
        throw new Error(`No compatible ${loaderName(targetContext.loader)} build was found for Minecraft ${targetContext.minecraftVersion}.`);
      }
      if (selectedInstanceIdRef.current !== targetInstanceId || instanceContextRef.current?.instanceId !== targetInstanceId) {
        throw new Error("The selected instance changed while checking versions. Select the target instance and try again.");
      }

      const id = crypto.randomUUID();
      activeDownloadId.current = id;
      setDownloadId(id);
      setDownloadProgress({ downloadId: id, projectTitle: project.title, phase: `Installing ${version.versionNumber}`, currentFile: "", completedFiles: 0, totalFiles: 0, downloadedBytes: 0, totalBytes: 0 });
      const installed = await invoke<InstalledMod[]>("install_mod_version", {
        instanceId: targetInstanceId,
        provider: project.provider,
        projectId: project.id,
        versionId: version.id,
        downloadId: id,
        apiKey: project.provider === "curseforge" ? curseForgeKey || null : null,
      });
      if (selectedInstanceIdRef.current === targetInstanceId) {
        setInstalledMods(installed);
        setInstalledError("");
      }
      setCardInstallFeedback({
        projectKey,
        instanceId: targetInstanceId,
        kind: "success",
        message: `Installed ${version.versionNumber} for Minecraft ${targetContext.minecraftVersion} · ${loaderName(targetContext.loader)}.`,
      });
      onModsChanged?.();
      setDownloadProgress(null);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setCardInstallFeedback({ projectKey, instanceId: targetInstanceId, kind: "error", message });
      setInstallError(message);
      setDownloadProgress(null);
    } finally {
      activeDownloadId.current = "";
      setDownloadId("");
      setIsInstalling(false);
      setInstallingProjectKey(null);
    }
  }

  async function cancelInstall() {
    if (!downloadId) return;
    try { await invoke("cancel_download", { downloadId }); } catch (reason) { setInstallError(String(reason)); }
  }

  async function toggleMod(mod: InstalledMod) {
    if (!selectedInstanceId || isBulkUpdatingMods) return;
    setBusyFile(mod.fileName);
    try {
      await invoke("set_mod_enabled", { instanceId: selectedInstanceId, fileName: mod.fileName, enabled: !mod.enabled });
      await refreshInstalled();
      onModsChanged?.();
    } catch (reason) {
      setInstalledError(String(reason));
    } finally {
      setBusyFile("");
    }
  }

  async function removeMod() {
    if (!selectedInstanceId || !removeTarget || isBulkUpdatingMods) return;
    setBusyFile(removeTarget.fileName);
    try {
      await invoke("remove_mod_file", { instanceId: selectedInstanceId, fileName: removeTarget.fileName, confirmed: true });
      setRemoveTarget(null);
      await refreshInstalled();
      onModsChanged?.();
    } catch (reason) {
      setInstalledError(String(reason));
    } finally {
      setBusyFile("");
    }
  }

  function toggleModSelection(fileName: string) {
    setSelectedModFiles((current) => current.includes(fileName)
      ? current.filter((selected) => selected !== fileName)
      : [...current, fileName]);
  }

  function toggleAllModSelection() {
    const visibleNames = visibleInstalledMods.map((mod) => mod.fileName);
    const allVisibleSelected = visibleNames.length > 0 && visibleNames.every((fileName) => selectedModFiles.includes(fileName));
    setSelectedModFiles((current) => allVisibleSelected
      ? current.filter((fileName) => !visibleNames.includes(fileName))
      : [...new Set([...current, ...visibleNames])]);
  }

  async function setSelectedModsEnabled() {
    if (!selectedInstanceId || selectedModFiles.length === 0 || isBulkUpdatingMods || busyFile || isSwitchingVersion || isRemovingMods) return;
    const selectedMods = installedMods.filter((mod) => selectedModFiles.includes(mod.fileName));
    const enable = selectedMods.some((mod) => !mod.enabled);
    setIsBulkUpdatingMods(true);
    setInstalledError("");
    try {
      for (const mod of selectedMods) {
        if (mod.enabled !== enable) {
          await invoke("set_mod_enabled", { instanceId: selectedInstanceId, fileName: mod.fileName, enabled: enable });
        }
      }
      setSelectedModFiles([]);
      await refreshInstalled();
      onModsChanged?.();
    } catch (reason) {
      setInstalledError(`Some selected mods may not have changed. ${String(reason)}`);
      await refreshInstalled();
    } finally {
      setIsBulkUpdatingMods(false);
    }
  }

  async function removeBulkTarget() {
    if (!selectedInstanceId || !bulkRemoveTarget?.length || isRemovingMods || isBulkUpdatingMods) return;
    setIsRemovingMods(true);
    try {
      await invoke("remove_mod_files", { instanceId: selectedInstanceId, fileNames: bulkRemoveTarget, confirmed: true });
      setBulkRemoveTarget(null);
      setSelectedModFiles([]);
      await refreshInstalled();
      onModsChanged?.();
    } catch (reason) {
      const message = String(reason);
      setInstalledError(message);
      setBulkRemoveError(message);
    } finally {
      setIsRemovingMods(false);
    }
  }

  async function switchInstalledModVersion(mod: InstalledMod, source: ModInstallSource, version: ModVersion): Promise<InstalledMod[]> {
    if (!selectedInstanceId || !source || isSwitchingVersion || isBulkUpdatingMods) return [];
    const targetInstanceId = selectedInstanceId;
    const id = crypto.randomUUID();
    activeDownloadId.current = id;
    setDownloadId(id);
    setDownloadProgress({ downloadId: id, projectTitle: mod.name, phase: `Switching to ${version.versionNumber}`, currentFile: "", completedFiles: 0, totalFiles: 0, downloadedBytes: 0, totalBytes: 0 });
    setIsSwitchingVersion(true);
    setInstallError("");
    try {
      const installed = await invoke<InstalledMod[]>("switch_mod_version", {
        instanceId: targetInstanceId,
        oldFileName: mod.fileName,
        provider: source.provider,
        projectId: source.projectId,
        versionId: version.id,
        downloadId: id,
        apiKey: source.provider === "curseforge" ? curseForgeKey || null : null,
      });
      if (selectedInstanceIdRef.current === targetInstanceId) {
        setInstalledMods(installed);
        setInstalledError("");
        setSelectedModFiles((current) => current.filter((fileName) => fileName !== mod.fileName));
      }
      onModsChanged?.();
      setDownloadProgress(null);
      return installed;
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setInstallError(message);
      setDownloadProgress(null);
      throw reason;
    } finally {
      activeDownloadId.current = "";
      setDownloadId("");
      setIsSwitchingVersion(false);
    }
  }

  const showingCurseForgeKeyPrompt = provider === "curseforge" && curseForgeKey.trim().length === 0;

  return (
    <>
      {!horizonFabricMode && !hidePageHeading && <section className="mods-page-heading">
        <div>
          <p className="eyebrow"><PackageCheck size={14} /> MODDING WORKSPACE</p>
          <h1>Mods, made manageable.</h1>
          <p className="page-subtitle">Explore trusted projects, choose the build for your exact Minecraft and loader version, and keep each instance's mods tidy.</p>
        </div>
        <span className="mods-heading-badge"><Sparkles size={15} /> Curated by their authors</span>
      </section>}

      {!desktop && <div className="notice-banner"><CircleAlert size={17} /><span>Open OpnLauncher as a desktop app to browse providers and manage your local mods.</span></div>}

      <section className="mod-instance-context">
        <div className="mod-context-icon"><Boxes size={19} /></div>
        <div className="mod-context-picker">
          <label htmlFor={hideInstancePicker ? undefined : "mods-instance"}>Working with this instance</label>
          {hideInstancePicker ? <span className="mod-context-empty">{selectedInstance?.name ?? "Choose a Horizon profile"}</span> : instances.length ? <div className="select-wrap"><select id="mods-instance" className="field-control" value={selectedInstanceId ?? ""} onChange={(event) => { onSelectInstance(event.target.value); restartSearch(); }}>
            {instances.map((instance) => <option value={instance.id} key={instance.id}>{instance.name}{instance.versionId ? ` · ${instance.versionId}` : ""}</option>)}
          </select><ChevronDown size={16} /></div> : <span className="mod-context-empty">Add an instance to begin.</span>}
        </div>
        {instanceContext?.minecraftVersion && <span className="mod-version-chip"><Layers3 size={14} /> Minecraft {instanceContext.minecraftVersion}</span>}
        {instanceContext?.loader ? <span className="mod-loader-chip"><span className={`loader-mark loader-${instanceContext.loader}`} />{horizonFabricMode ? "Fabric Loader" : loaderName(instanceContext.loader)}</span> : <label className="mod-loader-filter"><span>Browse for</span><select aria-label="Choose a loader to browse" value={loaderFilter} onChange={(event) => { setLoaderFilter(event.target.value as LoaderFilter); restartSearch(); }}>{loaders.map((loader) => <option value={loader.id} key={loader.id}>{loader.name}</option>)}</select><ChevronDown size={14} /></label>}
      </section>
      {horizonFabricMode && <div className="mods-java-runtime-note"><ShieldAlert size={15} /><span><strong>Optional Java + Fabric mode.</strong> These unchanged Fabric JARs run in this profile's isolated Java Minecraft copy. They do not run inside the Rust Horizon engine; switch the profile back to Horizon Rust in Horizon settings to use its separate worlds and content.</span></div>}
      {instanceContext?.sharedGameDirectory && <div className="mods-shared-warning"><CircleAlert size={15} /><span>This game folder is shared with another instance. Changes to its mods folder may affect both.</span></div>}
      {instanceContext && (!instanceContext.minecraftVersion || !instanceContext.loader) && <div className="mods-loader-notice"><CircleAlert size={15} /><span>{instanceContext.minecraftVersion ? "This instance is vanilla. Configure its Fabric or Forge version in Instances → Game setup; this mod list will then match it automatically." : "Choose a Minecraft version and Fabric or Forge loader for this instance in Instances → Game setup before installing mods."}</span><button type="button" className="text-link" onClick={onOpenInstances}>Configure instance <ArrowUpRight size={14} /></button></div>}

      <section className="mod-browser-shell">
        <div className="mod-browser-header">
          <div className="mod-tabs" role="tablist" aria-label="Mod library">
            <button type="button" role="tab" aria-selected={libraryTab === "discover"} className={libraryTab === "discover" ? "selected" : ""} onClick={() => setLibraryTab("discover")}><Search size={16} /> Discover</button>
            <button type="button" role="tab" aria-selected={libraryTab === "installed"} className={libraryTab === "installed" ? "selected" : ""} onClick={() => { setLibraryTab("installed"); void refreshInstalled(); }}><Boxes size={16} /> Installed <span className="mod-tab-count">{installedMods.length}</span></button>
          </div>
          <div className="provider-switch" role="group" aria-label="Mod provider">
            <button type="button" className={provider === "modrinth" ? "selected" : ""} onClick={() => { setProvider("modrinth"); restartSearch(); }}><span className="provider-dot provider-modrinth" /> Modrinth</button>
            <button type="button" className={provider === "curseforge" ? "selected" : ""} onClick={() => { setProvider("curseforge"); restartSearch(); }}><span className="provider-dot provider-curseforge" /> CurseForge</button>
          </div>
        </div>

        {provider === "curseforge" && <div className={`curseforge-key-banner ${showKeyInput ? "editing" : ""}`}>
          <div className="key-banner-icon"><ShieldCheck size={17} /></div>
          <div className="key-banner-copy"><strong>{curseForgeKey ? "CurseForge access is ready" : "Connect your CurseForge API key"}</strong><span>{curseForgeKey ? "This key stays in memory for this app session and is never saved to disk." : "The official CurseForge API requires your own key. OpnLauncher never embeds or persists it."}</span></div>
          {!showKeyInput && <button type="button" className="secondary-button key-edit-button" onClick={() => { setKeyDraft(curseForgeKey); setShowKeyInput(true); }}>{curseForgeKey ? "Change key" : "Enter key"}</button>}
          {showKeyInput && <div className="key-input-row"><input type="password" autoComplete="off" spellCheck={false} aria-label="CurseForge API key" placeholder="Paste your API key" value={keyDraft} onChange={(event) => setKeyDraft(event.target.value)} /><button type="button" className="secondary-button" disabled={keyDraft.trim().length < 16} onClick={() => { onCurseForgeKeyChange(keyDraft.trim()); setShowKeyInput(false); restartSearch(); }}>Use key</button><button type="button" className="icon-button" onClick={() => { setKeyDraft(curseForgeKey); setShowKeyInput(false); }} aria-label="Cancel editing key"><X size={16} /></button>{curseForgeKey && <button type="button" className="text-link" onClick={() => { onCurseForgeKeyChange(""); setKeyDraft(""); setShowKeyInput(false); restartSearch(); }}>Remove key</button>}<a href="https://console.curseforge.com/" target="_blank" rel="noreferrer"><ExternalLink size={14} /> API console</a></div>}
        </div>}

        <div className="mod-browser-safety"><ShieldAlert size={15} /><span>Mods are third-party executable code. Install only projects you trust.</span><span className="mod-safety-spacer" />
          <span><span className="provider-dot provider-modrinth" /> Modrinth API</span><span className="provider-attribution">·</span><span><span className="provider-dot provider-curseforge" /> CurseForge API</span>
        </div>

        {libraryTab === "discover" ? (
          <>
            <div className="mod-search-toolbar">
              <label className="mod-search-field"><Search size={17} /><input type="search" placeholder="Search mods, authors, and features" value={query} onChange={(event) => { setQuery(event.target.value); setSearchOffset(0); }} aria-label="Search mods" /><kbd>↵</kbd></label>
              <label className="mod-sort-select"><SlidersHorizontal size={15} /><select value={sort} onChange={(event) => { setSort(event.target.value as ModSort); restartSearch(); }} aria-label="Sort mod results"><option value="downloads">Most downloaded</option><option value="relevance">Most relevant</option><option value="updated">Recently updated</option><option value="newest">Newest projects</option></select><ChevronDown size={14} /></label>
              <label className="mod-loader-filter"><Filter size={15} /><select value={loaderFilter} disabled={Boolean(instanceContext?.loader)} onChange={(event) => { setLoaderFilter(event.target.value as LoaderFilter); restartSearch(); }} aria-label="Filter by mod loader">{loaders.map((loader) => <option value={loader.id} key={loader.id}>{loader.name}</option>)}</select><ChevronDown size={14} /></label>
            </div>
            {categories.length > 0 && <div className="mod-category-row"><button type="button" className={!categoryFilter ? "selected" : ""} onClick={() => setCategoryFilter("")}>All categories</button>{categories.map((category) => <button type="button" key={category} className={categoryFilter === category ? "selected" : ""} onClick={() => setCategoryFilter(category)}>{category.replaceAll("_", " ")}</button>)}</div>}

            {error && <div className="error-banner mod-error" role="alert"><CircleAlert size={16} /><span>{error}</span>{provider === "curseforge" && !curseForgeKey && <button type="button" className="text-link" onClick={() => setShowKeyInput(true)}>Set up key</button>}</div>}
            {!selectedInstanceId ? <div className="mod-empty-state"><span className="mod-empty-icon"><Boxes size={23} /></span><strong>Choose an instance to see compatible mods</strong><span>Search results and available builds will match that instance's Minecraft version and loader.</span></div>
              : !instanceContext?.minecraftVersion ? <div className="mod-empty-state"><span className="mod-empty-icon"><Layers3 size={23} /></span><strong>This instance needs a Minecraft version</strong><span>Install a vanilla version or register an existing game folder before browsing compatible mods.</span></div>
                : showingCurseForgeKeyPrompt ? <div className="mod-empty-state"><span className="mod-empty-icon"><ShieldCheck size={22} /></span><strong>Connect your CurseForge API key</strong><span>CurseForge requires a personal API key for browsing. Enter one above; OpnLauncher keeps it in memory only for this session.</span></div>
                : <>
                  <div className="mod-results-heading"><div><span className="eyebrow">{provider === "modrinth" ? "MODRINTH LIBRARY" : "CURSEFORGE LIBRARY"}</span><strong>{totalHits.toLocaleString()} projects</strong><small>for Minecraft {instanceContext.minecraftVersion} · {loaderName(loaderFilter)}</small></div><button className="text-link" type="button" onClick={() => { restartSearch(); setQuery(""); }}><Clock3 size={14} /> Reset filters</button></div>
                  {isLoading && projects.length === 0 ? <div className="mod-loading-state"><LoaderCircle className="spin" size={23} /><span>Finding matching projects…</span></div>
                    : filteredProjects.length ? <div className="mod-project-grid">{filteredProjects.map((project) => {
                      const projectKey = modProjectKey(project);
                      const feedback = cardInstallFeedback?.projectKey === projectKey && cardInstallFeedback.instanceId === selectedInstanceId ? cardInstallFeedback : null;
                      const installedMatch = matchingInstalledMod(project, installedMods);
                      const isCardInstalling = installingProjectKey === projectKey && isInstalling;
                      return <article className="mod-project-card" key={projectKey}>
                      <button type="button" className="mod-card-cover" onClick={() => void openProject(project)} aria-label={`View ${project.title}`}>
                        {project.gallery[0]?.url ? <img className="mod-cover-image" src={project.gallery[0].url} loading="lazy" alt="" onError={(event) => { event.currentTarget.style.display = "none"; }} /> : <span className="mod-cover-art"><span /><span /><span />{project.iconUrl && <img className="mod-cover-project-icon" src={project.iconUrl} loading="lazy" alt="" onError={(event) => { event.currentTarget.style.display = "none"; }} />}</span>}
                        <span className="mod-cover-shade" />
                        <span className="mod-cover-count"><Download size={13} /> {formatDownloads(project.downloads)}</span>
                        <span className={`mod-provider-pill provider-pill-${project.provider}`}>{project.provider === "modrinth" ? "MODRINTH" : "CURSEFORGE"}</span>
                      </button>
                      <div className="mod-card-body">
                        <div className="mod-card-title-row"><span className="mod-project-icon">{project.iconUrl ? <img src={project.iconUrl} loading="lazy" alt="" onError={(event) => { event.currentTarget.style.display = "none"; }} /> : <PackageCheck size={20} />}</span><button type="button" className="mod-project-title" onClick={() => void openProject(project)}><strong>{project.title}</strong><small>by {project.author}</small></button><button type="button" className="mod-card-open" aria-label={`View ${project.title} details`} onClick={() => void openProject(project)}><ArrowUpRight size={16} /></button></div>
                        <p className="mod-card-summary">{project.summary}</p>
                        <div className="mod-card-meta"><span><Download size={13} /> {formatDownloads(project.downloads)}</span><span><Layers3 size={13} /> {project.gameVersions.includes(instanceContext.minecraftVersion ?? "") ? instanceContext.minecraftVersion : "Version match"}</span></div>
                        <div className="mod-card-tags">{project.categories.slice(0, 3).map((tag) => <span key={tag}>{tag.replaceAll("_", " ")}</span>)}</div>
                        <button
                          className="play-button mod-card-install"
                          type="button"
                          disabled={!canInstallFromCard || isInstalling || Boolean(installedMatch) || feedback?.kind === "success"}
                          title={installedMatch ? `Already installed as ${installedMatch.fileName}.` : feedback?.kind === "success" ? "Installed in this instance." : cardInstallDisabledReason || `Install the latest compatible ${loaderName(instanceContext?.loader)} build for Minecraft ${instanceContext?.minecraftVersion}.`}
                          aria-label={installedMatch ? `${project.title} is already installed` : `Install the latest compatible ${project.title} build`}
                          onClick={() => void installProjectFromCard(project)}
                        >
                          {isCardInstalling ? <LoaderCircle className="spin" size={15} /> : installedMatch || feedback?.kind === "success" ? <Check size={15} /> : <Download size={15} />}
                          {isCardInstalling ? downloadId ? "Installing…" : "Finding build…" : installedMatch || feedback?.kind === "success" ? "Installed" : "Install"}
                        </button>
                        {feedback && <p className={`mod-card-install-feedback ${feedback.kind}`} role={feedback.kind === "error" ? "alert" : "status"}>{feedback.message}</p>}
                      </div>
                    </article>;
                    })}</div> : !isLoading && <div className="mod-empty-state compact"><span className="mod-empty-icon"><Search size={21} /></span><strong>No matching projects</strong><span>Try another term or loader filter.</span></div>}
                  {isLoading && projects.length > 0 && <div className="mod-fetching-more"><LoaderCircle className="spin" size={16} /> Updating results…</div>}
                  {projects.length < totalHits && !isLoading && <div className="mod-load-more"><button className="secondary-button" type="button" onClick={() => setSearchOffset((current) => current + 24)}><ArrowDownToLine size={15} /> Show more projects</button></div>}
                </>}
          </>
        ) : (
          <div className="installed-mods-panel">
            <div className="installed-mods-heading"><div><span className="eyebrow">INSTANCE CONTENT</span><h2>{installedMods.length} installed mod{installedMods.length === 1 ? "" : "s"}</h2><p>Search, compare compatible releases, update, enable, or remove files in this instance.</p></div><div className="installed-mods-tools"><button className="secondary-button" type="button" onClick={() => void checkForUpdates()} disabled={!desktop || !installedMods.length || isCheckingUpdates || isUpdatingAllMods}><RefreshCw className={isCheckingUpdates ? "spin" : ""} size={15} />{isCheckingUpdates ? "Checking…" : "Check updates"}</button>{availableUpdates.length > 0 && <button className="play-button" type="button" onClick={() => void updateAllMods()} disabled={isUpdatingAllMods || isCheckingUpdates || isSwitchingVersion}><ArrowDownToLine size={15} />{isUpdatingAllMods ? "Updating…" : `Update all · ${availableUpdates.length}`}</button>}{installedMods.length > 0 && <><button className="secondary-button" type="button" onClick={toggleAllModSelection} disabled={isBulkUpdatingMods || isUpdatingAllMods}>{visibleInstalledMods.length > 0 && visibleInstalledMods.every((mod) => selectedModFiles.includes(mod.fileName)) ? "Clear visible" : "Select visible"}</button><button className="danger-button" type="button" disabled={isBulkUpdatingMods || isUpdatingAllMods} onClick={() => { setBulkRemoveError(""); setBulkRemoveTarget(installedMods.map((mod) => mod.fileName)); }}><Trash2 size={14} /> Delete all</button></>}</div></div>
            {updateStatus && <div className="mod-update-status" role="status"><span className={isCheckingUpdates || isUpdatingAllMods ? "is-working" : ""} />{updateStatus}</div>}
            {updateError && <div className="error-banner mod-error mod-update-error" role="alert"><CircleAlert size={15} /><span>{updateError}</span></div>}
            {installedMods.length > 0 && <div className="installed-mods-toolbar"><label className="search-box"><Search size={16} /><input aria-label="Search installed mods" placeholder="Find a mod or file" value={installedQuery} onChange={(event) => setInstalledQuery(event.target.value)} /></label><div className="installed-state-filters" role="group" aria-label="Filter installed mods">{([{ id: "all", label: "All" }, { id: "enabled", label: "Enabled" }, { id: "disabled", label: "Disabled" }] as const).map((filter) => <button type="button" key={filter.id} className={installedFilter === filter.id ? "selected" : ""} onClick={() => setInstalledFilter(filter.id)} aria-pressed={installedFilter === filter.id}>{filter.label}{filter.id === "all" ? <small>{installedMods.length}</small> : <small>{installedMods.filter((mod) => filter.id === "enabled" ? mod.enabled : !mod.enabled).length}</small>}</button>)}</div><label className="installed-sort"><SlidersHorizontal size={14} /><select value={installedSort} onChange={(event) => setInstalledSort(event.target.value as typeof installedSort)} aria-label="Sort installed mods"><option value="name">Name</option><option value="size">Largest files</option><option value="enabled">Enabled first</option></select><ChevronDown size={13} /></label></div>}
            {!desktop ? <div className="mod-empty-state compact"><span>Open the desktop app to read local files.</span></div>
              : installedError ? <div className="error-banner mod-error" role="alert"><CircleAlert size={16} />{installedError}</div>
                : installedMods.length === 0 ? <div className="mod-empty-state"><span className="mod-empty-icon"><PackageCheck size={22} /></span><strong>No mod files here yet</strong><span>Pick a modded instance and install a compatible project from Discover.</span><button className="secondary-button" type="button" onClick={() => setLibraryTab("discover")}><Search size={15} /> Browse mods</button></div>
                  : <>
                  {selectedModFiles.length > 0 && <div className="installed-selection-bar"><span>{selectedModFiles.length} of {installedMods.length} selected</span><div><button className="secondary-button" type="button" onClick={() => void setSelectedModsEnabled()} disabled={!desktop || isBulkUpdatingMods || Boolean(busyFile) || isSwitchingVersion || isRemovingMods || isUpdatingAllMods}>{isBulkUpdatingMods ? <LoaderCircle className="spin" size={14} /> : <Check size={14} />}{isBulkUpdatingMods ? "Updating…" : selectedModFiles.some((fileName) => installedMods.some((mod) => mod.fileName === fileName && !mod.enabled)) ? "Enable selected" : "Disable selected"}</button><button className="danger-button" type="button" disabled={isBulkUpdatingMods || isUpdatingAllMods} onClick={() => { setBulkRemoveError(""); setBulkRemoveTarget([...selectedModFiles]); }}><Trash2 size={14} /> Delete selected</button></div></div>}
                  {visibleInstalledMods.length === 0 ? <div className="mod-empty-state compact"><span>No installed mods match these filters.</span><button type="button" className="text-link" onClick={() => { setInstalledQuery(""); setInstalledFilter("all"); }}>Clear search and filters</button></div> : <div className="installed-mod-list">{visibleInstalledMods.map((mod) => { const update = availableUpdates.find((candidate) => candidate.mod.fileName === mod.fileName); return <article className={`installed-mod-row ${mod.enabled ? "" : "is-disabled"} ${update ? "has-update" : ""}`} key={mod.fileName}>
                    <input className="installed-mod-checkbox" type="checkbox" checked={selectedModFiles.includes(mod.fileName)} onChange={() => toggleModSelection(mod.fileName)} aria-label={`Select ${mod.name}`} disabled={isBulkUpdatingMods || isUpdatingAllMods} />
                    <span className={`installed-mod-icon ${mod.enabled ? "" : "disabled"}`}>{mod.enabled ? <PackageCheck size={18} /> : <CircleAlert size={18} />}</span>
                    <span className="installed-mod-copy"><strong>{mod.name}{update && <span className="mod-update-pill">Update ready</span>}</strong><small>{mod.version ? `Version ${mod.version} · ` : ""}{mod.fileName} · {formatBytes(mod.size)}{update ? ` · latest ${update.version.versionNumber}` : ""}</small></span>
                    <span className={`installed-mod-state ${mod.enabled ? "enabled" : "disabled"}`}>{mod.enabled ? "Enabled" : "Disabled"}</span>
                    {update && <button className="play-button installed-update" type="button" disabled={!desktop || isSwitchingVersion || isBulkUpdatingMods || isUpdatingAllMods} onClick={() => { void switchInstalledModVersion(mod, update.source, update.version).then(() => { setAvailableUpdates((current) => current.filter((candidate) => candidate.mod.fileName !== mod.fileName)); setUpdateError(""); }).catch((reason) => { setUpdateStatus(`Could not update ${mod.name}.`); setUpdateError(reason instanceof Error ? reason.message : String(reason)); }); }}><ArrowDownToLine size={14} /> Update</button>}
                    <button className="secondary-button installed-switch" type="button" disabled={!desktop || isSwitchingVersion || isBulkUpdatingMods || isUpdatingAllMods} onClick={() => setSwitchTarget(mod)}><ArrowDownUp size={14} /> Switch version</button>
                    <button className="secondary-button installed-toggle" type="button" disabled={busyFile === mod.fileName || isBulkUpdatingMods || isUpdatingAllMods} onClick={() => void toggleMod(mod)}>{busyFile === mod.fileName ? <LoaderCircle className="spin" size={14} /> : mod.enabled ? "Disable" : "Enable"}</button>
                    <button className="icon-button installed-remove" type="button" disabled={busyFile === mod.fileName || isBulkUpdatingMods || isUpdatingAllMods} aria-label={`Remove ${mod.name}`} onClick={() => setRemoveTarget(mod)}><Trash2 size={16} /></button>
                  </article>; })}</div>}
                  </>}
          </div>
        )}
      </section>

      <section className="mod-privacy-note"><ShieldCheck size={17} /><span><strong>Provider requests are visible and on demand.</strong> Search and build requests go to the selected provider; project images load from its CDN. CurseForge keys stay in app memory for this session. Verified files go into this instance's mods folder; OpnLauncher does not run mods during installation.{horizonFabricMode && " On launch, Fabric and the selected Java mods execute in the isolated Java game process, not in Rust Horizon."}</span></section>

      {downloadProgress && <section className="mod-download-card">
        <div className="mod-download-heading"><span className="mod-download-icon"><LoaderCircle className={isInstalling ? "spin" : ""} size={17} /></span><span><strong>{downloadProgress.projectTitle}</strong><small>{downloadProgress.phase}{downloadProgress.currentFile ? ` · ${downloadProgress.currentFile}` : ""}</small></span>{isInstalling && <button className="secondary-button" type="button" onClick={() => void cancelInstall()}>Cancel</button>}</div>
        <div className="download-progress-track" role="progressbar" aria-label="Installing mod files" aria-valuemin={0} aria-valuemax={100} aria-valuenow={progressPercent}><span style={{ width: `${progressPercent}%` }} /></div>
        <div className="download-progress-meta"><span>{progressPercent}% · {downloadProgress.completedFiles} of {downloadProgress.totalFiles} files</span><span>{formatBytes(downloadProgress.downloadedBytes)}{downloadProgress.totalBytes ? ` / ${formatBytes(downloadProgress.totalBytes)}` : ""}</span></div>
        {installError && <div className="error-banner mod-error" role="alert"><CircleAlert size={15} /><span>{installError}</span></div>}
      </section>}

      {selectedProject && <div className="modal-backdrop mod-detail-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget && !isInstalling) setSelectedProject(null); }}>
        <section className="mod-detail-modal" role="dialog" aria-modal="true" aria-labelledby="mod-detail-title">
          <button className="icon-button mod-detail-close" type="button" aria-label="Close project details" onClick={() => !isInstalling && setSelectedProject(null)}><X size={18} /></button>
          <div className="mod-detail-gallery">
            {selectedProject.gallery[imageIndex]?.url ? <img className="mod-detail-cover" src={selectedProject.gallery[imageIndex].url} alt={selectedProject.gallery[imageIndex].title ?? `${selectedProject.title} screenshot`} onError={(event) => { event.currentTarget.style.display = "none"; }} /> : <div className="mod-detail-cover mod-cover-art"><span /><span /><span /></div>}
            <div className="mod-detail-gallery-shade" />
            <div className="mod-detail-source">{selectedProject.provider === "modrinth" ? "MODRINTH" : "CURSEFORGE"} <span>·</span> MOD</div>
            <div className="mod-detail-titleline"><span className="mod-project-icon large">{selectedProject.iconUrl ? <img src={selectedProject.iconUrl} alt="" onError={(event) => { event.currentTarget.style.display = "none"; }} /> : <PackageCheck size={24} />}</span><div><h2 id="mod-detail-title">{selectedProject.title}</h2><span>by {selectedProject.author}</span></div></div>
          </div>
          {selectedProject.gallery.length > 1 && <div className="mod-gallery-thumbnails">{selectedProject.gallery.slice(0, 8).map((image, index) => <button type="button" key={`${image.url}-${index}`} className={index === imageIndex ? "selected" : ""} onClick={() => setImageIndex(index)} aria-label={`Show screenshot ${index + 1}`}>{image.url && <img src={image.url} loading="lazy" alt="" />}</button>)}</div>}
          <div className="mod-detail-layout">
            <div className="mod-detail-main">
              <div className="mod-detail-stats"><span><Download size={14} /> {formatDownloads(selectedProject.downloads)} downloads</span><span><Layers3 size={14} /> Minecraft {instanceContext?.minecraftVersion ?? "version unavailable"}</span><span><span className={`loader-mark loader-${loaderFilter}`} /> {loaderName(loaderFilter)}</span></div>
              <div className="mod-detail-description"><p>{selectedProject.summary}</p><div className="mod-detail-tags">{selectedProject.categories.map((category) => <span key={category}>{category.replaceAll("_", " ")}</span>)}</div></div>
              {selectedProject.projectUrl && <a className="text-link mod-source-link" href={selectedProject.projectUrl} target="_blank" rel="noreferrer"><ExternalLink size={14} /> Open project page</a>}
            </div>
            <aside className="mod-version-panel">
              <div className="mod-version-heading"><div><span className="eyebrow">COMPATIBLE BUILDS</span><strong>Choose a version</strong></div><span className="mod-version-count">{projectVersions.length}</span></div>
              {isLoadingDetails ? <div className="mod-detail-loading"><LoaderCircle className="spin" size={17} /> Loading compatible versions…</div>
                : detailError ? <div className="mod-version-empty"><CircleAlert size={16} /><span>{detailError}</span></div>
                  : <div className="mod-version-list">{projectVersions.map((version) => <button type="button" key={version.id} className={`mod-version-option ${selectedVersionId === version.id ? "selected" : ""}`} onClick={() => setSelectedVersionId(version.id)}>
                    <span className="mod-version-check">{selectedVersionId === version.id ? <Check size={13} /> : <span />}</span><span className="mod-version-copy"><strong>{version.name}</strong><small>{version.versionNumber} · {version.releaseType} · {versionDate(version.datePublished)}</small><small>{version.files[0]?.name ?? "File name unavailable"}{version.files[0]?.size ? ` · ${formatBytes(version.files[0].size)}` : ""}</small></span>
                  </button>)}</div>}
              {selectedVersion && <div className="mod-version-dependencies"><span>Required dependencies</span><strong>{requiredDependencyCount === 0 ? "None listed" : `${requiredDependencyCount} project${requiredDependencyCount === 1 ? "" : "s"} · installed together`}</strong></div>}
              {!instanceContext?.loader && <div className="mod-version-warning"><CircleAlert size={15} /><span>Mod installation needs Fabric or Forge installed for this instance.</span><button type="button" className="text-link" onClick={onOpenInstances}>Configure instance <ArrowUpRight size={14} /></button></div>}
              {instanceContext?.loader && instanceContext.loader !== loaderFilter && <div className="mod-version-warning"><CircleAlert size={15} /><span>This list is filtered for {loaderName(loaderFilter)}. Your instance uses {loaderName(instanceContext.loader)}.</span></div>}
              {instanceContext?.sharedGameDirectory && <div className="mod-version-warning"><CircleAlert size={15} /><span>This mods folder is shared with another instance.</span></div>}
              {installError && <div className="error-banner mod-error" role="alert"><CircleAlert size={15} /><span>{installError}</span></div>}
              <button className="play-button mod-install-button" type="button" disabled={!canInstall || isLoadingDetails || isInstalling || selectedProjectIsInstalled} title={selectedProjectIsInstalled ? "This project is already installed in the selected instance." : undefined} onClick={() => setConfirmInstall(true)}>{isInstalling ? <LoaderCircle className="spin" size={16} /> : selectedProjectIsInstalled ? <Check size={16} /> : <Download size={16} />}{isInstalling ? "Installing…" : selectedProjectIsInstalled ? "Already installed" : `Install ${requiredDependencyCount ? `+ ${requiredDependencyCount} required` : "version"}`}</button>
              <small className="mod-install-boundary"><ShieldAlert size={13} /> Mod files contain executable code. Review the author and project before installing.</small>
            </aside>
          </div>
        </section>
      </div>}

      {confirmInstall && selectedProject && selectedVersion && <div className="modal-backdrop confirmation-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setConfirmInstall(false); }}>
        <section className="modal-card small-modal" role="alertdialog" aria-modal="true" aria-labelledby="mod-install-confirm-title"><div className="modal-header"><div><p className="eyebrow"><Download size={14} /> INSTALL MOD FILES</p><h2 id="mod-install-confirm-title">Add {selectedProject.title}?</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setConfirmInstall(false)}><X size={18} /></button></div>
          <p className="modal-copy">This installs <strong>{selectedVersion.versionNumber}</strong> for Minecraft {instanceContext?.minecraftVersion} · {loaderName(instanceContext?.loader)} into this instance's mods folder. Required dependencies listed by the provider are included. Existing files are never replaced.</p>
          <p className="mod-confirm-security"><ShieldAlert size={15} /> This is third-party executable code. Only install projects you trust.</p>
          <div className="dialog-actions"><button className="secondary-button" type="button" onClick={() => setConfirmInstall(false)}>Cancel</button><button className="play-button" type="button" onClick={() => void startInstall()}><Download size={15} /> Install to instance</button></div>
        </section>
      </div>}

      {removeTarget && <div className="modal-backdrop confirmation-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setRemoveTarget(null); }}>
        <section className="modal-card small-modal" role="alertdialog" aria-modal="true" aria-labelledby="mod-remove-title"><div className="modal-header"><div><p className="eyebrow danger-eyebrow"><Trash2 size={14} /> REMOVE MOD FILE</p><h2 id="mod-remove-title">Remove {removeTarget.name}?</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setRemoveTarget(null)}><X size={18} /></button></div><p className="modal-copy">This permanently removes <code>{removeTarget.fileName}</code> from this instance's mods folder. Other instance files are not changed.</p><div className="dialog-actions"><button className="secondary-button" type="button" onClick={() => setRemoveTarget(null)}>Keep file</button><button className="danger-button" type="button" onClick={() => void removeMod()}><Trash2 size={15} /> Remove mod file</button></div></section>
      </div>}

      {bulkRemoveTarget && <div className="modal-backdrop confirmation-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget && !isRemovingMods) setBulkRemoveTarget(null); }}>
        <section className="modal-card small-modal" role="alertdialog" aria-modal="true" aria-labelledby="mod-bulk-remove-title"><div className="modal-header"><div><p className="eyebrow danger-eyebrow"><Trash2 size={14} /> REMOVE MOD FILES</p><h2 id="mod-bulk-remove-title">Delete {bulkRemoveTarget.length === installedMods.length ? "all" : "selected"} mods?</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setBulkRemoveTarget(null)} disabled={isRemovingMods}><X size={18} /></button></div><p className="modal-copy">This permanently removes {bulkRemoveTarget.length} mod file{bulkRemoveTarget.length === 1 ? "" : "s"} from this instance's mods folder. Other instance files, saves, and settings are not changed.</p>{bulkRemoveError && <div className="error-banner mod-error bulk-remove-error" role="alert"><CircleAlert size={15} /><span>{bulkRemoveError}</span></div>}<div className="dialog-actions"><button className="secondary-button" type="button" onClick={() => setBulkRemoveTarget(null)} disabled={isRemovingMods}>Keep files</button><button className="danger-button" type="button" onClick={() => void removeBulkTarget()} disabled={isRemovingMods}>{isRemovingMods ? <LoaderCircle className="spin" size={15} /> : <Trash2 size={15} />}{isRemovingMods ? "Deleting…" : `Delete ${bulkRemoveTarget.length} mod${bulkRemoveTarget.length === 1 ? "" : "s"}`}</button></div></section>
      </div>}

      {switchTarget && selectedInstanceId && <ModVersionSwitchDialog mod={switchTarget} instanceId={selectedInstanceId} context={instanceContext} initialProvider={provider} curseForgeKey={curseForgeKey} switching={isSwitchingVersion} onClose={() => setSwitchTarget(null)} onRequestCurseForgeKey={() => { setSwitchTarget(null); setProvider("curseforge"); setKeyDraft(curseForgeKey); setShowKeyInput(true); }} onSwitch={async (mod, source, version) => { await switchInstalledModVersion(mod, source, version); }} />}
    </>
  );
}
