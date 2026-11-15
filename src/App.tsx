import { useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import {
  Bluetooth,
  Check,
  ChevronRight,
  CircleDot,
  Cpu,
  Download,
  FileAudio,
  KeyRound,
  LoaderCircle,
  Radio,
  RefreshCw,
  ScanLine,
  ShieldCheck,
  Sparkles,
  Unplug,
  Wifi,
} from "lucide-react";
import {
  connectDevice,
  downloadRecording,
  listRecordings,
  prepareWifiExport,
  scanDevices,
  summarizeTranscript,
  type ConnectedDevice,
  type DiscoveredDevice,
  type RecordingInfo,
  type WifiExportSession,
} from "./lib/tauri";

type BusyAction = "scan" | "connect" | "list" | "wifi" | "download" | "summary" | null;
type Provider = "deepseek" | "openai" | "custom";

const PROVIDERS: Record<Provider, { baseUrl: string; model: string }> = {
  deepseek: { baseUrl: "https://api.deepseek.com/v1", model: "deepseek-chat" },
  openai: { baseUrl: "https://api.openai.com/v1", model: "gpt-4.1-mini" },
  custom: { baseUrl: "", model: "" },
};

function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "发生未知错误";
}

function formatDuration(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const rest = Math.round(seconds % 60);
  return `${minutes.toString().padStart(2, "0")}:${rest.toString().padStart(2, "0")}`;
}

