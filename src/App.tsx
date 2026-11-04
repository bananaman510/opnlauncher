import { useEffect, useMemo, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import packageJson from "../package.json";
import { DownloadsPage } from "./pages/DownloadsPage";
import { PlaytimeCard } from "./components/PlaytimeCard";
import { HorizonModsPage } from "./pages/HorizonModsPage";
import { HorizonSettingsPage } from "./pages/HorizonSettingsPage";
import { InstanceSetupPanel, type InstanceSetupSelection } from "./pages/InstanceSetupPanel";
import { ModsPage } from "./pages/ModsPage";
import { SettingsPage } from "./pages/SettingsPage";
import { VersionsPage } from "./pages/VersionsPage";
import {
  ArrowRight,
  ArrowDownToLine,
  Boxes,
  Check,
  ChevronDown,
  CircleAlert,
  CircleCheck,
  Clock3,
  Cpu,
  ExternalLink,
  FolderOpen,
  Gamepad2,
  HardDrive,
  House,
  Info,
  Layers3,
  LayoutGrid,
  List,
  LogOut,
  LoaderCircle,
  MonitorCog,
  PackageOpen,
  Play,
  Plus,
  Search,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Star,
  TerminalSquare,
  UsersRound,
  Wrench,
  X,
  Zap,
} from "lucide-react";
import { formatRelativeTime, javaLabel } from "./format";
import { filterSortInstances, type InstanceEngineFilter, type InstanceFilter, type InstanceSort } from "./libraryFilters";
import type {
  Instance,
  HorizonClientInfo,
  HorizonEngineStatus,
  JavaRuntime,
  LaunchExitEvent,
  LaunchLogEvent,
  LaunchStarted,
  MinecraftInstallation,
  MicrosoftAccount,
  OfflineProfile,
  DownloadProgress,
  RemoteMinecraftVersion,
} from "./types";

type ThemeMode = "dark" | "light";
type AccentColor = "lime" | "mint" | "sky" | "violet";
type InstanceView = "grid" | "list";

const navigation = [
  { id: "home", label: "Overview", icon: House, group: "PLAY" },
  { id: "instances", label: "Instances", icon: Boxes, group: "LIBRARY" },
  { id: "versions", label: "Versions", icon: Layers3, group: "LIBRARY" },
  { id: "mods", label: "Mods", icon: PackageOpen, group: "LIBRARY" },
  { id: "horizon", label: "Horizon", icon: Sparkles, group: "LIBRARY" },
  { id: "modpacks", label: "Modpacks", icon: FolderOpen, group: "LIBRARY" },
  { id: "accounts", label: "Accounts", icon: UsersRound, group: "TOOLS" },
  { id: "downloads", label: "Downloads", icon: ArrowDownToLine, group: "TOOLS" },
  { id: "settings", label: "Settings", icon: Settings2, group: "TOOLS" },
] as const;

const navigationGroups = ["PLAY", "LIBRARY", "TOOLS"] as const;
const accentColors: { id: AccentColor; label: string }[] = [
  { id: "lime", label: "Citrus" },
  { id: "mint", label: "Mint" },
  { id: "sky", label: "Sky" },
  { id: "violet", label: "Violet" },
];

function readStoredValue(key: string): string | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage.getItem(`opnlauncher.${key}`);
  } catch {
    return null;
  }
}

