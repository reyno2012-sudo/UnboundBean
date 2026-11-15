import { invoke } from "@tauri-apps/api/core";

const DESKTOP_REQUIRED_MESSAGE =
  "当前是浏览器预览，无法访问蓝牙。请启动 UnboundBean 桌面应用：.\\scripts\\tauri-msvc.cmd dev";

function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
    return Promise.reject(new Error(DESKTOP_REQUIRED_MESSAGE));
  }
  return args === undefined ? invoke<T>(command) : invoke<T>(command, args);
}

export interface AiSummaryRequest {
  apiKey: string;
  baseUrl: string;
  model: string;
  transcript: string;
}

export interface DiscoveredDevice {
  id: string;
  name: string;
  address: string;
  rssi: number | null;
}

export interface ConnectedDevice {
  id: string;
  name: string;
  secure: boolean;
}

export interface RecordingInfo {
  fileId: number;
  sizeUnits: number;
  estimatedDurationSeconds: number;
}

export interface WifiExportSession {
  ssid: string;
  password: string;
  host: string;
  port: number;
  endpoint: string;
}

export interface DownloadRequest {
  endpoint: string;
  host: string;
  fileId: number;
  outputDirectory: string;
}

export interface DownloadResult {
  path: string;
  chunks: number;
  bytesWritten: number;
}

export function scanDevices(): Promise<DiscoveredDevice[]> {
  return invokeDesktop<DiscoveredDevice[]>("scan_devices");
}

export function connectDevice(deviceId: string): Promise<ConnectedDevice> {
  return invokeDesktop<ConnectedDevice>("connect_device", { deviceId });
}

export function listRecordings(): Promise<RecordingInfo[]> {
  return invokeDesktop<RecordingInfo[]>("list_recordings");
}

export function prepareWifiExport(): Promise<WifiExportSession> {
  return invokeDesktop<WifiExportSession>("prepare_wifi_export");
}

export function downloadRecording(request: DownloadRequest): Promise<DownloadResult> {
  return invokeDesktop<DownloadResult>("download_recording", { request });
}

export function summarizeTranscript(request: AiSummaryRequest): Promise<string> {
  return invokeDesktop<string>("summarize_transcript", { request });
}
