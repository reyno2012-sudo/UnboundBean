use std::{collections::HashMap, time::Duration};

use btleplug::{
    api::{
        Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter,
        WriteType,
    },
    platform::{Manager, Peripheral},
};
use futures_util::StreamExt;
use p256::{ecdh::EphemeralSecret, elliptic_curve::rand_core::OsRng, EncodedPoint, PublicKey};
use serde::Serialize;
use tauri::State;
use tokio::{sync::Mutex, time::timeout};
use uuid::Uuid;

use crate::{crypto, protocol};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Default)]
pub struct BleState {
    discovered: Mutex<HashMap<String, Peripheral>>,
    connection: Mutex<Option<Connection>>,
}

struct Connection {
    peripheral: Peripheral,
    write_characteristic: Characteristic,
    session_key: [u8; 32],
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredDevice {
    id: String,
    name: String,
    address: String,
    rssi: Option<i16>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedDevice {
    id: String,
    name: String,
    secure: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecordingInfo {
    pub file_id: u32,
    pub size_units: u32,
    pub estimated_duration_seconds: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiExportSession {
    ssid: String,
    password: String,
    host: String,
    port: u16,
    endpoint: String,
}

async fn first_adapter() -> Result<btleplug::platform::Adapter, String> {
    let manager = Manager::new()
        .await
        .map_err(|error| format!("初始化蓝牙失败：{error}"))?;
    manager
        .adapters()
        .await
        .map_err(|error| format!("枚举蓝牙适配器失败：{error}"))?
        .into_iter()
        .next()
        .ok_or_else(|| "未找到蓝牙适配器".to_string())
}

async fn collect_devices(
    adapter: &btleplug::platform::Adapter,
    service_uuid: Uuid,
) -> Result<Vec<(String, Peripheral, DiscoveredDevice)>, String> {
    let peripherals = adapter
        .peripherals()
        .await
        .map_err(|error| format!("枚举 BLE 设备失败：{error}"))?;
    let mut result = Vec::new();
    for peripheral in peripherals {
        let Some(properties) = peripheral
            .properties()
            .await
            .map_err(|error| format!("读取 BLE 广播失败：{error}"))?
        else {
            continue;
        };
        let name_matches = properties.local_name.as_deref().is_some_and(|name| {
            let lower = name.to_ascii_lowercase();
            lower.contains("soundcore") || lower.contains("d3200")
        });
        if !name_matches && !properties.services.contains(&service_uuid) {
            continue;
        }
        let id = peripheral.id().to_string();
        let device = DiscoveredDevice {
            id: id.clone(),
            name: properties
                .local_name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| "soundcore Work D3200".to_string()),
            address: properties.address.to_string(),
            rssi: properties.rssi,
        };
        result.push((id, peripheral, device));
    }
    result.sort_by_key(|item| std::cmp::Reverse(item.2.rssi));
    Ok(result)
}

#[tauri::command]
pub async fn scan_devices(state: State<'_, BleState>) -> Result<Vec<DiscoveredDevice>, String> {
    let adapter = first_adapter().await?;
    let service_uuid =
        Uuid::parse_str(protocol::SERVICE_UUID).map_err(|error| error.to_string())?;
    adapter
        .start_scan(ScanFilter {
            services: vec![service_uuid],
        })
        .await
        .map_err(|error| format!("启动 BLE 扫描失败：{error}"))?;
    tokio::time::sleep(Duration::from_secs(5)).await;
    let mut found = collect_devices(&adapter, service_uuid).await?;
    if found.is_empty() {
        let _ = adapter.stop_scan().await;
        adapter
            .start_scan(ScanFilter::default())
            .await
            .map_err(|error| format!("启动兼容扫描失败：{error}"))?;
        tokio::time::sleep(Duration::from_secs(3)).await;
        found = collect_devices(&adapter, service_uuid).await?;
    }
    let _ = adapter.stop_scan().await;

    let mut cache = state.discovered.lock().await;
    cache.clear();
    for (id, peripheral, _) in &found {
        cache.insert(id.clone(), peripheral.clone());
    }
    Ok(found.into_iter().map(|(_, _, device)| device).collect())
}

fn write_type(characteristic: &Characteristic) -> WriteType {
    if characteristic
        .properties
        .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
    {
        WriteType::WithoutResponse
    } else {
        WriteType::WithResponse
    }
}

async fn request_frame(
    connection: &Connection,
    command_type: u8,
    command_id: u8,
    payload: &[u8],
) -> Result<Vec<u8>, String> {
    let mut notifications = connection
        .peripheral
        .notifications()
        .await
        .map_err(|error| format!("打开 BLE 通知流失败：{error}"))?;
    connection
        .peripheral
        .write(
            &connection.write_characteristic,
            &protocol::build_command(command_type, command_id, payload),
            write_type(&connection.write_characteristic),
        )
        .await
        .map_err(|error| format!("BLE 命令 {command_type:02X}/{command_id:02X} 失败：{error}"))?;

    timeout(REQUEST_TIMEOUT, async {
        let mut decoder = protocol::FrameDecoder::default();
        while let Some(notification) = notifications.next().await {
            for frame in decoder.push(&notification.value) {
                if protocol::response_matches(&frame, command_type, command_id) {
                    return Ok(frame);
                }
            }
        }
        Err("BLE 通知流已关闭".to_string())
    })
    .await
    .map_err(|_| format!("BLE 命令 {command_type:02X}/{command_id:02X} 超时"))?
}

async fn negotiate_session_key(connection: &Connection) -> Result<[u8; 32], String> {
    let private_key = EphemeralSecret::random(&mut OsRng);
    let public_key = EncodedPoint::from(private_key.public_key());
    let response = request_frame(connection, 0x2E, 0x01, public_key.as_bytes()).await?;
    if !protocol::response_succeeded(&response) || response.len() < 107 {
        return Err("设备拒绝 ECDH 握手或返回帧过短".to_string());
    }
    let device_public_key = PublicKey::from_sec1_bytes(&response[9..74])
        .map_err(|error| format!("设备 P-256 公钥无效：{error}"))?;
    let shared = private_key.diffie_hellman(&device_public_key);
    let shared_bytes = shared.raw_secret_bytes();
    if response[74..106] != shared_bytes[..] {
        return Err("设备返回的 ECDH 证明与本地计算结果不一致".to_string());
    }
    crypto::derive_session_key(shared_bytes.as_slice())
}

#[tauri::command]
pub async fn connect_device(
    state: State<'_, BleState>,
    device_id: String,
) -> Result<ConnectedDevice, String> {
    let peripheral = state
        .discovered
        .lock()
        .await
        .get(&device_id)
        .cloned()
        .ok_or_else(|| "扫描缓存中没有该设备，请重新扫描".to_string())?;
    if !peripheral
        .is_connected()
        .await
        .map_err(|error| error.to_string())?
    {
        peripheral
            .connect()
            .await
            .map_err(|error| format!("连接录音豆失败：{error}"))?;
    }
    peripheral
        .discover_services()
        .await
        .map_err(|error| format!("发现 GATT 服务失败：{error}"))?;
    let write_uuid = Uuid::parse_str(protocol::WRITE_UUID).map_err(|error| error.to_string())?;
    let notify_uuid = Uuid::parse_str(protocol::NOTIFY_UUID).map_err(|error| error.to_string())?;
    let characteristics = peripheral.characteristics();
    let write_characteristic = characteristics
        .iter()
        .find(|characteristic| characteristic.uuid == write_uuid)
        .cloned()
        .ok_or_else(|| "未找到 D3200 写特征 7777".to_string())?;
    let notify_characteristic = characteristics
        .iter()
        .find(|characteristic| characteristic.uuid == notify_uuid)
        .cloned()
        .ok_or_else(|| "未找到 D3200 通知特征 8888".to_string())?;
    peripheral
        .subscribe(&notify_characteristic)
        .await
        .map_err(|error| format!("订阅 D3200 通知失败：{error}"))?;
    let name = peripheral
        .properties()
        .await
        .map_err(|error| error.to_string())?
        .and_then(|properties| properties.local_name)
        .unwrap_or_else(|| "soundcore Work D3200".to_string());
    let provisional = Connection {
        peripheral,
        write_characteristic,
        session_key: [0; 32],
    };
    let session_key = negotiate_session_key(&provisional).await?;
    *state.connection.lock().await = Some(Connection {
        session_key,
        ..provisional
    });
    Ok(ConnectedDevice {
        id: device_id,
        name,
        secure: true,
    })
}

pub fn parse_recording_list(frame: &[u8]) -> Result<Vec<RecordingInfo>, String> {
    if !protocol::is_valid_response(frame) || !protocol::response_matches(frame, 0x1A, 0x0E) {
        return Err("无效的 1A/0E 录音列表响应".to_string());
    }
    if !protocol::response_succeeded(frame) || frame.len() < 20 {
        return Err("设备拒绝录音列表请求".to_string());
    }
    let count = u16::from_le_bytes([frame[9], frame[10]]) as usize;
    let required = 11 + count * 8 + 8 + 1;
    if frame.len() < required {
        return Err(format!("录音列表被截断，应至少为 {required} 字节"));
    }
    let mut recordings = Vec::with_capacity(count);
    for index in 0..count {
        let offset = 11 + index * 8;
        let file_id = u32::from_le_bytes(frame[offset..offset + 4].try_into().unwrap());
        let size_units = u32::from_le_bytes(frame[offset + 4..offset + 8].try_into().unwrap());
        recordings.push(RecordingInfo {
            file_id,
            size_units,
            estimated_duration_seconds: size_units / 1_000,
        });
    }
    Ok(recordings)
}

#[tauri::command]
pub async fn list_recordings(state: State<'_, BleState>) -> Result<Vec<RecordingInfo>, String> {
    let guard = state.connection.lock().await;
    let connection = guard.as_ref().ok_or_else(|| "请先连接录音豆".to_string())?;
    let mut recordings = Vec::new();
    for page in 0u16..100 {
        let page_items = parse_recording_list(
            &request_frame(connection, 0x1A, 0x0E, &page.to_le_bytes()).await?,
        )?;
        let count = page_items.len();
        recordings.extend(page_items);
        if count < 10 {
            break;
        }
    }
    recordings.sort_by_key(|recording| std::cmp::Reverse(recording.file_id));
    recordings.dedup_by_key(|recording| recording.file_id);
    Ok(recordings)
}

pub fn parse_wifi_response(frame: &[u8]) -> Result<(String, u16), String> {
    if !protocol::is_valid_response(frame) || !protocol::response_matches(frame, 0x1A, 0x05) {
        return Err("无效的 1A/05 Wi-Fi 响应".to_string());
    }
    if !protocol::response_succeeded(frame) || frame.len() < 16 {
        return Err("设备拒绝 SoftAP 请求".to_string());
    }
    let host = format!("{}.{}.{}.{}", frame[12], frame[11], frame[10], frame[9]);
    let port = u16::from_le_bytes([frame[13], frame[14]]);
    if port == 0 {
        return Err("设备返回的 SoftAP 端口无效".to_string());
    }
    Ok((host, port))
}

#[tauri::command]
pub async fn prepare_wifi_export(state: State<'_, BleState>) -> Result<WifiExportSession, String> {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let ssid = format!("WiFi-{millis}");
    let password = millis.to_string();
    let mut payload = Vec::with_capacity(2 + ssid.len() + password.len());
    payload.push(u8::try_from(ssid.len()).map_err(|_| "SSID 过长")?);
    payload.extend_from_slice(ssid.as_bytes());
    payload.push(u8::try_from(password.len()).map_err(|_| "密码过长")?);
    payload.extend_from_slice(password.as_bytes());
    let guard = state.connection.lock().await;
    let connection = guard.as_ref().ok_or_else(|| "请先连接录音豆".to_string())?;
    let (host, port) =
        parse_wifi_response(&request_frame(connection, 0x1A, 0x05, &payload).await?)?;
    Ok(WifiExportSession {
        ssid,
        password,
        endpoint: format!("wss://{host}:{port}"),
        host,
        port,
    })
}

pub async fn session_key(state: &State<'_, BleState>) -> Result<[u8; 32], String> {
    state
        .connection
        .lock()
        .await
        .as_ref()
        .map(|connection| connection.session_key)
        .ok_or_else(|| "安全 BLE 会话不可用，请重新连接录音豆".to_string())
}
