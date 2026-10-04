import { invoke } from "@tauri-apps/api/core";
import { Box, Sword, Pickaxe, Leaf, Flame } from "lucide-react";
export type Loader =
  { kind: "vanilla" } | { kind: "fabric" | "neoforge"; version: string };
export type Instance = {
  id: string;
  name: string;
  version: string;
  loader: Loader;
  javaPath: string | null;
  ramMb: number;
  jvmArgs: string;
  width: number;
  height: number;
  icon: string;
  lastPlayed: string | null;
  playtimeSeconds: number;
};
export type Account = {
  id: string;
  name: string;
  kind: "microsoft" | "offline";
  xuid: string;
  expiresAt: number;
};
export type Settings = {
  ramMb: number;
  javaPath: string | null;
  downloadConcurrency: number;
  showSnapshots: boolean;
  theme: string;
  minimizeOnLaunch: boolean;
  width: number;
  height: number;
  microsoftClientId: string;
};
export type Database = {
  instances: Instance[];
  accounts: Account[];
  activeAccount: string | null;
  settings: Settings;
  warnings: string[];
};
export type Version = { id: string; kind: string; releaseTime: string };
export type Mod = { name: string; enabled: boolean; size: number };
export type Status = {
  instanceId: string;
  phase: string;
  progress: number;
  message: string;
  exitCode: number | null;
};
export type DeviceCode = {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  message: string;
  interval: number;
  expiresIn: number;
};
export function InstanceGlyph({ icon }: { icon: string }) {
  const Icon =
    (
      { sword: Sword, pickaxe: Pickaxe, leaf: Leaf, flame: Flame } as Record<
        string,
        typeof Box
      >
    )[icon] || Box;
  return <Icon size={24} />;
}
export const api = <T,>(command: string, args?: Record<string, unknown>) =>
  invoke<T>(command, args);