export default function App() {
  const [busy, setBusy] = useState<BusyAction>(null);
  const [status, setStatus] = useState("控制台就绪 · 等待扫描 D3200");
  const [devices, setDevices] = useState<DiscoveredDevice[]>([]);
  const [connected, setConnected] = useState<ConnectedDevice | null>(null);
  const [recordings, setRecordings] = useState<RecordingInfo[]>([]);
  const [selectedFileId, setSelectedFileId] = useState<number | null>(null);
  const [wifiSession, setWifiSession] = useState<WifiExportSession | null>(null);
  const [outputDirectory, setOutputDirectory] = useState("");
  const [lastExport, setLastExport] = useState("");

  const [provider, setProvider] = useState<Provider>("deepseek");
  const [baseUrl, setBaseUrl] = useState(PROVIDERS.deepseek.baseUrl);
  const [model, setModel] = useState(PROVIDERS.deepseek.model);
  const [apiKey, setApiKey] = useState("");
  const [transcript, setTranscript] = useState("");
  const [summary, setSummary] = useState("");

  const selectedRecording = useMemo(
    () => recordings.find((recording) => recording.fileId === selectedFileId) ?? null,
    [recordings, selectedFileId],
  );

  async function run<T>(action: Exclude<BusyAction, null>, start: string, task: () => Promise<T>) {
    setBusy(action);
    setStatus(start);
    try {
      return await task();
    } catch (error) {
      setStatus(`错误 · ${errorText(error)}`);
      throw error;
    } finally {
      setBusy(null);
    }
  }

  async function handleScan() {
    try {
      const result = await run("scan", "BLE 扫描中 · 监听 D3200 服务广播", scanDevices);
      setDevices(result);
      setStatus(result.length ? `发现 ${result.length} 台兼容设备` : "未发现设备 · 请唤醒录音豆后重试");
    } catch {
      // run() already surfaced a user-facing error.
    }
  }

  async function handleConnect(device: DiscoveredDevice) {
    try {
      const result = await run("connect", `连接 ${device.name} · 协商 P-256 会话密钥`, () => connectDevice(device.id));
      setConnected(result);
      setWifiSession(null);
      setStatus(`安全链路已建立 · ${result.name}`);
      await handleList();
    } catch {
      // run() already surfaced a user-facing error.
    }
  }

  async function handleList() {
    try {
      const result = await run("list", "读取设备录音索引 · 1A/0E", listRecordings);
      setRecordings(result);
      if (result.length && !result.some((item) => item.fileId === selectedFileId)) {
        setSelectedFileId(result[0].fileId);
      }
      setStatus(`索引同步完成 · ${result.length} 条录音`);
    } catch {
      // run() already surfaced a user-facing error.
    }
  }

  async function handlePrepareWifi() {
    try {
      const result = await run("wifi", "正在请求录音豆启动 SoftAP · 1A/05", prepareWifiExport);
      setWifiSession(result);
      setStatus(`热点已启动 · 请在系统 Wi-Fi 中加入 ${result.ssid}`);
    } catch {
      // run() already surfaced a user-facing error.
    }
  }

  async function handleDownload() {
    if (!wifiSession || selectedFileId === null) return;
    try {
      const result = await run("download", "WSS 拉取中 · 校验序号并解密 AES-256-CTR", () =>
        downloadRecording({
          endpoint: wifiSession.endpoint,
          host: wifiSession.host,
          fileId: selectedFileId,
          outputDirectory,
        }),
      );
      setLastExport(result.path);
      setStatus(`导出完成 · ${result.chunks} 个分片 / ${result.bytesWritten.toLocaleString()} 字节`);
    } catch {
      // run() already surfaced a user-facing error.
    }
  }

  async function handleSummary() {
    if (!apiKey.trim() || !transcript.trim() || !baseUrl.trim() || !model.trim()) {
      setStatus("AI 配置不完整 · 请填写 Key、端点、模型和转写文本");
      return;
    }
    try {
      const result = await run("summary", "AI 整理中 · 提取摘要、决策与待办", () =>
        summarizeTranscript({ apiKey, baseUrl, model, transcript }),
      );
      setSummary(result);
      setStatus("AI 结构化纪要已生成");
    } catch {
      // run() already surfaced a user-facing error.
    }
  }

  function handleProviderChange(next: Provider) {
    setProvider(next);
    setBaseUrl(PROVIDERS[next].baseUrl);
    setModel(PROVIDERS[next].model);
  }

  return (
    <main className="app-shell relative">
      <div className="ambient ambient-a" />
      <div className="ambient ambient-b" />

      <header className="topbar reveal reveal-1">
        <div className="brand-mark"><Radio size={23} strokeWidth={1.6} /></div>
        <div>
          <p className="eyebrow">UNBOUND LOCAL CONSOLE / D3200</p>
          <h1>UnboundBean <span>无界豆</span></h1>
        </div>
        <div className="topbar-status">
          <span className={connected ? "live-dot" : "idle-dot"} />
          {connected ? "SECURE LINK" : "STANDBY"}
        </div>
      </header>

      <section className="status-strip reveal reveal-2" aria-live="polite">
        <CircleDot size={14} />
        <span>{status}</span>
        <span className="protocol-label">BLE · ECDH · WSS · AES-CTR</span>
      </section>

      <div className="workspace">
        <section className="device-console reveal reveal-3">
          <div className="section-heading">
            <div>
              <p className="section-index">01 / DEVICE TRANSPORT</p>
              <h2>录音豆数据链路</h2>
            </div>
            <button className="icon-button" onClick={handleScan} disabled={busy !== null} title="扫描设备">
              {busy === "scan" ? <LoaderCircle className="spin" size={18} /> : <ScanLine size={18} />}
            </button>
          </div>

          {!connected ? (
            <div className="scan-zone">
              <Bluetooth size={34} strokeWidth={1.25} />
              <strong>{devices.length ? "选择发现的设备" : "等待 BLE 广播"}</strong>
              <p>按服务 UUID 精确过滤，不依赖可能缺失的设备名称。</p>
              {!devices.length && (
                <button className="primary-action" onClick={handleScan} disabled={busy !== null}>
                  {busy === "scan" ? <LoaderCircle className="spin" size={17} /> : <ScanLine size={17} />}
                  开始扫描
                </button>
              )}
            </div>
          ) : (
            <div className="connected-card">
              <div className="device-orbit"><Bluetooth size={22} /></div>
              <div>
                <span>CONNECTED DEVICE</span>
                <strong>{connected.name}</strong>
                <small>{connected.id}</small>
              </div>
              <ShieldCheck className="secure-icon" size={24} />
            </div>
          )}

          {!connected && devices.length > 0 && (
            <div className="device-list">
              {devices.map((device) => (
                <button key={device.id} onClick={() => handleConnect(device)} disabled={busy !== null}>
                  <span className="signal-bars"><i /><i /><i /></span>
                  <span><strong>{device.name}</strong><small>{device.address}</small></span>
                  <em>{device.rssi === null ? "—" : `${device.rssi} dBm`}</em>
                  <ChevronRight size={17} />
                </button>
              ))}
            </div>
          )}

          <div className="pipeline">
            <div className={connected ? "pipeline-step done" : "pipeline-step active"}>
              <span>{connected ? <Check size={14} /> : "1"}</span>
              <div><strong>BLE + ECDH</strong><small>GATT 7777 / 8888</small></div>
            </div>
            <div className={recordings.length ? "pipeline-step done" : connected ? "pipeline-step active" : "pipeline-step"}>
              <span>{recordings.length ? <Check size={14} /> : "2"}</span>
              <div><strong>录音索引</strong><small>1A / 0E</small></div>
            </div>
            <div className={wifiSession ? "pipeline-step done" : recordings.length ? "pipeline-step active" : "pipeline-step"}>
              <span>{wifiSession ? <Check size={14} /> : "3"}</span>
              <div><strong>SoftAP</strong><small>WSS transport</small></div>
            </div>
          </div>

          <div className="recording-header">
            <div><FileAudio size={17} /><span>设备录音</span><b>{recordings.length}</b></div>
            <button onClick={handleList} disabled={!connected || busy !== null}>
              <RefreshCw className={busy === "list" ? "spin" : ""} size={14} /> 刷新
            </button>
          </div>

          <div className="recording-list">
            {recordings.length ? recordings.map((recording, index) => (
              <button
                key={recording.fileId}
                className={selectedFileId === recording.fileId ? "selected" : ""}
                onClick={() => setSelectedFileId(recording.fileId)}
              >
                <span className="record-number">{String(index + 1).padStart(2, "0")}</span>
                <div><strong>REC-{recording.fileId}</strong><small>{recording.sizeUnits.toLocaleString()} units</small></div>
                <time>{formatDuration(recording.estimatedDurationSeconds)}</time>
              </button>
            )) : (
              <div className="empty-list"><Unplug size={19} />连接后读取设备索引</div>
            )}
          </div>

          <div className="export-panel">
            {!wifiSession ? (
              <button className="secondary-action" onClick={handlePrepareWifi} disabled={!selectedRecording || busy !== null}>
                {busy === "wifi" ? <LoaderCircle className="spin" size={17} /> : <Wifi size={17} />}
                启动 SoftAP 导出
              </button>
            ) : (
              <>
                <div className="wifi-ticket">
                  <div><Wifi size={18} /><span>JOIN NETWORK</span></div>
                  <strong>{wifiSession.ssid}</strong>
                  <code>{wifiSession.password}</code>
                  <small>{wifiSession.endpoint}</small>
                </div>
                <label className="field-label">
                  输出目录 <span>留空则保存到文档目录</span>
                  <input value={outputDirectory} onChange={(event) => setOutputDirectory(event.target.value)} placeholder="C:\Users\...\Documents\UnboundBean" />
                </label>
                <button className="primary-action download-action" onClick={handleDownload} disabled={busy !== null}>
                  {busy === "download" ? <LoaderCircle className="spin" size={17} /> : <Download size={17} />}
                  拉取并解密 REC-{selectedFileId}
                </button>
              </>
            )}
            {lastExport && <p className="export-path"><Check size={13} />{lastExport}</p>}
          </div>
        </section>

        <section className="ai-console reveal reveal-4">
          <div className="section-heading">
            <div>
              <p className="section-index">02 / INTELLIGENCE LAYER</p>
              <h2>结构化纪要引擎</h2>
            </div>
            <div className="ai-chip"><Cpu size={15} /> API COMPATIBLE</div>
          </div>

          <div className="provider-tabs">
            {(["deepseek", "openai", "custom"] as Provider[]).map((item) => (
              <button key={item} className={provider === item ? "active" : ""} onClick={() => handleProviderChange(item)}>
                {item === "custom" ? "自定义" : item}
              </button>
            ))}
          </div>

          <div className="config-grid">
            <label className="field-label wide">
              <span><KeyRound size={13} /> API KEY · 仅驻留当前内存</span>
              <input type="password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder="sk-••••••••••••••••" autoComplete="off" />
            </label>
            <label className="field-label">
              API 端点
              <input value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} placeholder="https://api.example.com/v1" />
            </label>
            <label className="field-label">
              模型
              <input value={model} onChange={(event) => setModel(event.target.value)} placeholder="model-name" />
            </label>
          </div>

          <label className="field-label transcript-label">
            <span>TRANSCRIPT INPUT <em>{transcript.length.toLocaleString()} chars</em></span>
            <textarea
              value={transcript}
              onChange={(event) => setTranscript(event.target.value)}
              placeholder="粘贴本地 Whisper、Soniox 或其他转写工具生成的文本……"
            />
          </label>

          <button className="summary-action" onClick={handleSummary} disabled={busy !== null || !transcript.trim()}>
            {busy === "summary" ? <LoaderCircle className="spin" size={18} /> : <Sparkles size={18} />}
            生成结构化纪要
            <span>SUMMARY / DECISIONS / ACTIONS</span>
          </button>

          <div className="summary-output">
            <div className="output-heading">
              <span>AI OUTPUT</span>
              <i className={summary ? "ready" : ""}>{summary ? "READY" : "EMPTY"}</i>
            </div>
            {summary ? (
              <article><ReactMarkdown>{summary}</ReactMarkdown></article>
            ) : (
              <div className="summary-placeholder">
                <Sparkles size={24} strokeWidth={1.2} />
                <p>摘要、关键讨论点与待办将在这里展开。</p>
                <small>Markdown rendering enabled</small>
              </div>
            )}
          </div>
        </section>
      </div>

      <footer className="footer-line reveal reveal-4">
        <span>LOCAL-FIRST / NO CLOUD LOCK-IN</span>
        <span>PROTOCOL BUILD 2026.11</span>
      </footer>
    </main>
  );
}
