# D3200 协议实现说明

本文描述无界豆当前实现的互操作子集。公开逆向资料的权威来源仍是 [`tacshi/Soundcore/PROTOCOL.md`](https://github.com/tacshi/Soundcore/blob/main/PROTOCOL.md)。

## BLE 标识

| 角色 | UUID |
|---|---|
| D3200 服务 | `020cf5da-0000-1000-8000-00805f9b34fb` |
| 写特征 | `00007777-0000-1000-8000-00805f9b34fb` |
| 通知特征 | `00008888-0000-1000-8000-00805f9b34fb` |
| CCCD | `00002902-0000-1000-8000-00805f9b34fb` |

扫描首先使用服务 UUID 过滤；若平台没有返回过滤结果，再进行无过滤扫描并按服务或 `soundcore / D3200` 名称识别。

## 二进制帧

### App → 设备

```text
08 EE 00 00 00 | type | id | totalLength LE16 | payload | checksum
```

### 设备 → App

```text
09 FF 00 00 | status | type | id | totalLength LE16 | payload | checksum
```

校验和为前面所有字节之和的低 8 位。`FrameDecoder` 将来自多个 BLE 通知或 WebSocket 消息的字节流重组为完整帧；无效校验和帧会被丢弃。

## 已实现命令

| Type/ID | 方向 | 用途 |
|---|---|---|
| `2E/01` | 双向 | P-256 ECDH 密钥协商 |
| `1A/0E` | 双向 | 分页读取录音文件索引 |
| `1A/05` | 双向 | 发送 SoftAP SSID/密码并接收 IP/端口 |
| `1A/07` | WSS | 请求文件并接收加密文件头 |
| `1A/08`、`1A/12` | WSS | 接收音频分片 |
| `1A/0A` | WSS | 传输完成 |

## 会话密钥

1. App 生成临时 P-256 私钥和 65 字节未压缩公钥。
2. App 发送 `2E/01 + publicKey`。
3. 设备响应包含 65 字节设备公钥，以及 32 字节 ECDH 共享秘密证明。
4. App 本地执行 ECDH，并拒绝不匹配的证明。
5. 会话密钥：

```text
HKDF-SHA256(
  ikm  = sharedSecret,
  salt = 01 02 03,
  info = 01 02 03,
  len  = 32
)
```

不存在固定 AES-128 Key/IV。

## 文件头与文件密钥

加密 `1A/07` 文件头至少 97 字节：

| 绝对偏移 | 长度 | 字段 |
|---:|---:|---|
| 9 | 4 | `fileId`，LE32 |
| 13 | 4 | 传输长度，LE32 |
| 17 | 16 | 音频 nonce |
| 33 | 46 | 加密文件密钥材料 |
| 79 | 16 | 会话 nonce |
| 95 | 1 | 错误码 |

用 AES-256-CTR、会话密钥和会话 nonce 解包 46 字节密钥材料。明文必须严格等于：

```text
ASCII "soundcored3200" (14 bytes) || fileKey (32 bytes)
```

magic 不匹配即中止，不能把随机明文当成文件密钥。

## 音频单元解密

`1A/08` 或 `1A/12` 载荷由 166 字节单元组成：

```text
sequence LE32 | flags u8 | encryptedAudio 160 bytes | pad u8
```

每个单元独立构造 CTR 初始计数器：

```text
counter = audioNonce[0..12] || BE32(sequence * 10)
plaintext = AES-256-CTR(fileKey, counter, encryptedAudio)
```

无界豆要求序号连续，并使用文件头传输长度估算应有分片数。

## SoftAP 与 WebSocket

### BLE 请求载荷

```text
ssidLength u8 | ssid UTF-8 | passwordLength u8 | password UTF-8
```

设备响应偏移 `9..12` 的 IP 字节按逆序显示，`13..14` 为小端端口。用户加入热点后：

- 默认连接 `wss://IP:port`；设备证书为本地自签名证书。
- 文件请求通过 WebSocket 文本发送十六进制字符串。
- 设备通过 WebSocket binary 返回与 BLE 相同的 `09 FF` 帧。
- 每约 2 秒向 `IP:32003` 发送 `soundcore-keep-alive-unicast` UDP 数据。
- 完成后发送文本 `FINISH`。

## 仍需真机验证的部分

- 不同主固件版本是否都使用相同的 166/160 字节单元。
- SoftAP 端口和 TLS 行为在所有批次上的一致性。
- 原始 Opus 单元到标准 Ogg/Opus 容器的精确封装规则。
- 绑定状态和并发录音对历史文件下载的影响。

任何扩展都应先保存用户自有设备的 BLE/HCI 抓包，增加已知向量测试，再修改实现。