function readStoredIds(key: string): string[] {
  try {
    const parsed: unknown = JSON.parse(readStoredValue(key) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((value): value is string => typeof value === "string") : [];
  } catch {
    return [];
  }
}

function readStoredPaths(key: string): string[] {
  try {
    const parsed: unknown = JSON.parse(readStoredValue(key) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((value): value is string => typeof value === "string" && value.trim().length > 0) : [];
  } catch {
    return [];
  }
}

interface HorizonWindowSettings { width: number; height: number; fullscreen: boolean }

function readHorizonWindowSettings(): Record<string, HorizonWindowSettings> {
  try {
    const parsed: unknown = JSON.parse(readStoredValue("horizon-window-settings") ?? "{}");
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    return Object.fromEntries(Object.entries(parsed).flatMap(([id, item]) => {
      if (!item || typeof item !== "object") return [];
      const value = item as Partial<HorizonWindowSettings>;
      if (!Number.isInteger(value.width) || !Number.isInteger(value.height) || typeof value.fullscreen !== "boolean") return [];
      return [[id, { width: value.width as number, height: value.height as number, fullscreen: value.fullscreen }]];
    }));
  } catch {
    return {};
  }
}

async function discoverJavaRuntimes(): Promise<JavaRuntime[]> {
  const systemRuntimes = await invoke<JavaRuntime[]>("list_java_runtimes");
  const customRuntimes = await Promise.all(readStoredPaths("custom-java-paths").map(async (path) => {
    try {
      return await invoke<JavaRuntime>("inspect_java_runtime", { path });
    } catch {
      return null;
    }
  }));
  const byPath = new Map<string, JavaRuntime>();
  [...systemRuntimes, ...customRuntimes].forEach((runtime) => { if (runtime) byPath.set(runtime.path, runtime); });
  return [...byPath.values()];
}

function writeStoredValue(key: string, value: string): void {
  try {
    window.localStorage.setItem(`opnlauncher.${key}`, value);
  } catch {
    // Preferences are optional; the launcher remains usable if storage is unavailable.
  }
}

function removeStoredValue(key: string): void {
  try {
    window.localStorage.removeItem(`opnlauncher.${key}`);
  } catch {
    // Preferences are optional; the launcher remains usable if storage is unavailable.
  }
}

function formatMemory(memoryMb: number): string {
  const gigabytes = memoryMb / 1024;
  return `${Number.isInteger(gigabytes) ? gigabytes : gigabytes.toFixed(1)} GB`;
}

const appVersion = packageJson.version;

function App() {
  const desktop = isTauri();
  const [activePage, setActivePage] = useState<string>("home");
  const [instances, setInstances] = useState<Instance[]>([]);
  const [installations, setInstallations] = useState<MinecraftInstallation[]>([]);
  const [runtimes, setRuntimes] = useState<JavaRuntime[]>([]);
  const [profiles, setProfiles] = useState<OfflineProfile[]>([]);
  const [microsoftAccounts, setMicrosoftAccounts] = useState<MicrosoftAccount[]>([]);
  const [microsoftClientId, setMicrosoftClientId] = useState(() => readStoredValue("microsoft-client-id") ?? "");
  const [microsoftSignInBusy, setMicrosoftSignInBusy] = useState(false);
  const [accountError, setAccountError] = useState("");
  const [curseForgeApiKey, setCurseForgeApiKey] = useState("");
  const [selectedInstanceId, setSelectedInstanceId] = useState<string | null>(() => readStoredValue("selected-instance"));
  const [selectedProfileId, setSelectedProfileId] = useState(() => readStoredValue("selected-profile") ?? "");
  const [favoriteIds, setFavoriteIds] = useState<string[]>(() => readStoredIds("favorite-instances"));
  const [theme, setTheme] = useState<ThemeMode>(() => readStoredValue("theme") === "light" ? "light" : "dark");
  const [accent, setAccent] = useState<AccentColor>(() => {
    const stored = readStoredValue("accent");
    return accentColors.some((color) => color.id === stored) ? stored as AccentColor : "lime";
  });
  const [defaultJavaPath, setDefaultJavaPath] = useState(() => readStoredValue("default-java-path") ?? "");
  const [defaultMemoryMb, setDefaultMemoryMb] = useState(() => {
    const stored = Number(readStoredValue("default-memory-mb"));
    return Number.isFinite(stored) && stored >= 1024 && stored <= 16384 ? stored : 2048;
  });
  const [horizonPrepareJavaPath, setHorizonPrepareJavaPath] = useState(() => readStoredValue("horizon-prepare-java-path") ?? "");
  const [horizonResourcesPath, setHorizonResourcesPath] = useState("");
  const [horizonResourcesReady, setHorizonResourcesReady] = useState(false);
  const [horizonMinecraftSourcePath, setHorizonMinecraftSourcePath] = useState(() => readStoredValue("horizon-minecraft-source-path") ?? "");
  const [horizonScanning, setHorizonScanning] = useState(false);
  const [horizonClientInfo, setHorizonClientInfo] = useState<HorizonClientInfo | null>(null);
  const [horizonPreparing, setHorizonPreparing] = useState(false);
  const [horizonSettingsError, setHorizonSettingsError] = useState("");
  const [horizonWindowSettings, setHorizonWindowSettings] = useState<Record<string, HorizonWindowSettings>>(readHorizonWindowSettings);
  const [newInstanceEngine, setNewInstanceEngine] = useState<"minecraft" | "horizon">("minecraft");
  const [selectedJavaPath, setSelectedJavaPath] = useState("");
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [showInstanceSetup, setShowInstanceSetup] = useState(false);
  const [showVersionInstallDialog, setShowVersionInstallDialog] = useState(false);
  const [showDetectedInstallDialog, setShowDetectedInstallDialog] = useState(false);
  const [detectedInstallTarget, setDetectedInstallTarget] = useState<{ installation: MinecraftInstallation; versionId: string } | null>(null);
  const [detectedInstallName, setDetectedInstallName] = useState("");
  const [scanningInstallations, setScanningInstallations] = useState(false);
  const [showProfileForm, setShowProfileForm] = useState(false);
  const [instanceName, setInstanceName] = useState("");
  const [versionInstanceName, setVersionInstanceName] = useState("");
  const [profileName, setProfileName] = useState("");
  const [installPath, setInstallPath] = useState("");
  const [inspectedInstall, setInspectedInstall] = useState<MinecraftInstallation | null>(null);
  const [versionChoice, setVersionChoice] = useState("");
  const [memoryChoice, setMemoryChoice] = useState(2048);
  const [instanceSearch, setInstanceSearch] = useState("");
  const [instanceFilter, setInstanceFilter] = useState<InstanceFilter>("all");
  const [instanceEngineFilter, setInstanceEngineFilter] = useState<InstanceEngineFilter>(() => {
    const stored = readStoredValue("instance-engine-filter");
    return stored === "minecraft" || stored === "horizon" ? stored : "all";
  });
  const [instanceSort, setInstanceSort] = useState<InstanceSort>(() => {
    const stored = readStoredValue("instance-sort");
    return stored === "name" || stored === "version" ? stored : "recent";
  });
  const [instanceView, setInstanceView] = useState<InstanceView>("grid");
  const [isBusy, setIsBusy] = useState(false);
  const [isSavingSettings, setIsSavingSettings] = useState(false);
  const [busyMessage, setBusyMessage] = useState("");
  const [error, setError] = useState("");
  const [logs, setLogs] = useState<string[]>([]);
  const [launchState, setLaunchState] = useState<"idle" | "running" | "exited">("idle");
  const [logPath, setLogPath] = useState("");
  const [officialVersions, setOfficialVersions] = useState<RemoteMinecraftVersion[]>([]);
  const [versionsLoading, setVersionsLoading] = useState(false);
  const [versionsError, setVersionsError] = useState("");
  const [selectedRemoteVersion, setSelectedRemoteVersion] = useState<RemoteMinecraftVersion | null>(null);
  const [installError, setInstallError] = useState("");
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgress | null>(null);
  const [activeDownloadId, setActiveDownloadId] = useState<string | null>(null);
  const [isInstallingVersion, setIsInstallingVersion] = useState(false);
  const [showRenameDialog, setShowRenameDialog] = useState(false);
  const [showForgetDialog, setShowForgetDialog] = useState(false);
  const [renameValue, setRenameValue] = useState("");
  const activeDownloadIdRef = useRef<string | null>(null);
  const selectedInstanceIdRef = useRef(selectedInstanceId);
  const globalSearchRef = useRef<HTMLInputElement>(null);
  selectedInstanceIdRef.current = selectedInstanceId;

  const selectedInstance = instances.find((instance) => instance.id === selectedInstanceId) ?? null;
  const selectedHorizonWindow = selectedInstance?.engine === "horizon"
    ? horizonWindowSettings[selectedInstance.id] ?? { width: 1280, height: 720, fullscreen: false }
    : { width: 1280, height: 720, fullscreen: false };
  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId)
    ?? microsoftAccounts.find((account) => account.id === selectedProfileId)
    ?? null;
  const selectedProfileIsMicrosoft = selectedProfileId.startsWith("microsoft:");
  const selectedJava = runtimes.find((runtime) => runtime.path === selectedJavaPath)
    ?? (selectedJavaPath === "" ? runtimes[0] ?? null : null);
  const downloadPercent = downloadProgress
    ? downloadProgress.totalBytes > 0
      ? Math.min(100, Math.round((downloadProgress.downloadedBytes / downloadProgress.totalBytes) * 100))
      : downloadProgress.totalFiles > 0
        ? Math.round((downloadProgress.completedFiles / downloadProgress.totalFiles) * 100)
        : 0
    : 0;
  const latestLogLines = useMemo(() => logs.slice(-100), [logs]);
  const visibleInstances = useMemo(() => filterSortInstances(instances, {
    query: instanceSearch,
    filter: instanceFilter,
    engine: instanceEngineFilter,
    sort: instanceSort,
    favorites: favoriteIds,
  }), [favoriteIds, instanceEngineFilter, instanceFilter, instanceSearch, instanceSort, instances]);
  const recentInstances = useMemo(
    () => [...instances].sort((left, right) => (right.lastPlayedAt ?? right.createdAt) - (left.lastPlayedAt ?? left.createdAt)).slice(0, 4),
    [instances],
  );

  useEffect(() => {
    if (!desktop) return;
    let cancelled = false;
    let unlistenLog: UnlistenFn | undefined;
    let unlistenExit: UnlistenFn | undefined;
    let unlistenDownload: UnlistenFn | undefined;

    const load = async () => {
      setIsBusy(true);
      setBusyMessage("Checking this device");
      try {
        const [savedInstances, foundInstallations, foundRuntimes] = await Promise.all([
          invoke<Instance[]>("list_instances"),
          invoke<MinecraftInstallation[]>("scan_installations"),
          discoverJavaRuntimes(),
        ]);
        if (cancelled) return;
        const detectedInstallations = [...foundInstallations];
        let savedMinecraftSourceIsValid = Boolean(horizonMinecraftSourcePath);
        const foundSavedSource = detectedInstallations.find((installation) => installation.root === horizonMinecraftSourcePath);
        if (horizonMinecraftSourcePath && foundSavedSource && !foundSavedSource.versions.some((version) => version.id === "26.3")) {
          setHorizonMinecraftSourcePath("");
          savedMinecraftSourceIsValid = false;
        } else if (horizonMinecraftSourcePath && !foundSavedSource) {
          try {
            const configured = await invoke<MinecraftInstallation>("inspect_installation", { path: horizonMinecraftSourcePath });
            if (configured.versions.some((version) => version.id === "26.3")) detectedInstallations.push(configured);
            else {
              setHorizonMinecraftSourcePath("");
              savedMinecraftSourceIsValid = false;
            }
          } catch {
            setHorizonMinecraftSourcePath("");
            savedMinecraftSourceIsValid = false;
          }
        }
        if (cancelled) return;
        setInstances(savedInstances);
        setInstallations(detectedInstallations);
        setRuntimes(foundRuntimes);
        try {
          const status = await invoke<HorizonEngineStatus>("inspect_horizon_engine");
          setHorizonClientInfo(status.client);
          setHorizonResourcesPath(status.resourcesPath);
          setHorizonResourcesReady(status.resourcesReady);
        } catch (reason) {
          setHorizonClientInfo(null);
          setHorizonResourcesReady(false);
          setHorizonSettingsError(String(reason));
        }
        if (!savedMinecraftSourceIsValid) {
          const matching = detectedInstallations.find((installation) => installation.versions.some((version) => version.id === "26.3"));
          if (matching) setHorizonMinecraftSourcePath(matching.root);
        }
        setSelectedInstanceId((current) => current && savedInstances.some((instance) => instance.id === current) ? current : savedInstances[0]?.id ?? null);
        if (savedInstances.length > 0) setMemoryChoice(savedInstances[0].memoryMb);
        setFavoriteIds((current) => current.filter((id) => savedInstances.some((instance) => instance.id === id)));
        if (foundRuntimes.length > 0) setSelectedJavaPath(foundRuntimes[0].path);
        const [savedProfiles, savedAccounts] = await Promise.all([
          invoke<OfflineProfile[]>("list_offline_profiles"),
          invoke<MicrosoftAccount[]>("list_microsoft_accounts"),
        ]);
        if (!cancelled) {
          setProfiles(savedProfiles);
          setMicrosoftAccounts(savedAccounts);
          const savedAccountIds = [...savedProfiles, ...savedAccounts].map((account) => account.id);
          setSelectedProfileId((current) => savedAccountIds.includes(current) ? current : savedProfiles[0]?.id ?? savedAccounts[0]?.id ?? "");
        }
      } catch (reason) {
        if (!cancelled) setError(String(reason));
      } finally {
        if (!cancelled) setIsBusy(false);
      }
    };

    void load();
    void listen<LaunchLogEvent>("launch-log", (event) => {
      if (event.payload.instanceId === selectedInstanceIdRef.current) {
        setLogs((current) => [...current, `[${event.payload.stream}] ${event.payload.text}`]);
      }
    }).then((dispose) => {
      unlistenLog = dispose;
    });
    void listen<LaunchExitEvent>("launch-exit", (event) => {
      if (event.payload.instanceId === selectedInstanceIdRef.current) {
        setLaunchState("exited");
        setLogs((current) => [
          ...current,
          event.payload.success
            ? `Minecraft closed successfully (exit ${event.payload.code ?? "unknown"}).`
            : `Minecraft exited with code ${event.payload.code ?? "unknown"}.`,
        ]);
        void refreshInstances();
      }
    }).then((dispose) => {
      unlistenExit = dispose;
    });
    void listen<DownloadProgress>("download-progress", (event) => {
      if (event.payload.downloadId === activeDownloadIdRef.current) {
        setDownloadProgress(event.payload);
      }
    }).then((dispose) => {
      unlistenDownload = dispose;
    });
    return () => {
      cancelled = true;
      unlistenLog?.();
      unlistenExit?.();
      unlistenDownload?.();
    };
    // The selected instance ref keeps event subscriptions stable while selection changes.
  }, [desktop]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.accent = accent;
    writeStoredValue("theme", theme);
    writeStoredValue("accent", accent);
  }, [accent, theme]);

  useEffect(() => {
    if (selectedInstanceId) writeStoredValue("selected-instance", selectedInstanceId);
    else removeStoredValue("selected-instance");
  }, [selectedInstanceId]);

  useEffect(() => {
    if (selectedProfileId) writeStoredValue("selected-profile", selectedProfileId);
    else removeStoredValue("selected-profile");
  }, [selectedProfileId]);

  useEffect(() => {
    writeStoredValue("favorite-instances", JSON.stringify(favoriteIds));
  }, [favoriteIds]);

  useEffect(() => { writeStoredValue("instance-engine-filter", instanceEngineFilter); }, [instanceEngineFilter]);
  useEffect(() => { writeStoredValue("instance-sort", instanceSort); }, [instanceSort]);

  useEffect(() => {
    writeStoredValue("default-java-path", defaultJavaPath);
  }, [defaultJavaPath]);

  useEffect(() => {
    writeStoredValue("default-memory-mb", String(defaultMemoryMb));
  }, [defaultMemoryMb]);

  useEffect(() => { writeStoredValue("horizon-prepare-java-path", horizonPrepareJavaPath); }, [horizonPrepareJavaPath]);
  useEffect(() => { writeStoredValue("horizon-minecraft-source-path", horizonMinecraftSourcePath); }, [horizonMinecraftSourcePath]);
  useEffect(() => { writeStoredValue("horizon-window-settings", JSON.stringify(horizonWindowSettings)); }, [horizonWindowSettings]);

  useEffect(() => {
    if (!horizonPrepareJavaPath) {
      const jdk25 = runtimes.find((runtime) => runtime.majorVersion === 25);
      if (jdk25) setHorizonPrepareJavaPath(jdk25.path);
    }
  }, [horizonPrepareJavaPath, runtimes]);

  useEffect(() => {
    function handleShortcut(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "k") {
        event.preventDefault();
        setActivePage("instances");
        window.requestAnimationFrame(() => globalSearchRef.current?.focus());
      }
    }
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, []);

  useEffect(() => {
    if (selectedInstance) {
      setMemoryChoice(selectedInstance.memoryMb);
      setSelectedJavaPath(selectedInstance.javaPath ?? defaultJavaPath);
    }
  }, [selectedInstanceId, instances, defaultJavaPath]);

  useEffect(() => {
    if (desktop && activePage === "versions" && officialVersions.length === 0) {
      void loadOfficialVersions();
    }
  }, [desktop, activePage]);

  async function refreshInstances() {
    const savedInstances = await invoke<Instance[]>("list_instances");
    setInstances(savedInstances);
  }

  async function refreshInstallations() {
    setScanningInstallations(true);
    try {
      const found = await invoke<MinecraftInstallation[]>("scan_installations");
      const detected = [...found];
      const selectedSource = detected.find((installation) => installation.root === horizonMinecraftSourcePath);
      if (horizonMinecraftSourcePath && selectedSource && !selectedSource.versions.some((version) => version.id === "26.3")) {
        setHorizonMinecraftSourcePath("");
      } else if (horizonMinecraftSourcePath && !selectedSource) {
        try {
          const configured = await invoke<MinecraftInstallation>("inspect_installation", { path: horizonMinecraftSourcePath });
          if (configured.versions.some((version) => version.id === "26.3")) detected.push(configured);
          else setHorizonMinecraftSourcePath("");
        } catch {
          setHorizonMinecraftSourcePath("");
        }
      }
      setInstallations(detected.sort((a, b) => a.root.localeCompare(b.root)));
    } catch (reason) {
      setError(`Could not scan Minecraft installations: ${String(reason)}`);
    } finally {
      setScanningInstallations(false);
    }
  }

  async function refreshHorizonDiscovery() {
    setHorizonScanning(true);
    setHorizonSettingsError("");
    try {
      const [foundInstallations, foundRuntimes] = await Promise.all([
        invoke<MinecraftInstallation[]>("scan_installations"),
        discoverJavaRuntimes(),
      ]);
      const detectedInstallations = [...foundInstallations];
      let savedMinecraftSourceIsValid = Boolean(horizonMinecraftSourcePath);
      const foundSavedSource = detectedInstallations.find((installation) => installation.root === horizonMinecraftSourcePath);
      if (horizonMinecraftSourcePath && foundSavedSource && !foundSavedSource.versions.some((version) => version.id === "26.3")) {
        setHorizonMinecraftSourcePath("");
        savedMinecraftSourceIsValid = false;
      } else if (horizonMinecraftSourcePath && !foundSavedSource) {
        try {
          const configured = await invoke<MinecraftInstallation>("inspect_installation", { path: horizonMinecraftSourcePath });
          if (configured.versions.some((version) => version.id === "26.3")) detectedInstallations.push(configured);
          else {
            setHorizonMinecraftSourcePath("");
            savedMinecraftSourceIsValid = false;
          }
        } catch {
          setHorizonMinecraftSourcePath("");
          savedMinecraftSourceIsValid = false;
        }
      }
      setInstallations(detectedInstallations.sort((a, b) => a.root.localeCompare(b.root)));
      setRuntimes(foundRuntimes);
      const status = await invoke<HorizonEngineStatus>("inspect_horizon_engine");
      setHorizonClientInfo(status.client);
      setHorizonResourcesPath(status.resourcesPath);
      setHorizonResourcesReady(status.resourcesReady);
      if (!savedMinecraftSourceIsValid) {
        const matching = detectedInstallations.find((installation) => installation.versions.some((version) => version.id === "26.3"));
        if (matching) setHorizonMinecraftSourcePath(matching.root);
      }
    } catch (reason) {
      setHorizonSettingsError(String(reason));
    } finally {
      setHorizonScanning(false);
    }
  }

  async function addHorizonMinecraftFolder(path: string) {
    const installation = await invoke<MinecraftInstallation>("inspect_installation", { path: path.trim() });
    if (!installation.versions.some((version) => version.id === "26.3")) {
      throw new Error("This folder does not contain Minecraft 26.3. Choose a folder with the 26.3 version installed.");
    }
    setInstallations((current) => [...current.filter((entry) => entry.root !== installation.root), installation].sort((a, b) => a.root.localeCompare(b.root)));
    setHorizonMinecraftSourcePath(installation.root);
  }

  async function refreshRuntimes() {
    const found = await discoverJavaRuntimes();
    setRuntimes(found);
  }

  async function addJavaRuntime(path: string) {
    const runtime = await invoke<JavaRuntime>("inspect_java_runtime", { path });
    setRuntimes((current) => [...current.filter((entry) => entry.path !== runtime.path), runtime]);
    const paths = [...new Set([...readStoredPaths("custom-java-paths"), runtime.path])];
    writeStoredValue("custom-java-paths", JSON.stringify(paths));
  }

  async function applyInstanceDefaults(instance: Instance): Promise<Instance> {
    const changes: Partial<Instance> = {};
    if (defaultMemoryMb !== instance.memoryMb) {
      await invoke("set_instance_memory", { instanceId: instance.id, memoryMb: defaultMemoryMb });
      changes.memoryMb = defaultMemoryMb;
    }
    if (defaultJavaPath && runtimes.some((runtime) => runtime.path === defaultJavaPath) && defaultJavaPath !== instance.javaPath) {
      await invoke("set_instance_java", { instanceId: instance.id, javaPath: defaultJavaPath });
      changes.javaPath = defaultJavaPath;
    }
    return { ...instance, ...changes };
  }

  async function loadOfficialVersions() {
    setVersionsLoading(true);
    setVersionsError("");
    try {
      const found = await invoke<RemoteMinecraftVersion[]>("list_official_versions");
      setOfficialVersions(found);
    } catch (reason) {
      setVersionsError(String(reason));
    } finally {
      setVersionsLoading(false);
    }
  }

  function chooseRemoteVersion(version: RemoteMinecraftVersion) {
    setSelectedRemoteVersion(version);
    setVersionInstanceName(`Minecraft ${version.id}`);
    setInstallError("");
    setShowVersionInstallDialog(true);
  }

  async function installRemoteVersion(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedRemoteVersion || !versionInstanceName.trim() || isInstallingVersion) return;
    const downloadId = crypto.randomUUID();
    activeDownloadIdRef.current = downloadId;
    setActiveDownloadId(downloadId);
    setDownloadProgress({
      downloadId,
      versionId: selectedRemoteVersion.id,
      phase: "Preparing version metadata",
      currentFile: "",
      completedFiles: 0,
      totalFiles: 0,
      downloadedBytes: 0,
      totalBytes: 0,
    });
    setInstallError("");
    setIsInstallingVersion(true);
    try {
      const installed = await invoke<Instance>("install_minecraft_version", {
        name: versionInstanceName.trim(),
        versionId: selectedRemoteVersion.id,
        downloadId,
      });
      const created = await applyInstanceDefaults(installed);
      setInstances((current) => [created, ...current]);
      setSelectedInstanceId(created.id);
      setMemoryChoice(created.memoryMb);
      setDownloadProgress((current) => current ? { ...current, phase: "Installation complete" } : current);
      setShowVersionInstallDialog(false);
      setSelectedRemoteVersion(null);
      setActivePage("instances");
    } catch (reason) {
      const message = String(reason);
      setInstallError(message);
      setDownloadProgress((current) => current ? {
        ...current,
        phase: message.toLowerCase().includes("cancel") ? "Installation cancelled" : "Installation failed",
      } : current);
    } finally {
      activeDownloadIdRef.current = null;
      setActiveDownloadId(null);
      setIsInstallingVersion(false);
    }
  }

  async function cancelDownload() {
    if (!activeDownloadId) return;
    try {
      await invoke("cancel_download", { downloadId: activeDownloadId });
    } catch (reason) {
      setInstallError(String(reason));
    }
  }

  async function installInstanceSetup(selection: InstanceSetupSelection) {
    const downloadId = crypto.randomUUID();
    activeDownloadIdRef.current = downloadId;
    setActiveDownloadId(downloadId);
    setDownloadProgress({
      downloadId,
      versionId: selection.minecraftVersion,
      phase: "Preparing instance setup",
      currentFile: "",
      completedFiles: 0,
      totalFiles: 0,
      downloadedBytes: 0,
      totalBytes: 0,
    });
    setIsInstallingVersion(true);
    try {
      const updated = await invoke<Instance>("install_instance_setup", {
        instanceId: selection.instanceId,
        minecraftVersion: selection.minecraftVersion,
        loader: selection.loader,
        loaderVersion: selection.loaderVersion,
        javaPath: selection.javaPath,
        downloadId,
      });
      setInstances((current) => current.map((instance) => instance.id === updated.id ? updated : instance));
      setSelectedInstanceId(updated.id);
      setMemoryChoice(updated.memoryMb);
      setDownloadProgress((current) => current ? { ...current, phase: "Installation complete" } : current);
    } catch (reason) {
      const message = String(reason);
      setDownloadProgress((current) => current ? {
        ...current,
        phase: message.toLowerCase().includes("cancel") ? "Installation cancelled" : "Installation failed",
      } : current);
      throw reason;
    } finally {
      activeDownloadIdRef.current = null;
      setActiveDownloadId(null);
      setIsInstallingVersion(false);
    }
  }

  function requestAddDetectedInstallation(installation: MinecraftInstallation, versionId: string) {
    const suggested = `${versionId} · ${installation.root.split(/[\/]/).filter(Boolean).at(-1) ?? "Minecraft"}`;
    setDetectedInstallTarget({ installation, versionId });
    setDetectedInstallName(suggested.slice(0, 48));
    setShowDetectedInstallDialog(true);
  }

  async function addDetectedInstallation() {
    if (!detectedInstallTarget || !detectedInstallName.trim()) return;
    const { installation, versionId } = detectedInstallTarget;
    setError("");
    setIsBusy(true);
    setBusyMessage("Adding local installation");
    try {
      const created = await invoke<Instance>("register_installation", {
        name: detectedInstallName.trim(),
        root: installation.root,
        versionId,
      });
      setInstances((current) => [created, ...current]);
      setSelectedInstanceId(created.id);
      setShowDetectedInstallDialog(false);
      setShowAddDialog(false);
      setDetectedInstallTarget(null);
      setActivePage("instances");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setIsBusy(false);
    }
  }

  async function inspectCustomInstallation() {
    setError("");
    setIsBusy(true);
    try {
      const inspected = await invoke<MinecraftInstallation>("inspect_installation", { path: installPath });
      setInspectedInstall(inspected);
      setVersionChoice(inspected.versions[0]?.id ?? "");
    } catch (reason) {
      setInspectedInstall(null);
      setError(String(reason));
    } finally {
      setIsBusy(false);
    }
  }

  async function addInspectedInstallation() {
    if (!inspectedInstall || !versionChoice) return;
    requestAddDetectedInstallation(inspectedInstall, versionChoice);
  }

  async function createBlankInstance(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!instanceName.trim()) return;
    setError("");
    setIsBusy(true);
    setBusyMessage(newInstanceEngine === "horizon" ? "Creating Horizon profile" : "Creating instance");
    try {
      const empty = await invoke<Instance>(newInstanceEngine === "horizon" ? "create_horizon_instance" : "create_instance", { name: instanceName.trim() });
      const created = newInstanceEngine === "horizon" ? empty : await applyInstanceDefaults(empty);
      setInstances((current) => [created, ...current]);
      setSelectedInstanceId(created.id);
      setInstanceName("");
      setNewInstanceEngine("minecraft");
      setShowAddDialog(false);
      setActivePage("instances");
      if (created.engine === "minecraft") setShowInstanceSetup(true);
      else setActivePage("horizon");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setIsBusy(false);
    }
  }

  async function createProfile(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!profileName.trim()) return;
    setError("");
    try {
      const created = await invoke<OfflineProfile>("create_offline_profile", { name: profileName });
      setProfiles((current) => [...current, created]);
      setSelectedProfileId(created.id);
      setProfileName("");
      setShowProfileForm(false);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function signInMicrosoft() {
    if (!desktop || microsoftSignInBusy || !microsoftClientId.trim()) return;
    setMicrosoftSignInBusy(true);
    setAccountError("");
    writeStoredValue("microsoft-client-id", microsoftClientId.trim());
    try {
      const account = await invoke<MicrosoftAccount>("add_microsoft_account", { clientId: microsoftClientId.trim() });
      setMicrosoftAccounts((current) => [...current.filter((saved) => saved.id !== account.id), account]);
      setSelectedProfileId(account.id);
    } catch (reason) {
      setAccountError(String(reason));
    } finally {
      setMicrosoftSignInBusy(false);
    }
  }

  async function signOutMicrosoft(account: MicrosoftAccount) {
    setAccountError("");
    try {
      await invoke("remove_microsoft_account", { accountId: account.id });
      setMicrosoftAccounts((current) => current.filter((saved) => saved.id !== account.id));
      if (selectedProfileId === account.id) setSelectedProfileId(profiles[0]?.id ?? "");
    } catch (reason) {
      setAccountError(String(reason));
    }
  }

  async function saveInstanceSettings() {
    if (!selectedInstance) return;
    setError("");
    setIsSavingSettings(true);
    try {
      await invoke("set_instance_memory", {
        instanceId: selectedInstance.id,
        memoryMb: memoryChoice,
      });
      await invoke("set_instance_java", {
        instanceId: selectedInstance.id,
        javaPath: selectedJavaPath || null,
      });
      await refreshInstances();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setIsSavingSettings(false);
    }
  }

  async function renameSelectedInstance(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedInstance) return;
    const name = renameValue.trim();
    if (!name || name === selectedInstance.name) {
      setShowRenameDialog(false);
      return;
    }
    setError("");
    try {
      await invoke("rename_instance", { instanceId: selectedInstance.id, name });
      await refreshInstances();
      setShowRenameDialog(false);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function forgetSelectedInstance() {
    if (!selectedInstance) return;
    setError("");
    try {
      await invoke("forget_instance", { instanceId: selectedInstance.id, confirmed: true });
      const remaining = instances.filter((instance) => instance.id !== selectedInstance.id);
      setFavoriteIds((current) => current.filter((id) => id !== selectedInstance.id));
      setInstances(remaining);
      setSelectedInstanceId(remaining[0]?.id ?? null);
      setShowForgetDialog(false);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function openSelectedInstanceFolder() {
    if (!selectedInstance) return;
    setError("");
    try {
      await invoke("open_instance_folder", { instanceId: selectedInstance.id });
    } catch (reason) {
      setError(String(reason));
    }
  }

  function toggleFavorite(instanceId: string) {
    setFavoriteIds((current) => current.includes(instanceId)
      ? current.filter((id) => id !== instanceId)
      : [...current, instanceId]);
  }

  async function launchSelected() {
    if (!selectedInstance || !selectedProfile || isInstallingVersion) return;
    const repairDownloadId = selectedInstance.engine === "minecraft" || selectedInstance.horizonFabricEnabled
      ? crypto.randomUUID()
      : null;
    if (repairDownloadId) {
      activeDownloadIdRef.current = repairDownloadId;
      setActiveDownloadId(repairDownloadId);
      setDownloadProgress({
        downloadId: repairDownloadId,
        versionId: selectedInstance.versionId ?? "Minecraft",
        phase: "Checking Minecraft files",
        currentFile: "Checking this instance before launch",
        completedFiles: 0,
        totalFiles: 0,
        downloadedBytes: 0,
        totalBytes: 0,
      });
      setIsInstallingVersion(true);
    }
    setError("");
    setLogs([]);
    setLaunchState("running");
    setBusyMessage("Starting Minecraft");
    try {
      const started = selectedInstance.engine === "horizon"
        ? await invoke<LaunchStarted>("launch_horizon_instance", {
          instanceId: selectedInstance.id,
          profileId: selectedProfile.id,
          width: selectedHorizonWindow.width,
          height: selectedHorizonWindow.height,
          fullscreen: selectedHorizonWindow.fullscreen,
          downloadId: repairDownloadId ?? crypto.randomUUID(),
        })
        : await invoke<LaunchStarted>("launch_instance", {
          instanceId: selectedInstance.id,
          profileId: selectedProfile.id,
          downloadId: repairDownloadId ?? crypto.randomUUID(),
        });
      setLogPath(started.logPath);
      setLogs((current) => [`Process ${started.pid} started. Local log: ${started.logPath}`, ...current]);
    } catch (reason) {
      setLaunchState("idle");
      const message = String(reason);
      setError(message);
      if (repairDownloadId) {
        setDownloadProgress((current) => current ? {
          ...current,
          phase: message.toLowerCase().includes("cancel") ? "Installation cancelled" : "Installation failed",
          currentFile: "",
        } : current);
      }
    } finally {
      if (repairDownloadId) {
        activeDownloadIdRef.current = null;
        setActiveDownloadId(null);
        setIsInstallingVersion(false);
      }
    }
  }

  async function prepareHorizonResources() {
    if (isInstallingVersion || !horizonPrepareJavaPath) return;
    const downloadId = crypto.randomUUID();
    activeDownloadIdRef.current = downloadId;
    setActiveDownloadId(downloadId);
    setDownloadProgress({
      downloadId,
      versionId: "26.3 · Horizon",
      phase: "Preparing Minecraft 26.3 files",
      currentFile: horizonMinecraftSourcePath ? "Checking the selected installation and fetching missing files" : "Fetching official files into Horizon’s private cache",
      completedFiles: 0,
      totalFiles: 0,
      downloadedBytes: 0,
      totalBytes: 0,
    });
    setIsInstallingVersion(true);
    setHorizonPreparing(true);
    setHorizonSettingsError("");
    try {
      const path = await invoke<string>("prepare_horizon_resources", {
        javaPath: horizonPrepareJavaPath,
        minecraftSourcePath: horizonMinecraftSourcePath || null,
        downloadId,
      });
      setHorizonResourcesPath(path);
      setHorizonResourcesReady(true);
      setDownloadProgress((current) => current ? {
        ...current,
        phase: "Installation complete",
        currentFile: path,
        completedFiles: current.totalFiles,
      } : current);
    } catch (reason) {
      const message = String(reason);
      setHorizonSettingsError(message);
      setDownloadProgress((current) => current ? {
        ...current,
        phase: message.toLowerCase().includes("cancel") ? "Installation cancelled" : "Installation failed",
      } : current);
    } finally {
      activeDownloadIdRef.current = null;
      setActiveDownloadId(null);
      setIsInstallingVersion(false);
      setHorizonPreparing(false);
    }
  }

  function updateHorizonWindowSettings(value: HorizonWindowSettings) {
    if (!selectedInstance || selectedInstance.engine !== "horizon") return;
    setHorizonWindowSettings((current) => ({ ...current, [selectedInstance.id]: value }));
  }

  const activeNavigation = navigation.find((item) => item.id === activePage) ?? navigation[0];
  const horizonSelected = selectedInstance?.engine === "horizon";
  const horizonJavaFabricSelected = horizonSelected && Boolean(selectedInstance?.horizonFabricEnabled);
  const canLaunch = desktop && Boolean(selectedInstance && selectedProfile
    && (!horizonSelected || !selectedProfileIsMicrosoft)
    && (horizonSelected
      ? horizonJavaFabricSelected
        ? selectedInstance.horizonFabricVersionId && selectedInstance.horizonFabricJavaPath
        : horizonClientInfo && horizonResourcesReady
      : selectedInstance.versionId && selectedJava))
    && launchState !== "running" && !isBusy && !isInstallingVersion;
  const launchHint = !desktop ? "Open the desktop app to launch Minecraft."
    : !selectedInstance ? "Choose or install an instance to get started."
      : !selectedProfile ? "Create a local developer profile for testing."
        : horizonSelected && selectedProfileIsMicrosoft ? "Horizon currently uses local profiles. Select a local account, or choose a Minecraft Java instance for Microsoft sign-in."
        : horizonJavaFabricSelected && !selectedInstance.horizonFabricVersionId ? "Install the optional Java + Fabric runtime in Horizon settings first."
          : horizonJavaFabricSelected && !selectedInstance.horizonFabricJavaPath ? "Choose Java 25+ for this profile in Horizon settings."
        : horizonSelected && !horizonJavaFabricSelected && !horizonClientInfo ? "The bundled Horizon engine is unavailable in this build."
          : horizonSelected && !horizonJavaFabricSelected && !horizonResourcesReady ? "Download the official 26.3 files and prepare Horizon resources in Horizon settings."
            : !horizonSelected && !selectedInstance.versionId ? "Install a Minecraft version for this instance first."
              : !horizonSelected && !selectedJava ? "Select a Java runtime before launching."
                : launchState === "running" ? "The selected game is running."
                  : isBusy ? "The launcher is working."
                    : horizonJavaFabricSelected ? "Ready to launch the isolated Java + Fabric copy with your local profile."
                      : horizonSelected ? "Ready to launch Horizon offline with telemetry disabled."
                      : "Ready to launch.";
  const versionedInstanceCount = instances.filter((instance) => instance.versionId).length;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark"><Gamepad2 size={21} strokeWidth={2.1} /></div>
          <div className="brand-copy"><strong>OpnLauncher</strong><span>YOUR WORLDS, YOUR WAY</span></div>
        </div>

        <button className="sidebar-profile" type="button" onClick={() => setActivePage("accounts")} title="Manage local developer profiles">
          <span className="profile-avatar">{selectedProfile?.name.slice(0, 1).toUpperCase() ?? <UsersRound size={16} />}</span>
          <span className="sidebar-profile-copy"><strong>{selectedProfile?.name ?? "Set up a profile"}</strong><small>{selectedProfileIsMicrosoft ? "MICROSOFT · VERIFIED" : "UNVERIFIED · LOCAL"}</small></span>
          <ChevronDown size={15} />
        </button>

        <div className="sidebar-navigation">
          {navigationGroups.map((group) => (
            <div className="nav-group" key={group}>
              <div className="nav-label">{group}</div>
              <nav className="main-nav" aria-label={`${group.toLocaleLowerCase()} navigation`}>
                {navigation.filter((item) => item.group === group).map(({ id, label, icon: Icon }) => (
                  <button
                    className={`nav-item ${activePage === id ? "is-active" : ""}`}
                    key={id}
                    onClick={() => setActivePage(id)}
                    type="button"
                    aria-label={label}
                    title={label}
                    aria-current={activePage === id ? "page" : undefined}
                  >
                    <Icon size={18} strokeWidth={1.8} />
                    <span>{label}</span>
                    {id === "instances" && instances.length > 0 && <span className="nav-count">{instances.length}</span>}
                    {id === "downloads" && isInstallingVersion && <span className="nav-dot" aria-label="Download in progress" />}
                  </button>
                ))}
              </nav>
            </div>
          ))}
        </div>

        <div className="sidebar-bottom">
          <div className="privacy-card">
            <div className="privacy-icon"><ShieldCheck size={16} /></div>
            <div><strong>Private by default</strong><span>Your launcher data stays here.</span></div>
          </div>
          <div className="sidebar-version"><span className="version-mark">OP</span><span>{appVersion}</span><span className="dev-badge">DEV</span></div>
        </div>
      </aside>

      <main className="main-content">
        <header className="topbar">
          <div className="breadcrumbs"><span>Workspace</span><ArrowRight size={13} /><strong>{activeNavigation.label}</strong></div>
          <div className="topbar-actions">
            <label className="global-search">
              <Search size={16} />
              <input
                ref={globalSearchRef}
                aria-label="Search instances"
                placeholder="Find an instance"
                value={instanceSearch}
                onFocus={() => setActivePage("instances")}
                onChange={(event) => { setInstanceSearch(event.target.value); setActivePage("instances"); }}
              />
              <kbd>Ctrl K</kbd>
            </label>
            <button className="topbar-new-button" type="button" onClick={() => setShowAddDialog(true)}><Plus size={16} /><span>New instance</span></button>
            <div className="topbar-profile-switcher">
              <label className="topbar-account topbar-account-select">
                <span className={`profile-avatar ${selectedProfileIsMicrosoft ? "microsoft-avatar" : ""}`}>{selectedProfile?.name.slice(0, 1).toUpperCase() ?? <UsersRound size={15} />}</span>
                <select aria-label="Switch active account" value={selectedProfileId} onChange={(event) => setSelectedProfileId(event.target.value)}>
                  {!selectedProfileId && <option value="">Set up a profile</option>}
                  {microsoftAccounts.length > 0 && <optgroup label="Microsoft · verified">{microsoftAccounts.map((account) => <option key={account.id} value={account.id}>{account.name}</option>)}</optgroup>}
                  {profiles.length > 0 && <optgroup label="Local · unverified">{profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.name}</option>)}</optgroup>}
                </select>
                <ChevronDown size={14} />
              </label>
              <button className="topbar-account-manage" type="button" onClick={() => setActivePage("accounts")} aria-label="Manage accounts" title="Manage accounts"><UsersRound size={16} /></button>
            </div>
          </div>
        </header>

        <div className="page-scroll">
          {error && <div className="error-banner" role="alert"><CircleAlert size={18} /><span>{error}</span><button type="button" onClick={() => setError("")} aria-label="Dismiss error"><X size={16} /></button></div>}
          {!desktop && <div className="notice-banner"><CircleAlert size={18} /><span>Desktop features are available when OpnLauncher is opened as a desktop app.</span></div>}

          {activePage === "home" ? (
            <>
              <section className="page-heading">
                <div><p className="eyebrow"><Sparkles size={14} /> YOUR LOCAL GAME SPACE</p><h1>Let’s play.</h1><p className="page-subtitle">A calm home for every instance, version, and test profile.</p></div>
                <div className="device-status"><span className="status-pip" /> Local device <span className="status-divider" /> {runtimes.length} Java runtime{runtimes.length === 1 ? "" : "s"}</div>
              </section>

              <div className="home-grid">
                <section className="hero-card">
                  <div className="hero-art" aria-hidden="true" />
                  <div className="hero-content">
                    <div className="hero-topline"><span className="hero-kicker"><Zap size={14} /> QUICK PLAY</span><span className="hero-local-tag"><span /> LOCAL INSTANCE</span></div>
                    {selectedInstance ? (
                      <>
                        <div className="hero-version"><Layers3 size={15} /> {selectedInstance.engine === "horizon" ? selectedInstance.horizonFabricEnabled ? "Minecraft 26.3 · Fabric Java runtime" : `Minecraft ${selectedInstance.versionId} · Horizon (Rust)` : `Minecraft Java · ${selectedInstance.versionId ?? "No version installed"}`}</div>
                        <h2>{selectedInstance.name}</h2>
                        <p className="hero-description">{selectedInstance.engine === "horizon" ? selectedInstance.horizonFabricEnabled ? "Optional mode runs the Java Minecraft client with Fabric in a separate folder. Switch back to the Rust engine in Horizon settings." : "A separate Rust-powered profile with its own local worlds and content mods." : "Your selected world is ready whenever you are."}</p>
                        <div className="hero-facts">
                          {selectedInstance.engine === "horizon" ? <><span><Sparkles size={15} /> {selectedInstance.horizonFabricEnabled ? "Java + Fabric backend" : "Rust engine"}</span><span><ShieldCheck size={15} /> Local profile · offline</span></> : <><span><Cpu size={15} /> {selectedJava ? javaLabel(selectedJava.majorVersion) : "Java not selected"}</span><span><HardDrive size={15} /> {formatMemory(memoryChoice)}</span></>}
                          <span><Clock3 size={15} /> {formatRelativeTime(selectedInstance.lastPlayedAt)}</span>
                        </div>
                        <div className="hero-actions">
                          <button className="play-button" type="button" disabled={!canLaunch} onClick={() => void launchSelected()}>
                            {launchState === "running" ? <LoaderCircle className="spin" size={18} /> : <Play size={17} fill="currentColor" />}
                            {launchState === "running" ? "Launching…" : "Play now"}
                          </button>
                          <button className="hero-secondary-button" type="button" onClick={() => setActivePage("instances")}><Settings2 size={16} /> Instance settings</button>
                          <button className={`favorite-toggle hero-favorite ${favoriteIds.includes(selectedInstance.id) ? "is-favorite" : ""}`} type="button" onClick={() => toggleFavorite(selectedInstance.id)} aria-label={favoriteIds.includes(selectedInstance.id) ? "Remove from favorites" : "Add to favorites"} aria-pressed={favoriteIds.includes(selectedInstance.id)}><Star size={17} fill={favoriteIds.includes(selectedInstance.id) ? "currentColor" : "none"} /></button>
                        </div>
                        <p className={`launch-hint ${canLaunch ? "is-ready" : ""}`}><span /> {launchHint}</p>
                      </>
                    ) : (
                      <div className="hero-empty">
                        <h2>Your next world awaits.</h2>
                        <p>Install an official Minecraft version or connect an existing game folder to create your first instance.</p>
                        <div className="hero-actions"><button className="play-button" type="button" onClick={() => setActivePage("versions")}><ArrowDownToLine size={17} /> Browse versions</button><button className="hero-secondary-button" type="button" onClick={() => setShowAddDialog(true)}><Plus size={16} /> Add existing files</button></div>
                      </div>
                    )}
                  </div>
                </section>

                <aside className="home-rail">
                  <section className="account-summary-card">
                    <div className="card-kicker"><UsersRound size={15} /> TEST PROFILE</div>
                    <button className="account-summary-main" type="button" onClick={() => setActivePage("accounts")}>
                      <span className="account-large-avatar">{selectedProfile?.name.slice(0, 1).toUpperCase() ?? <UsersRound size={20} />}</span>
                      <span><strong>{selectedProfile?.name ?? "Create an account"}</strong><small>{selectedProfile ? selectedProfileIsMicrosoft ? "Microsoft · Minecraft ownership verified" : "Unverified · local only" : "Needed to launch an instance"}</small></span>
                      <ChevronDown size={16} />
                    </button>
                    <div className="account-local-note"><ShieldCheck size={15} /><span>{selectedProfileIsMicrosoft ? "Tokens stay in the operating system credential store. Online access uses Microsoft authentication." : "No password is collected. This local profile does not authenticate to Microsoft or online servers."}</span></div>
                    <button className="text-link" type="button" onClick={() => setActivePage("accounts")}>Manage accounts<ArrowRight size={14} /></button>
                  </section>
                  <section className="device-card">
                    <div className="device-card-heading"><span className="device-card-icon"><MonitorCog size={17} /></span><span><strong>This device</strong><small>Local setup overview</small></span></div>
                    <div className="device-stat-row"><span>Instances</span><strong>{instances.length}</strong></div>
                    <div className="device-stat-row"><span>Versioned instances</span><strong>{versionedInstanceCount}</strong></div>
                    <div className="device-stat-row"><span>Java runtimes</span><strong>{runtimes.length}</strong></div>
                    <button className="device-settings-link" type="button" onClick={() => setActivePage("settings")}>Review setup <ArrowRight size={14} /></button>
                  </section>
                </aside>
              </div>

              <PlaytimeCard desktop={desktop} />

              <section className="section-block">
                <div className="section-heading"><div><p className="eyebrow">PICK UP WHERE YOU LEFT OFF</p><h2>Recent instances</h2></div><button className="text-link" type="button" onClick={() => setActivePage("instances")}>View library <ArrowRight size={15} /></button></div>
                {recentInstances.length > 0 ? (
                  <div className="home-instance-grid">
                    {recentInstances.map((instance, index) => (
                      <article className={`home-instance-card ${selectedInstanceId === instance.id ? "selected" : ""}`} key={instance.id}>
                        <button className={`home-instance-main art-${index % 4}`} type="button" onClick={() => setSelectedInstanceId(instance.id)} aria-label={`Select ${instance.name}`}>
                          <span className="instance-art-icon"><Gamepad2 size={20} /></span><span className="instance-version-chip">{instance.versionId ?? "Setup needed"}</span>
                        </button>
                        <div className="home-instance-info"><button type="button" className="instance-title-button" onClick={() => setSelectedInstanceId(instance.id)}><strong>{instance.name}</strong><small>{formatRelativeTime(instance.lastPlayedAt)}</small></button><button className={`favorite-toggle ${favoriteIds.includes(instance.id) ? "is-favorite" : ""}`} type="button" onClick={() => toggleFavorite(instance.id)} aria-label={favoriteIds.includes(instance.id) ? "Remove from favorites" : "Add to favorites"} aria-pressed={favoriteIds.includes(instance.id)}><Star size={16} fill={favoriteIds.includes(instance.id) ? "currentColor" : "none"} /></button></div>
                      </article>
                    ))}
                    <button className="add-instance-tile" type="button" onClick={() => setShowAddDialog(true)}><span><Plus size={18} /></span><strong>New instance</strong><small>Download or connect a game</small></button>
                  </div>
                ) : (
                  <div className="empty-library-card"><div className="empty-icon"><Boxes size={23} /></div><div><strong>Your library is empty</strong><span>Start with a Minecraft version download or add local files.</span></div><button className="play-button" type="button" onClick={() => setActivePage("versions")}><ArrowDownToLine size={16} /> Browse versions</button></div>
                )}
              </section>

              {launchState !== "idle" && (
                <section className="console-panel">
                  <div className="console-heading"><div><TerminalSquare size={17} /><strong>Game session output</strong><span className={`console-status ${launchState}`}>{launchState === "running" ? "RUNNING" : "CLOSED"}</span></div><span>{logPath}</span></div>
                  <div className="console-body" role="log" aria-live="polite">{latestLogLines.map((line, index) => <code key={`${index}-${line}`}>{line}</code>)}</div>
                </section>
              )}

              <section className="shortcut-row">
                <button type="button" onClick={() => setActivePage("versions")}><span className="shortcut-icon accent-version"><Layers3 size={18} /></span><span><strong>Explore versions</strong><small>Releases, snapshots, and classics</small></span><ArrowRight size={16} /></button>
                <button type="button" onClick={() => setActivePage("downloads")}><span className="shortcut-icon accent-download"><ArrowDownToLine size={18} /></span><span><strong>Download activity</strong><small>{isInstallingVersion ? "An installation is in progress" : "Review your latest install"}</small></span><ArrowRight size={16} /></button>
                <button type="button" onClick={() => setActivePage("accounts")}><span className="shortcut-icon accent-account"><UsersRound size={18} /></span><span><strong>Test profiles</strong><small>{profiles.length} local profile{profiles.length === 1 ? "" : "s"} saved here</small></span><ArrowRight size={16} /></button>
              </section>
            </>
          ) : activePage === "instances" ? (
            <>
              <section className="page-heading page-heading-row">
                <div><p className="eyebrow"><Boxes size={14} /> YOUR LIBRARY</p><h1>Instances</h1><p className="page-subtitle">Separate game folders, settings, and worlds for every setup.</p></div>
                <div className="heading-actions"><button className="secondary-button" type="button" onClick={() => void refreshInstallations()}><FolderOpen size={16} /> Scan local files</button><button className="play-button" type="button" onClick={() => setShowAddDialog(true)}><Plus size={16} /> Add instance</button></div>
              </section>
              <div className="library-overview">
                <button type="button" className={instanceFilter === "all" ? "selected" : ""} onClick={() => setInstanceFilter("all")}><span>All instances</span><strong>{instances.length}</strong></button>
                <button type="button" className={instanceFilter === "favorites" ? "selected" : ""} onClick={() => setInstanceFilter("favorites")}><span>Favorites</span><strong>{favoriteIds.length}</strong></button>
                <button type="button" onClick={() => setActivePage("versions")}><span>Install a version</span><ArrowRight size={16} /></button>
              </div>
              <div className="instance-toolbar">
                <label className="search-box"><Search size={17} /><input aria-label="Search instances" placeholder="Search by name, version, or folder" value={instanceSearch} onChange={(event) => setInstanceSearch(event.target.value)} /><kbd>/</kbd></label>
                <label className="instance-sort-control"><span>Sort</span><select aria-label="Sort instances" value={instanceSort} onChange={(event) => setInstanceSort(event.target.value as InstanceSort)}><option value="recent">Recently played</option><option value="name">Name</option><option value="version">Minecraft version</option></select><ChevronDown size={14} /></label>
                <div className="view-switch" role="group" aria-label="Instance layout">
                  <button type="button" className={instanceView === "grid" ? "selected" : ""} onClick={() => setInstanceView("grid")} aria-label="Grid view"><LayoutGrid size={17} /></button>
                  <button type="button" className={instanceView === "list" ? "selected" : ""} onClick={() => setInstanceView("list")} aria-label="List view"><List size={18} /></button>
                </div>
              </div>
              <div className="instance-filter-row" role="group" aria-label="Filter instances by game engine">
                <span>Show</span>
                {([{ id: "all", label: "Everything" }, { id: "minecraft", label: "Minecraft Java" }, { id: "horizon", label: "Horizon" }] as const).map((filter) => <button key={filter.id} type="button" className={instanceEngineFilter === filter.id ? "selected" : ""} onClick={() => setInstanceEngineFilter(filter.id)} aria-pressed={instanceEngineFilter === filter.id}>{filter.label}<small>{filter.id === "all" ? instances.length : instances.filter((instance) => instance.engine === filter.id).length}</small></button>)}
              </div>
              {instances.length === 0 ? (
                <div className="empty-library-card empty-library-large"><div className="empty-icon"><Gamepad2 size={23} /></div><div><strong>Nothing in your library yet</strong><span>Install a version from Mojang or register a game folder already on this device.</span></div><div className="empty-actions"><button className="play-button" type="button" onClick={() => setActivePage("versions")}><ArrowDownToLine size={16} /> Browse versions</button><button className="secondary-button" type="button" onClick={() => setShowAddDialog(true)}>Add local files</button></div></div>
              ) : visibleInstances.length === 0 ? (
                <div className="empty-library-card empty-library-large"><div className="empty-icon"><Search size={22} /></div><div><strong>{instanceFilter === "favorites" && favoriteIds.length === 0 ? "No favorites yet" : "No matches found"}</strong><span>{instanceFilter === "favorites" && favoriteIds.length === 0 ? "Mark an instance with a star and it will appear here." : "Try another search or clear the current filter."}</span></div><button className="secondary-button" type="button" onClick={() => { setInstanceSearch(""); setInstanceFilter("all"); setInstanceEngineFilter("all"); }}>Clear filters</button></div>
              ) : (
                <div className={`instances-workspace ${instanceView === "list" ? "list-view" : "grid-view"}`}>
                  <div className={`instance-collection ${instanceView === "list" ? "as-list" : "as-grid"}`}>
                    {visibleInstances.map((instance, index) => (
                      <article className={`library-instance-card ${selectedInstanceId === instance.id ? "selected" : ""}`} key={instance.id}>
                        <button className={`library-instance-art art-${index % 4}`} type="button" onClick={() => setSelectedInstanceId(instance.id)} aria-label={`Select ${instance.name}`}><Gamepad2 size={20} /><span className="art-grid-overlay" /></button>
                        <div className="library-instance-body">
                        <div className="library-instance-heading"><button type="button" className="instance-title-button" onClick={() => setSelectedInstanceId(instance.id)}><strong>{instance.name}</strong><small>{instance.engine === "horizon" ? `Horizon · Minecraft ${instance.versionId ?? "26.3"}` : `Minecraft Java · ${instance.versionId ?? "No version installed"}`}</small></button><button className={`favorite-toggle ${favoriteIds.includes(instance.id) ? "is-favorite" : ""}`} type="button" onClick={() => toggleFavorite(instance.id)} aria-label={favoriteIds.includes(instance.id) ? "Remove from favorites" : "Add to favorites"} aria-pressed={favoriteIds.includes(instance.id)}><Star size={16} fill={favoriteIds.includes(instance.id) ? "currentColor" : "none"} /></button></div>
                          <div className="library-instance-meta"><span><Clock3 size={13} /> {formatRelativeTime(instance.lastPlayedAt)}</span><span className={`install-state ${instance.versionId ? "ready" : "needs-setup"}`}><span /> {instance.versionId ? "Installed" : "Setup needed"}</span></div>
                        </div>
                      </article>
                    ))}
                    <button className="add-instance-tile library-add-tile" type="button" onClick={() => setShowAddDialog(true)}><span><Plus size={18} /></span><strong>Add an instance</strong><small>Use a download or local files</small></button>
                  </div>

                  {selectedInstance && (
                    <section className="instance-settings-card">
                      <div className="details-card-topline"><span>INSTANCE SETTINGS</span><button className={`favorite-toggle ${favoriteIds.includes(selectedInstance.id) ? "is-favorite" : ""}`} type="button" onClick={() => toggleFavorite(selectedInstance.id)} aria-label={favoriteIds.includes(selectedInstance.id) ? "Remove from favorites" : "Add to favorites"} aria-pressed={favoriteIds.includes(selectedInstance.id)}><Star size={17} fill={favoriteIds.includes(selectedInstance.id) ? "currentColor" : "none"} /></button></div>
                      <div className="details-instance-title"><span className="details-game-icon"><Gamepad2 size={22} /></span><span><strong>{selectedInstance.name}</strong><small>{selectedInstance.engine === "horizon" ? selectedInstance.horizonFabricEnabled ? "Minecraft 26.3 · Java + Fabric compatibility runtime" : `Minecraft ${selectedInstance.versionId ?? "26.3"} · Horizon Rust engine` : selectedInstance.versionId ?? "No game version selected"}</small></span></div>
                      <div className="details-divider" />
                      {selectedInstance.engine === "horizon" ? (
                        <>
                          <div className="instance-game-summary"><span><strong>{selectedInstance.horizonFabricEnabled ? "Fabric compatibility mode" : "Horizon game setup"}</strong><small>{selectedInstance.horizonFabricEnabled ? "Compatible Java Fabric mods run in the isolated Minecraft client." : "Separate native Rust engine for Minecraft 26.3"}</small></span><button className="secondary-button" type="button" onClick={() => setActivePage("horizon")}><Wrench size={14} /> Settings</button></div>
                          <div className="horizon-instance-actions"><button className="secondary-button" type="button" onClick={() => setActivePage("mods")}><PackageOpen size={14} /> {selectedInstance.horizonFabricEnabled ? "Manage Fabric mods" : "Manage Horizon mods"}</button><button className="settings-quiet-button" type="button" onClick={() => setActivePage("horizon")}><Cpu size={14} /> Client &amp; resources</button></div>
                          <div className="settings-callout"><ShieldCheck size={15} /><span>Uses the selected unverified local profile. Microsoft sign-in is off and Horizon telemetry is disabled.</span></div>
                        </>
                      ) : (
                        <>
                          <div className="instance-game-summary"><span><strong>Game setup</strong><small>{selectedInstance.versionId ? "Change Minecraft or loader version" : "Choose Minecraft and a loader to begin"}</small></span><button className="secondary-button" type="button" onClick={() => setShowInstanceSetup(true)}><Wrench size={14} /> Configure</button></div>
                          <label className="field-label" htmlFor="java-path">Java runtime</label>
                          <div className="select-wrap"><select id="java-path" className="field-control" value={selectedJavaPath} onChange={(event) => setSelectedJavaPath(event.target.value)}><option value="">Automatic detection</option>{runtimes.map((runtime) => <option key={runtime.path} value={runtime.path}>Java {runtime.majorVersion} · {runtime.vendor ?? "Runtime"}</option>)}</select><ChevronDown size={16} /></div>
                          {selectedJava ? <div className="runtime-description"><span className="runtime-dot" /><span><strong>{selectedJavaPath ? selectedJava.version : `Automatic · ${selectedJava.version}`}</strong><small title={selectedJava.path}>{selectedJava.path}</small></span><span className="runtime-architecture">{selectedJava.architecture ?? "Unknown"}</span></div> : <div className="inline-warning"><CircleAlert size={15} /><span>No Java runtime detected. Set JAVA_HOME, then refresh this page.</span></div>}
                          <div className="memory-heading"><div><label className="field-label" htmlFor="memory">Memory allocation</label><small>Maximum RAM available to Minecraft</small></div><strong>{formatMemory(memoryChoice)}</strong></div>
                          <input id="memory" className="memory-slider" type="range" min="1024" max="16384" step="512" value={memoryChoice} onChange={(event) => setMemoryChoice(Number(event.target.value))} aria-valuetext={formatMemory(memoryChoice)} style={{ background: `linear-gradient(90deg, var(--accent) 0%, var(--accent) ${((memoryChoice - 1024) / (16384 - 1024)) * 100}%, var(--line) ${((memoryChoice - 1024) / (16384 - 1024)) * 100}%, var(--line) 100%)` }} />
                          <div className="slider-scale"><span>1 GB</span><span>16 GB</span></div>
                          <button className="save-button" type="button" onClick={() => void saveInstanceSettings()} disabled={isSavingSettings}>{isSavingSettings ? <LoaderCircle className="spin" size={16} /> : <Check size={16} />}{isSavingSettings ? "Saving settings…" : "Save settings"}</button>
                        </>
                      )}
                      <div className="instance-folder-tools"><div className="detail-path"><HardDrive size={15} /><span title={selectedInstance.gameDir}>{selectedInstance.gameDir}</span></div><button className="secondary-button" type="button" onClick={() => void openSelectedInstanceFolder()}><FolderOpen size={15} /> Open folder</button></div>
                      <button className="detail-launch-button" type="button" disabled={!canLaunch} onClick={() => void launchSelected()}><Play size={15} fill="currentColor" /> Launch this instance</button>
                      <div className="instance-action-links"><button type="button" onClick={() => { setRenameValue(selectedInstance.name); setShowRenameDialog(true); }}>Rename</button><button type="button" className="danger-link" onClick={() => setShowForgetDialog(true)}>Remove from library</button></div>
                      <p className="preserve-note">Removing an instance keeps its game files, saves, and worlds on disk.</p>
                    </section>
                  )}
                </div>
              )}
              <form className="quick-create" onSubmit={(event) => void createBlankInstance(event)}><div><strong>Need a clean profile?</strong><span>Create a Java instance or Horizon Rust profile.</span></div><input value={instanceName} onChange={(event) => setInstanceName(event.target.value)} placeholder="New profile name" aria-label="New profile name" maxLength={48} /><select className="field-control quick-create-engine" aria-label="New profile engine" value={newInstanceEngine} onChange={(event) => setNewInstanceEngine(event.target.value as "minecraft" | "horizon")}><option value="minecraft">Minecraft Java</option><option value="horizon">Horizon Rust</option></select><button type="submit" className="secondary-button" disabled={isBusy || !instanceName.trim()}><Plus size={15} /> Create</button></form>
            </>
          ) : activePage === "versions" ? (
            <VersionsPage versions={officialVersions} loading={versionsLoading} error={versionsError} onRefresh={() => void loadOfficialVersions()} onInstall={chooseRemoteVersion} onCreateHorizon={() => { setNewInstanceEngine("horizon"); setInstanceName("Horizon 26.3"); setShowAddDialog(true); }} installations={installations} instances={instances} scanningInstallations={scanningInstallations} onScanInstallations={() => void refreshInstallations()} onAddDetected={requestAddDetectedInstallation} installing={isInstallingVersion} desktop={desktop} />
          ) : activePage === "downloads" ? (
            <DownloadsPage progress={downloadProgress} active={isInstallingVersion} onCancel={() => void cancelDownload()} />
          ) : activePage === "accounts" ? (
            <AccountsPage
              profiles={profiles}
              microsoftAccounts={microsoftAccounts}
              selectedProfileId={selectedProfileId}
              clientId={microsoftClientId}
              signingIn={microsoftSignInBusy}
              error={accountError}
              desktop={desktop}
              onSelect={(profileId) => setSelectedProfileId(profileId)}
              onCreate={() => setShowProfileForm(true)}
              onClientIdChange={setMicrosoftClientId}
              onSignIn={() => void signInMicrosoft()}
              onSignOut={(account) => void signOutMicrosoft(account)}
            />
          ) : activePage === "mods" ? (
            selectedInstance?.engine === "horizon"
              ? selectedInstance.horizonFabricEnabled
                ? <ModsPage desktop={desktop} instances={instances.filter((instance) => instance.engine === "horizon")} selectedInstanceId={selectedInstanceId} onSelectInstance={setSelectedInstanceId} onOpenInstances={() => setActivePage("horizon")} curseForgeKey={curseForgeApiKey} onCurseForgeKeyChange={setCurseForgeApiKey} horizonFabricMode hideInstancePicker />
                : <HorizonModsPage desktop={desktop} instances={instances} selectedInstanceId={selectedInstanceId} onSelectInstance={setSelectedInstanceId} onOpenInstances={() => setActivePage("instances")} onOpenHorizonSettings={() => setActivePage("horizon")} curseForgeKey={curseForgeApiKey} onCurseForgeKeyChange={setCurseForgeApiKey} />
              : <ModsPage desktop={desktop} instances={instances} selectedInstanceId={selectedInstanceId} onSelectInstance={setSelectedInstanceId} curseForgeKey={curseForgeApiKey} onCurseForgeKeyChange={setCurseForgeApiKey} onOpenInstances={() => setActivePage("instances")} />
          ) : activePage === "horizon" ? (
            <HorizonSettingsPage
              instances={instances}
              selectedInstance={selectedInstance}
              onSelectInstance={setSelectedInstanceId}
              clientInfo={horizonClientInfo}
              resourcesReady={horizonResourcesReady}
              prepareJavaPath={horizonPrepareJavaPath}
              onPrepareJavaPathChange={setHorizonPrepareJavaPath}
              runtimes={runtimes}
              installations={installations}
              minecraftSourcePath={horizonMinecraftSourcePath}
              onMinecraftSourceChange={setHorizonMinecraftSourcePath}
              onAddMinecraftFolder={addHorizonMinecraftFolder}
              onScanDevice={() => void refreshHorizonDiscovery()}
              scanning={horizonScanning}
              onPrepareResources={() => void prepareHorizonResources()}
              preparing={horizonPreparing}
              progress={downloadProgress}
              resourcesPath={horizonResourcesPath}
              error={horizonSettingsError}
              width={selectedHorizonWindow.width}
              height={selectedHorizonWindow.height}
              fullscreen={selectedHorizonWindow.fullscreen}
              onWindowChange={updateHorizonWindowSettings}
              onInstanceUpdated={(updated) => setInstances((current) => current.map((instance) => instance.id === updated.id ? updated : instance))}
              onOpenMods={() => setActivePage("mods")}
              desktop={desktop}
            />
          ) : activePage === "settings" ? (
            <SettingsPage
              runtimes={runtimes}
              installations={installations}
              instances={instances}
              selectedInstance={selectedInstance?.engine === "minecraft" ? selectedInstance : null}
              refreshInstallations={refreshInstallations}
              refreshRuntimes={refreshRuntimes}
              addRuntime={addJavaRuntime}
              theme={theme}
              onThemeChange={setTheme}
              accent={accent}
              onAccentChange={setAccent}
              defaultJavaPath={defaultJavaPath}
              onDefaultJavaChange={setDefaultJavaPath}
              defaultMemoryMb={defaultMemoryMb}
              onDefaultMemoryChange={setDefaultMemoryMb}
              desktop={desktop}
              appVersion={appVersion}
              onInstanceUpdated={(updated) => {
                setInstances((current) => current.map((instance) => instance.id === updated.id ? updated : instance));
                if (selectedInstanceId === updated.id) {
                  setMemoryChoice(updated.memoryMb);
                  setSelectedJavaPath(updated.javaPath ?? defaultJavaPath);
                }
              }}
              onBrowseVersions={() => setActivePage("versions")}
              onOpenInstances={() => setActivePage("instances")}
            />
          ) : (
            <>
              <section className="page-heading"><div><p className="eyebrow"><PackageOpen size={14} /> MODPACK WORKSPACE</p><h1>{activeNavigation.label}</h1><p className="page-subtitle">Create, install, and maintain modpacks without losing local changes.</p></div><span className="planned-badge"><span /> IN DEVELOPMENT</span></section>
              <div className="feature-preview-card"><div className="preview-icon"><PackageOpen size={22} /></div><div><span className="eyebrow">PLANNED INCREMENTALLY</span><h2>Modpacks, kept safely separate.</h2><p>Import, update, and export workflows will preserve local changes before they touch your files.</p><button className="text-link" type="button" onClick={() => setActivePage("versions")}>Browse Minecraft versions <ArrowRight size={15} /></button></div></div>
            </>
          )}

          <footer className="page-footer"><span>OpnLauncher is an independent community project and is not affiliated with Mojang or Microsoft.</span><span>{appVersion}</span></footer>
        </div>
      </main>

      {isBusy && busyMessage && <div className="busy-toast"><LoaderCircle className="spin" size={17} /> {busyMessage}</div>}

      {showAddDialog && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setShowAddDialog(false); }}>
          <section className="modal-card add-instance-modal" role="dialog" aria-modal="true" aria-labelledby="add-modal-title">
            <div className="modal-header"><div><p className="eyebrow"><Plus size={14} /> BUILD YOUR LIBRARY</p><h2 id="add-modal-title">Add an instance</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setShowAddDialog(false)}><X size={18} /></button></div>
            <p className="modal-copy">Create an isolated Java instance or a separate Rust-powered Horizon profile. Both use the same local test accounts.</p>
            <form className="new-instance-config-form" onSubmit={(event) => void createBlankInstance(event)}><label className="field-label" htmlFor="new-config-engine">Game engine</label><select id="new-config-engine" className="field-control" value={newInstanceEngine} onChange={(event) => setNewInstanceEngine(event.target.value as "minecraft" | "horizon")}><option value="minecraft">Minecraft Java · standard game</option><option value="horizon">Horizon Rust · Minecraft 26.3</option></select><label className="field-label" htmlFor="new-config-instance-name">Profile name</label><div><input id="new-config-instance-name" className="field-control" value={instanceName} onChange={(event) => setInstanceName(event.target.value)} placeholder={newInstanceEngine === "horizon" ? "For example, Horizon 26.3" : "For example, Fabric Survival"} maxLength={48} /><button className="play-button" type="submit" disabled={isBusy || !instanceName.trim()}><Wrench size={15} /> {newInstanceEngine === "horizon" ? "Create Horizon profile" : "Create & configure"}</button></div><small>{newInstanceEngine === "horizon" ? "Horizon uses its own game runtime, settings, content mods, and worlds." : "Choose a Minecraft version and Vanilla, Fabric, or Forge after creating the instance."}</small></form>
            <button className="version-download-callout" type="button" onClick={() => { setShowAddDialog(false); setActivePage("versions"); }} disabled={!desktop}><span className="version-note-icon"><ArrowDownToLine size={17} /></span><span><strong>Install an official version</strong><small>Choose a release, snapshot, or historical version.</small></span><ArrowRight size={17} /></button>
            <div className="modal-section-heading"><span>EXISTING INSTALLATIONS</span><button className="text-link" type="button" onClick={() => void refreshInstallations()}><FolderOpen size={14} /> Rescan</button></div>
            {installations.length > 0 ? <div className="detected-list">{installations.map((installation) => <div className="detected-install" key={installation.root}><div className="detected-root"><HardDrive size={16} /><span title={installation.root}>{installation.root}</span></div><div className="detected-version-list">{installation.versions.slice(0, 6).map((version) => <button className="version-option" key={version.id} type="button" onClick={() => requestAddDetectedInstallation(installation, version.id)}><span><strong>{version.id}</strong><small>{version.kind}</small></span><Plus size={16} /></button>)}</div></div>)}</div> : <div className="no-detected"><FolderOpen size={19} /><span>No standard installation found. You can still download a version above or inspect a folder below.</span></div>}
            <label className="field-label" htmlFor="custom-install-path">Inspect another game folder</label><div className="custom-path-row"><input id="custom-install-path" className="field-control" value={installPath} onChange={(event) => setInstallPath(event.target.value)} placeholder="Path to .minecraft" /><button className="secondary-button" type="button" onClick={() => void inspectCustomInstallation()} disabled={!installPath.trim() || isBusy}>Inspect</button></div>
            {inspectedInstall && <div className="inspected-version"><label className="field-label" htmlFor="version-choice">Choose installed version</label><select id="version-choice" className="field-control" value={versionChoice} onChange={(event) => setVersionChoice(event.target.value)}>{inspectedInstall.versions.map((version) => <option value={version.id} key={version.id}>{version.id} · {version.kind}</option>)}</select><button className="play-button" type="button" onClick={() => void addInspectedInstallation()}><Plus size={16} /> Add selected version</button></div>}
          </section>
        </div>
      )}

      {showDetectedInstallDialog && detectedInstallTarget && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget && !isBusy) setShowDetectedInstallDialog(false); }}>
          <section className="modal-card small-modal" role="dialog" aria-modal="true" aria-labelledby="detected-install-title">
            <div className="modal-header"><div><p className="eyebrow"><HardDrive size={14} /> FOUND ON THIS DEVICE</p><h2 id="detected-install-title">Add Minecraft {detectedInstallTarget.versionId}</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setShowDetectedInstallDialog(false)} disabled={isBusy}><X size={18} /></button></div>
            <p className="modal-copy">This adds a launcher entry that points to the existing game files. OpnLauncher leaves the installation and its worlds in place.</p>
            <label className="field-label" htmlFor="detected-install-name">Instance name</label>
            <input id="detected-install-name" className="field-control" value={detectedInstallName} maxLength={48} onChange={(event) => setDetectedInstallName(event.target.value)} autoFocus />
            <p className="detected-install-path" title={detectedInstallTarget.installation.root}><HardDrive size={14} />{detectedInstallTarget.installation.root}</p>
            <div className="dialog-actions"><button className="secondary-button" type="button" onClick={() => setShowDetectedInstallDialog(false)} disabled={isBusy}>Cancel</button><button className="play-button" type="button" onClick={() => void addDetectedInstallation()} disabled={!detectedInstallName.trim() || isBusy}>{isBusy ? <LoaderCircle className="spin" size={15} /> : <Plus size={15} />}{isBusy ? "Adding…" : "Add to library"}</button></div>
          </section>
        </div>
      )}

      {showVersionInstallDialog && selectedRemoteVersion && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget && !isInstallingVersion) setShowVersionInstallDialog(false); }}>
          <section className="modal-card version-install-modal" role="dialog" aria-modal="true" aria-labelledby="version-install-title">
            <div className="modal-header"><div><p className="eyebrow"><Layers3 size={14} /> OFFICIAL MINECRAFT FILES</p><h2 id="version-install-title">Install {selectedRemoteVersion.id}</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setShowVersionInstallDialog(false)} disabled={isInstallingVersion}><X size={18} /></button></div>
            <p className="modal-copy">Files will be verified and installed into a new OpnLauncher instance folder. Your other Minecraft folders and worlds are left alone.</p>
            <form className="version-install-form" onSubmit={(event) => void installRemoteVersion(event)}><label className="field-label" htmlFor="version-instance-name">Instance name</label><input id="version-instance-name" className="field-control" value={versionInstanceName} onChange={(event) => setVersionInstanceName(event.target.value)} maxLength={48} required disabled={isInstallingVersion} />
              {installError && <div className="error-banner" role="alert"><CircleAlert size={16} /><span>{installError}</span></div>}
              {isInstallingVersion && downloadProgress && <div className="install-progress-card"><div><LoaderCircle className="spin" size={16} /><strong>{downloadProgress.phase}</strong><span>{downloadProgress.completedFiles} / {downloadProgress.totalFiles} files</span></div><div className="download-progress-track"><span style={{ width: `${downloadPercent}%` }} /></div>{downloadProgress.currentFile && <small>{downloadProgress.currentFile}</small>}</div>}
              <div className="version-install-actions">{isInstallingVersion ? <button className="secondary-button" type="button" onClick={() => void cancelDownload()}>Cancel download</button> : <button className="play-button" type="submit" disabled={!versionInstanceName.trim()}><ArrowDownToLine size={16} /> Install version</button>}</div>
            </form>
          </section>
        </div>
      )}

      {showInstanceSetup && selectedInstance && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget && !isInstallingVersion) setShowInstanceSetup(false); }}>
          <section className="modal-card instance-setup-modal" role="dialog" aria-modal="true" aria-labelledby="instance-setup-modal-title">
            <div className="modal-header"><div><p className="eyebrow"><Wrench size={14} /> INSTANCE CONFIGURATION</p><h2 id="instance-setup-modal-title">Configure {selectedInstance.name}</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setShowInstanceSetup(false)} disabled={isInstallingVersion}><X size={18} /></button></div>
            <InstanceSetupPanel
              key={selectedInstance.id}
              instance={selectedInstance}
              versions={officialVersions}
              versionsLoading={versionsLoading}
              versionsError={versionsError}
              onRefreshVersions={() => void loadOfficialVersions()}
              runtimes={runtimes}
              selectedJavaPath={selectedJavaPath}
              desktop={desktop}
              installing={isInstallingVersion}
              onInstall={installInstanceSetup}
            />
          </section>
        </div>
      )}

      {showProfileForm && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setShowProfileForm(false); }}>
          <section className="modal-card profile-modal" role="dialog" aria-modal="true" aria-labelledby="profile-modal-title">
            <div className="modal-header"><div><p className="eyebrow"><UsersRound size={14} /> LOCAL TESTING</p><h2 id="profile-modal-title">Developer profile</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setShowProfileForm(false)}><X size={18} /></button></div>
            <div className="profile-warning"><ShieldCheck size={17} /><span>Unverified local profile. It does not sign in to Microsoft, verify game ownership, or authenticate to online servers.</span></div>
            {profiles.length > 0 && <div className="saved-profiles">{profiles.map((profile) => <button className={`saved-profile ${selectedProfileId === profile.id ? "selected" : ""}`} key={profile.id} type="button" onClick={() => { setSelectedProfileId(profile.id); setShowProfileForm(false); }}><span className="profile-avatar">{profile.name.slice(0, 1).toUpperCase()}</span><span><strong>{profile.name}</strong><small>UNVERIFIED · LOCAL</small></span>{selectedProfileId === profile.id && <Check size={17} />}</button>)}</div>}
            <form className="profile-create-form" onSubmit={(event) => void createProfile(event)}><label className="field-label" htmlFor="profile-name">Test username</label><input id="profile-name" className="field-control" value={profileName} onChange={(event) => setProfileName(event.target.value)} placeholder="LocalPlayer" maxLength={16} pattern="[A-Za-z0-9_]{1,16}" required /><small>Use 1–16 letters, numbers, or underscores.</small><button className="play-button" type="submit"><Plus size={16} /> Create local profile</button></form>
          </section>
        </div>
      )}

      {showRenameDialog && selectedInstance && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setShowRenameDialog(false); }}>
          <section className="modal-card small-modal" role="dialog" aria-modal="true" aria-labelledby="rename-title"><div className="modal-header"><div><p className="eyebrow"><SlidersHorizontal size={14} /> INSTANCE SETTINGS</p><h2 id="rename-title">Rename instance</h2></div><button className="icon-button" type="button" onClick={() => setShowRenameDialog(false)} aria-label="Close"><X size={18} /></button></div><form className="dialog-form" onSubmit={(event) => void renameSelectedInstance(event)}><label className="field-label" htmlFor="rename-instance">Instance name</label><input id="rename-instance" className="field-control" value={renameValue} onChange={(event) => setRenameValue(event.target.value)} maxLength={48} required autoFocus /><div className="dialog-actions"><button className="secondary-button" type="button" onClick={() => setShowRenameDialog(false)}>Cancel</button><button className="play-button" type="submit" disabled={!renameValue.trim()}>Save name</button></div></form></section>
        </div>
      )}

      {showForgetDialog && selectedInstance && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setShowForgetDialog(false); }}>
          <section className="modal-card small-modal" role="alertdialog" aria-modal="true" aria-labelledby="forget-title"><div className="modal-header"><div><p className="eyebrow danger-eyebrow"><CircleAlert size={14} /> REMOVE FROM LIBRARY</p><h2 id="forget-title">Remove {selectedInstance.name}?</h2></div><button className="icon-button" type="button" onClick={() => setShowForgetDialog(false)} aria-label="Close"><X size={18} /></button></div><p className="modal-copy">This removes the instance entry from OpnLauncher. Its game files, saves, and worlds will stay on disk.</p><div className="dialog-actions"><button className="secondary-button" type="button" onClick={() => setShowForgetDialog(false)}>Keep instance</button><button className="danger-button" type="button" onClick={() => void forgetSelectedInstance()}>Remove from launcher</button></div></section>
        </div>
      )}
    </div>
  );
}

