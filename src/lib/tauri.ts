import { invoke } from "@tauri-apps/api/core";

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
  return invoke<DiscoveredDevice[]>("scan_devices");
}

export function connectDevice(deviceId: string): Promise<ConnectedDevice> {
  return invoke<ConnectedDevice>("connect_device", { deviceId });
}

export function listRecordings(): Promise<RecordingInfo[]> {
  return invoke<RecordingInfo[]>("list_recordings");
}

export function prepareWifiExport(): Promise<WifiExportSession> {
  return invoke<WifiExportSession>("prepare_wifi_export");
}

export function downloadRecording(request: DownloadRequest): Promise<DownloadResult> {
  return invoke<DownloadResult>("download_recording", { request });
}

export function summarizeTranscript(request: AiSummaryRequest): Promise<string> {
  return invoke<string>("summarize_transcript", { request });
}
