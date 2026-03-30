use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;

use super::{prompt, ChunkCallback, LlmConfig, LlmProvider, PolishRequest, PolishResponse};

/// Cloud LLM provider that proxies requests through the talkmore-web API.
/// Auth token is passed via the api_key field in LlmConfig. Quota is enforced server-side.
pub struct CloudLlmProvider {
    client: Client,
}

impl Default for CloudLlmProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CloudLlmProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub fn with_client(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl LlmProvider for CloudLlmProvider {
    async fn polish(
        &self,
        config: &LlmConfig,
        req: &PolishRequest,
        on_chunk: Option<&ChunkCallback>,
    ) -> Result<PolishResponse> {
        if config.api_key.is_empty() {
            anyhow::bail!("Cloud LLM: session token is missing. Please sign in first.");
        }

        let has_selected_text = req
            .selected_text
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty());

        // Build V2-XML format user prompt
        let user_prompt = prompt::build_user_prompt(
            &req.raw_text,
            req.app_type,
            &req.dictionary,
            req.translate_enabled,
            &req.target_lang,
            has_selected_text,
        );

        let messages = prompt::build_messages(user_prompt, req.selected_text.as_deref());

        let api_base_url = crate::api_base_url();

        let body = serde_json::json!({
            "messages": messages,
            "stream": on_chunk.is_some()
        });

        let response = self
            .client
            .post(format!("{}/api/proxy/llm", api_base_url))
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            if status.as_u16() == 403 {
                let msg = serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|v| v["error"].as_str().map(String::from))
                    .unwrap_or_else(|| "LLM quota exceeded".to_string());
                anyhow::bail!("{}", msg);
            }
            let truncate_at = text
                .char_indices()
                .take_while(|&(i, _)| i < 200)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(text.len());
            let sanitized = &text[..truncate_at];
            anyhow::bail!("Cloud LLM error ({}): {}", status, sanitized);
        }

        if let Some(callback) = on_chunk {
            let mut full_text = String::new();
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            break;
                        }
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(content) = v["choices"][0]["delta"]["content"].as_str() {
                                if !content.is_empty() {
                                    full_text.push_str(content);
                                    callback(content);
                                }
                            }
                        }
                    }
                }
            }

            Ok(PolishResponse {
                polished_text: full_text,
            })
        } else {
            let v: serde_json::Value = response.json().await?;
            let text = v["text"]
                .as_str()
                .or_else(|| v["choices"][0]["message"]["content"].as_str())
                .unwrap_or("")
                .to_string();

            Ok(PolishResponse {
                polished_text: text,
            })
        }
    }

    fn name(&self) -> &str {
        "Cloud"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::test_support::spawn_json_server;
    use crate::llm::AppType;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn test_config() -> LlmConfig {
        LlmConfig {
            api_key: "session-token".to_string(),
            model: "cloud-model".to_string(),
            base_url: "unused".to_string(),
            max_tokens: 128,
            temperature: 0.3,
        }
    }

    fn test_request(selected_text: Option<&str>) -> PolishRequest {
        PolishRequest {
            raw_text: "翻译成英文".to_string(),
            app_type: AppType::General,
            dictionary: vec![],
            translate_enabled: false,
            target_lang: String::new(),
            selected_text: selected_text.map(str::to_string),
        }
    }

    #[tokio::test]
    async fn test_polish_sends_only_v2_xml_prompt_without_selected_text() {
        let _guard = env_lock().lock().expect("env mutex");
        let original = std::env::var("API_BASE_URL").ok();
        let (base_url, rx) = spawn_json_server(r#"{"text":"ok"}"#);
        std::env::set_var("API_BASE_URL", &base_url);

        let provider = CloudLlmProvider::new();
        let result = provider
            .polish(&test_config(), &test_request(None), None)
            .await;

        match original {
            Some(value) => std::env::set_var("API_BASE_URL", value),
            None => std::env::remove_var("API_BASE_URL"),
        }

        result.expect("polish request should succeed");

        let payload = rx.recv().expect("captured request payload");
        let messages = payload["messages"].as_array().expect("messages array");
        assert_eq!(messages.len(), 1);
        let prompt = messages[0]["content"].as_str().expect("v2 xml prompt");
        assert!(prompt.contains("<instructions>"));
        assert!(prompt.contains("<transcript>"));
        assert!(prompt.contains("翻译成英文"));
    }

    #[tokio::test]
    async fn test_polish_sends_selected_text_as_separate_user_message() {
        let _guard = env_lock().lock().expect("env mutex");
        let original = std::env::var("API_BASE_URL").ok();
        let (base_url, rx) = spawn_json_server(r#"{"text":"ok"}"#);
        std::env::set_var("API_BASE_URL", &base_url);

        let provider = CloudLlmProvider::new();
        let result = provider
            .polish(&test_config(), &test_request(Some("需要编辑的原文")), None)
            .await;

        match original {
            Some(value) => std::env::set_var("API_BASE_URL", value),
            None => std::env::remove_var("API_BASE_URL"),
        }

        result.expect("polish request should succeed");

        let payload = rx.recv().expect("captured request payload");
        let messages = payload["messages"].as_array().expect("messages array");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["content"], "[Selected Text]\n需要编辑的原文");
        let prompt = messages[1]["content"].as_str().expect("v2 xml prompt");
        assert!(prompt.contains("SELECTED TEXT MODE"));
        assert!(prompt.contains("翻译成英文"));
    }

    #[tokio::test]
    async fn test_polish_ignores_blank_selected_text() {
        let _guard = env_lock().lock().expect("env mutex");
        let original = std::env::var("API_BASE_URL").ok();
        let (base_url, rx) = spawn_json_server(r#"{"text":"ok"}"#);
        std::env::set_var("API_BASE_URL", &base_url);

        let provider = CloudLlmProvider::new();
        let result = provider
            .polish(&test_config(), &test_request(Some("   ")), None)
            .await;

        match original {
            Some(value) => std::env::set_var("API_BASE_URL", value),
            None => std::env::remove_var("API_BASE_URL"),
        }

        result.expect("polish request should succeed");

        let payload = rx.recv().expect("captured request payload");
        let messages = payload["messages"].as_array().expect("messages array");
        assert_eq!(messages.len(), 1);
        let prompt = messages[0]["content"].as_str().expect("v2 xml prompt");
        assert!(!prompt.contains("[Selected Text]"));
        assert!(!prompt.contains("SELECTED TEXT MODE"));
    }
}
