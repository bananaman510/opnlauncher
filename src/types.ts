export interface MinecraftVersion {
  id: string;
  kind: string;
  versionJson: string;
}

export interface MinecraftInstallation {
  root: string;
  versions: MinecraftVersion[];
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
