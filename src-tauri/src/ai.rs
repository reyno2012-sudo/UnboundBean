use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

const SYSTEM_PROMPT: &str = "你是一名严谨的会议秘书。请仅依据用户提供的转写内容整理 Markdown 纪要，依次包含：核心摘要、关键讨论点、明确决策、待办事项（负责人和截止时间未知时标注待确认）、风险与未决问题。不要编造转写中没有的信息。";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryRequest {
    api_key: String,
    base_url: String,
    model: String,
    transcript: String,
}

pub fn parse_summary_response(body: &Value) -> Result<String, String> {
    body.get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "AI response did not contain choices[0].message.content".to_string())
}

#[tauri::command]
pub async fn summarize_transcript(request: AiSummaryRequest) -> Result<String, String> {
    let api_key = request.api_key.trim();
    let base_url = request.base_url.trim().trim_end_matches('/');
    let model = request.model.trim();
    let transcript = request.transcript.trim();

    if api_key.is_empty() || model.is_empty() || transcript.is_empty() {
        return Err("API Key、模型和转写文本不能为空".to_string());
    }
    if !(base_url.starts_with("https://")
        || base_url.starts_with("http://127.0.0.1")
        || base_url.starts_with("http://localhost"))
    {
        return Err("API 地址必须使用 HTTPS（本机服务可使用 HTTP）".to_string());
    }

    let endpoint = if base_url.ends_with("/chat/completions") {
        base_url.to_string()
    } else {
        format!("{base_url}/chat/completions")
    };
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| format!("初始化 HTTP 客户端失败：{error}"))?;
    let response = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&json!({
            "model": model,
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": transcript}
            ],
            "temperature": 0.2
        }))
        .send()
        .await
        .map_err(|error| format!("AI 请求失败：{error}"))?;
    let status = response.status();
    let response_body = response
        .text()
        .await
        .map_err(|error| format!("读取 AI 响应失败：{error}"))?;
    if !status.is_success() {
        let excerpt: String = response_body.chars().take(400).collect();
        return Err(format!("AI API 返回 {status}：{excerpt}"));
    }
    let value: Value = serde_json::from_str(&response_body)
        .map_err(|error| format!("AI API 返回了无效 JSON：{error}"))?;
    parse_summary_response(&value)
}
