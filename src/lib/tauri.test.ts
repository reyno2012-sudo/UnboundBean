import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import {
  connectDevice,
  downloadRecording,
  listRecordings,
  prepareWifiExport,
  scanDevices,
  summarizeTranscript,
} from "./tauri";

describe("Tauri command bridge", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("maps the AI request to Rust camelCase command arguments", async () => {
    vi.mocked(invoke).mockResolvedValueOnce("# 摘要");

    await summarizeTranscript({
      apiKey: "test-key",
      baseUrl: "https://api.deepseek.com/v1",
      model: "deepseek-chat",
      transcript: "会议内容",
    });

    expect(invoke).toHaveBeenCalledWith("summarize_transcript", {
      request: {
        apiKey: "test-key",
        baseUrl: "https://api.deepseek.com/v1",
        model: "deepseek-chat",
        transcript: "会议内容",
      },
    });
  });

  it("uses stable command names for the device workflow", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    await scanDevices();
    await connectDevice("device-1");
    await listRecordings();
    await prepareWifiExport();
    await downloadRecording({
      endpoint: "wss://192.168.4.1:443",
      host: "192.168.4.1",
      fileId: 123,
      outputDirectory: "",
    });

    expect(invoke).toHaveBeenNthCalledWith(1, "scan_devices");
    expect(invoke).toHaveBeenNthCalledWith(2, "connect_device", { deviceId: "device-1" });
    expect(invoke).toHaveBeenNthCalledWith(3, "list_recordings");
    expect(invoke).toHaveBeenNthCalledWith(4, "prepare_wifi_export");
    expect(invoke).toHaveBeenNthCalledWith(5, "download_recording", {
      request: {
        endpoint: "wss://192.168.4.1:443",
        host: "192.168.4.1",
        fileId: 123,
        outputDirectory: "",
      },
    });
  });
});
