use std::{path::Path, time::Duration};

use futures_util::{SinkExt, StreamExt};
use native_tls::TlsConnector;
use serde::Serialize;
use tokio::{net::UdpSocket, task::JoinHandle};
use tokio_tungstenite::{connect_async_tls_with_config, tungstenite::Message, Connector};

use crate::{crypto, protocol};

#[derive(Debug)]
pub struct EncryptedFileHead {
    file_id: u32,
    expected_transport_bytes: u32,
    audio_nonce: [u8; 16],
    encrypted_file_key: [u8; 46],
    session_nonce: [u8; 16],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResult {
    pub path: String,
    pub chunks: u32,
    pub bytes_written: usize,
}

pub fn parse_encrypted_file_head(
    frame: &[u8],
    requested_file_id: u32,
) -> Result<EncryptedFileHead, String> {
    if frame.len() < 97 || frame.get(5..7) != Some(&[0x1A, 0x07]) {
        return Err("加密 1A/07 文件头不足 97 字节".to_string());
    }
    let file_id = u32::from_le_bytes(frame[9..13].try_into().unwrap());
    if file_id != requested_file_id {
        return Err(format!(
            "设备返回文件 {file_id}，但 requested {requested_file_id}；已中止传输"
        ));
    }
    if frame[95] != 0 {
        return Err(format!("设备文件头报告错误码 {}", frame[95]));
    }
    Ok(EncryptedFileHead {
        file_id,
        expected_transport_bytes: u32::from_le_bytes(frame[13..17].try_into().unwrap()),
        audio_nonce: frame[17..33].try_into().unwrap(),
        encrypted_file_key: frame[33..79].try_into().unwrap(),
        session_nonce: frame[79..95].try_into().unwrap(),
    })
}

fn build_file_request(file_id: u32) -> String {
    let mut request = vec![0x08, 0xEE, 0, 0, 0, 0x1A, 0x07, 0x12, 0x00];
    request.extend_from_slice(&0u32.to_le_bytes());
    request.extend_from_slice(&file_id.to_le_bytes());
    request.push(0x1B);
    hex::encode_upper(request)
}

fn start_udp_keepalive(host: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await else {
            return;
        };
        let destination = format!("{host}:32003");
        loop {
            let _ = socket
                .send_to(b"soundcore-keep-alive-unicast", &destination)
                .await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    })
}

pub async fn download_and_decrypt(
    endpoint: &str,
    host: &str,
    file_id: u32,
    output_path: &Path,
    session_key: &[u8; 32],
) -> Result<DownloadResult, String> {
    let parsed =
        url::Url::parse(endpoint).map_err(|error| format!("WebSocket 地址无效：{error}"))?;
    if !matches!(parsed.scheme(), "wss" | "ws") {
        return Err("录音豆端点必须使用 wss:// 或 ws://".to_string());
    }
    if parsed.host_str() != Some(host) {
        return Err("WebSocket 主机与 BLE SoftAP 响应不一致".to_string());
    }
    let connector = if parsed.scheme() == "wss" {
        let tls = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .map_err(|error| format!("配置本地设备 TLS 失败：{error}"))?;
        Some(Connector::NativeTls(tls))
    } else {
        Some(Connector::Plain)
    };
    let (mut socket, _) = connect_async_tls_with_config(endpoint, None, true, connector)
        .await
        .map_err(|error| format!("连接录音豆 WebSocket 失败：{error}"))?;
    socket
        .send(Message::Text(build_file_request(file_id).into()))
        .await
        .map_err(|error| format!("请求录音 {file_id} 失败：{error}"))?;

    let keepalive = start_udp_keepalive(host.to_string());
    let result = async {
        let mut decoder = protocol::FrameDecoder::default();
        let mut head: Option<EncryptedFileHead> = None;
        let mut file_key: Option<[u8; 32]> = None;
        let mut decrypted = Vec::new();
        let mut expected_sequence: Option<u32> = None;
        let mut chunks = 0u32;
        let mut completed = false;

        while let Some(message) = tokio::time::timeout(Duration::from_secs(15), socket.next())
            .await
            .map_err(|_| "录音豆传输超时".to_string())?
        {
            let message = message.map_err(|error| format!("WebSocket 传输失败：{error}"))?;
            match message {
                Message::Binary(bytes) => {
                    for frame in decoder.push(bytes.as_ref()) {
                        match (frame[5], frame[6]) {
                            (0x1A, 0x07) => {
                                let parsed_head = parse_encrypted_file_head(&frame, file_id)?;
                                file_key = Some(crypto::unwrap_file_key(
                                    session_key,
                                    &parsed_head.session_nonce,
                                    &parsed_head.encrypted_file_key,
                                )?);
                                head = Some(parsed_head);
                            }
                            (0x1A, 0x08 | 0x12) => {
                                let current_head = head
                                    .as_ref()
                                    .ok_or_else(|| "文件头之前收到了音频分片".to_string())?;
                                let key = file_key
                                    .as_ref()
                                    .ok_or_else(|| "文件密钥尚未初始化".to_string())?;
                                let payload = &frame[9..frame.len() - 1];
                                let (units, remainder) = payload.as_chunks::<166>();
                                if !remainder.is_empty() {
                                    return Err(format!(
                                        "音频载荷长度 {} 不是 166 的倍数",
                                        payload.len()
                                    ));
                                }
                                for unit in units {
                                    let sequence =
                                        u32::from_le_bytes(unit[0..4].try_into().unwrap());
                                    if expected_sequence
                                        .is_some_and(|expected| sequence != expected)
                                    {
                                        return Err(format!(
                                            "音频序号不连续：应为 {}，实际为 {sequence}",
                                            expected_sequence.unwrap()
                                        ));
                                    }
                                    decrypted.extend_from_slice(&crypto::decrypt_audio_chunk(
                                        &unit[5..165],
                                        key,
                                        &current_head.audio_nonce,
                                        sequence,
                                    )?);
                                    expected_sequence = Some(sequence.wrapping_add(1));
                                    chunks = chunks.saturating_add(1);
                                }
                            }
                            (0x1A, 0x0A) => {
                                completed = true;
                                break;
                            }
                            _ => {}
                        }
                    }
                    if completed {
                        break;
                    }
                }
                Message::Text(text) if text.eq_ignore_ascii_case("ping") => {
                    socket
                        .send(Message::Text("pong".into()))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Message::Ping(payload) => {
                    socket
                        .send(Message::Pong(payload))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        let head = head.ok_or_else(|| "传输结束但未收到文件头".to_string())?;
        if !completed {
            return Err("传输结束但未收到 1A/0A 完成帧".to_string());
        }
        if chunks == 0 {
            return Err("传输完成但未收到音频分片".to_string());
        }
        let expected_chunks = head.expected_transport_bytes as usize / 166;
        if expected_chunks > 0 && chunks as usize != expected_chunks {
            return Err(format!(
                "文件 {} 不完整：应有 {expected_chunks} 个分片，实际收到 {chunks}",
                head.file_id
            ));
        }
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| format!("创建导出目录失败：{error}"))?;
        }
        tokio::fs::write(output_path, &decrypted)
            .await
            .map_err(|error| format!("保存录音失败：{error}"))?;
        let _ = socket.send(Message::Text("FINISH".into())).await;
        Ok(DownloadResult {
            path: output_path.to_string_lossy().into_owned(),
            chunks,
            bytes_written: decrypted.len(),
        })
    }
    .await;
    keepalive.abort();
    result
}