function AccountsPage({
  profiles,
  microsoftAccounts,
  selectedProfileId,
  clientId,
  signingIn,
  error,
  desktop,
  onSelect,
  onCreate,
  onClientIdChange,
  onSignIn,
  onSignOut,
}: {
  profiles: OfflineProfile[];
  microsoftAccounts: MicrosoftAccount[];
  selectedProfileId: string;
  clientId: string;
  signingIn: boolean;
  error: string;
  desktop: boolean;
  onSelect: (profileId: string) => void;
  onCreate: () => void;
  onClientIdChange: (clientId: string) => void;
  onSignIn: () => void;
  onSignOut: (account: MicrosoftAccount) => void;
}) {
  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId)
    ?? microsoftAccounts.find((account) => account.id === selectedProfileId)
    ?? null;

  return (
    <>
      <section className="page-heading page-heading-row">
        <div><p className="eyebrow"><UsersRound size={14} /> ACCOUNT CENTER</p><h1>Accounts</h1><p className="page-subtitle">Choose a verified Microsoft account for Java, or keep local profiles for development.</p></div>
        <button className="secondary-button" type="button" onClick={onCreate}><Plus size={16} /> Add local profile</button>
      </section>

      <div className="account-content-grid">
        <section className="account-manager-card">
          <section className="account-auth-section">
            <div className="section-heading account-list-heading"><div><p className="eyebrow">OFFICIAL JAVA ACCOUNT</p><h2>Microsoft sign-in</h2></div><span className="count-pill">{microsoftAccounts.length}</span></div>
            <p className="account-section-copy">Sign in through Microsoft’s system-browser flow. OpnLauncher checks for Minecraft Java ownership before saving the account.</p>
            <label className="account-client-id-label" htmlFor="microsoft-client-id">Application (client) ID</label>
            <div className="microsoft-signin-row">
              <input id="microsoft-client-id" className="field-control" value={clientId} onChange={(event) => onClientIdChange(event.target.value)} placeholder="Your Microsoft Entra app client ID" autoComplete="off" spellCheck={false} aria-describedby="microsoft-client-help" />
              <button className="play-button microsoft-signin-button" type="button" onClick={onSignIn} disabled={!desktop || signingIn || !clientId.trim()}>{signingIn ? <LoaderCircle className="spin" size={16} /> : <ExternalLink size={16} />}{signingIn ? "Waiting for Microsoft…" : "Continue with Microsoft"}</button>
            </div>
            <p className="account-client-help" id="microsoft-client-help">Use the public client ID from your own app registration. No client secret or password belongs in OpnLauncher.</p>
            <div className="microsoft-setup-guide">
              <div className="microsoft-guide-heading"><span className="microsoft-guide-mark"><ExternalLink size={16} /></span><span><strong>Connect your Microsoft account</strong><small>One-time app setup, then sign in here.</small></span><a href="https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade" target="_blank" rel="noreferrer">Open app registrations <ExternalLink size={13} /></a></div>
              <ol>
                <li><span>1</span><span>Create an app registration for personal Microsoft accounts and copy its <strong>Application (client) ID</strong>.</span></li>
                <li><span>2</span><span>Under <strong>Authentication</strong>, add a <strong>Mobile and desktop application</strong> platform with <code>http://localhost</code>, then allow public client flows.</span></li>
                <li><span>3</span><span>Paste the ID above. OpnLauncher opens Microsoft in your browser and verifies Minecraft Java ownership.</span></li>
              </ol>
              <a className="microsoft-guide-docs" href="https://learn.microsoft.com/en-us/entra/identity-platform/scenario-desktop-app-registration" target="_blank" rel="noreferrer">Read Microsoft’s desktop app registration guide <ArrowRight size={13} /></a>
            </div>
            {!desktop && <p className="account-inline-note"><Info size={15} /> Microsoft sign-in is available in the installed desktop app.</p>}
            {error && <p className="account-auth-error" role="alert"><CircleAlert size={15} />{error}</p>}

            {microsoftAccounts.length > 0 ? <div className="account-profile-list microsoft-account-list">{microsoftAccounts.map((account) => (
              <article className={`account-profile-card ${selectedProfileId === account.id ? "selected" : ""}`} key={account.id}>
                <div className="microsoft-account-row">
                  <button className="account-profile-select" type="button" onClick={() => onSelect(account.id)} aria-pressed={selectedProfileId === account.id}>
                    <span className="account-large-avatar microsoft-avatar">{account.name.slice(0, 1).toUpperCase()}</span>
                    <span><strong>{account.name}</strong><small>VERIFIED · MINECRAFT JAVA</small></span>
                    {selectedProfileId === account.id ? <span className="active-profile-badge"><CircleCheck size={14} /> Selected</span> : <span className="select-profile-label">Use account</span>}
                  </button>
                  <button className="account-signout-button" type="button" onClick={() => onSignOut(account)} aria-label={`Remove ${account.name} from OpnLauncher`} title="Remove account"><LogOut size={15} /></button>
                </div>
                <div className="profile-identifier"><span>Minecraft UUID</span><code>{account.uuid}</code></div>
              </article>
            ))}</div> : <div className="account-empty-inline"><ShieldCheck size={18} /><span>No verified Microsoft accounts saved on this device.</span></div>}
          </section>

          <section className="account-auth-section local-account-section">
            <div className="section-heading account-list-heading"><div><p className="eyebrow">DEVELOPMENT AND OFFLINE TESTING</p><h2>Local profiles</h2></div><span className="count-pill">{profiles.length}</span></div>
            <div className="profile-warning account-warning"><ShieldCheck size={18} /><span>Local profiles are unverified test identities. They do not prove ownership or grant Microsoft services or online-mode server access.</span></div>
            {profiles.length > 0 ? <div className="account-profile-list">{profiles.map((profile) => (
              <article className={`account-profile-card ${selectedProfileId === profile.id ? "selected" : ""}`} key={profile.id}>
                <button className="account-profile-select" type="button" onClick={() => onSelect(profile.id)} aria-pressed={selectedProfileId === profile.id}>
                  <span className="account-large-avatar">{profile.name.slice(0, 1).toUpperCase()}</span>
                  <span><strong>{profile.name}</strong><small>UNVERIFIED · LOCAL TEST PROFILE</small></span>
                  {selectedProfileId === profile.id ? <span className="active-profile-badge"><CircleCheck size={14} /> Selected</span> : <span className="select-profile-label">Use profile</span>}
                </button>
                {selectedProfileId === profile.id && <div className="profile-identifier"><span>Offline UUID</span><code>{profile.uuid}</code></div>}
              </article>
            ))}</div> : <div className="empty-profile-state"><span className="empty-icon"><UsersRound size={22} /></span><strong>No local profiles yet</strong><span>Create a local username for launcher development and offline testing.</span><button className="secondary-button" type="button" onClick={onCreate}><Plus size={15} /> Create the first profile</button></div>}
            {profiles.length > 0 && <button className="secondary-button add-profile-button" type="button" onClick={onCreate}><Plus size={15} /> Add another profile</button>}
          </section>
        </section>

        <aside className="account-boundaries-card">
          <div className="account-boundaries-icon"><ShieldCheck size={21} /></div>
          <p className="eyebrow">ACCOUNT SAFETY</p><h2>Two account types, separate purposes.</h2>
          <p>Microsoft accounts are checked with Microsoft, Xbox services, and Minecraft services. Local profiles stay on this device for development and offline testing.</p>
          <div className="boundary-list"><div><CircleCheck size={16} /><span>Java sessions refresh before launch.</span></div><div><CircleCheck size={16} /><span>Refresh tokens stay in the Linux desktop credential store.</span></div><div className="boundary-restricted"><CircleAlert size={16} /><span>Secure Microsoft token storage for Windows and macOS is not ready in this build.</span></div><div className="boundary-restricted"><CircleAlert size={16} /><span>Horizon Rust currently uses local profiles; Microsoft sign-in is not connected to Horizon.</span></div></div>
          <div className="planned-signin"><span className="planned-badge"><span /> PRIVACY</span><strong>No launcher account or tracking</strong><small>OpnLauncher does not operate an account server. Authentication requests go directly to Microsoft, Xbox, and Minecraft services when you sign in or launch.</small></div>
          {selectedProfile && <div className="selected-profile-note">Currently selected: <strong>{selectedProfile.name}</strong>{selectedProfileId.startsWith("microsoft:") ? " · verified" : " · local and unverified"}</div>}
        </aside>
      </div>
    </>
  );
}

export default App;
