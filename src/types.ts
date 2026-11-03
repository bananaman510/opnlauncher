export interface MinecraftVersion {
  id: string;
  kind: string;
  versionJson: string;
}

export interface MinecraftInstallation {
  root: string;
  versions: MinecraftVersion[];
}

export interface RemoteMinecraftVersion {
  id: string;
  kind: string;
  releaseTime: string;
  url: string;
  sha1: string | null;
}

export interface DownloadProgress {
  downloadId: string;
  versionId: string;
  phase: string;
  currentFile: string;
  completedFiles: number;
  totalFiles: number;
  downloadedBytes: number;
  totalBytes: number;
}

export interface Instance {
  id: string;
  name: string;
  gameDir: string;
  versionId: string | null;
  javaPath: string | null;
  memoryMb: number;
  createdAt: number;
  lastPlayedAt: number | null;
}

export interface JavaRuntime {
  path: string;
  version: string;
  majorVersion: number;
  architecture: string | null;
  vendor: string | null;
}

export interface LoaderVersion {
  version: string;
  stable: boolean;
  recommended: boolean;
}

export interface InstalledLoaderProfile {
  versionId: string;
  minecraftVersion: string;
  loader: string;
  loaderVersion: string;
}

export interface OfflineProfile {
  id: string;
  name: string;
  uuid: string;
  verified: boolean;
  createdAt: number;
}

export interface LaunchLogEvent {
  instanceId: string;
  stream: string;
  text: string;
}

export interface LaunchExitEvent {
  instanceId: string;
  success: boolean;
  code: number | null;
  logPath: string;
}

export interface LaunchStarted {
  pid: number;
  logPath: string;
}

export type ModProvider = "modrinth" | "curseforge";

export interface ModGalleryImage {
  url: string;
  title: string | null;
  description: string | null;
}

export interface ModProject {
  provider: ModProvider;
  id: string;
  title: string;
  author: string;
  summary: string;
  iconUrl: string | null;
  downloads: number;
  categories: string[];
  gameVersions: string[];
  loaders: string[];
  projectUrl: string | null;
  gallery: ModGalleryImage[];
}

export interface ModSearchResult {
  projects: ModProject[];
  totalHits: number;
  offset: number;
  limit: number;
}

export interface ModDependency {
  projectId: string | null;
  versionId: string | null;
  fileName: string | null;
  dependencyType: string;
}

export interface ModVersionFile {
  name: string;
  size: number | null;
  sha1: string | null;
}

export interface ModVersion {
  provider: ModProvider;
  id: string;
  projectId: string;
  name: string;
  versionNumber: string;
  releaseType: string;
  datePublished: string;
  gameVersions: string[];
  loaders: string[];
  files: ModVersionFile[];
  dependencies: ModDependency[];
  changelog: string | null;
}

export interface ModInstanceContext {
  instanceId: string;
  instanceName: string;
  gameDir: string;
  minecraftVersion: string | null;
  loader: string | null;
  versionId: string | null;
  sharedGameDirectory: boolean;
}

export interface InstalledMod {
  fileName: string;
  name: string;
  version: string | null;
  modId: string | null;
  enabled: boolean;
  size: number;
}

export interface ModDownloadProgress {
  downloadId: string;
  projectTitle: string;
  phase: string;
  currentFile: string;
  completedFiles: number;
  totalFiles: number;
  downloadedBytes: number;
  totalBytes: number;
}
