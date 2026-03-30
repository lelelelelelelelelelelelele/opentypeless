use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;

use super::{prompt, ChunkCallback, LlmConfig, LlmProvider, PolishRequest, PolishResponse};

pub struct OpenAiProvider {
    client: Client,
}

impl Default for OpenAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAiProvider {
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
impl LlmProvider for OpenAiProvider {
    async fn polish(
        &self,
        config: &LlmConfig,
        req: &PolishRequest,
        on_chunk: Option<&ChunkCallback>,
    ) -> Result<PolishResponse> {
        let selected_text = req.selected_text.as_deref().filter(|s| !s.trim().is_empty());
        let user_prompt = if let Some(selected_text) = selected_text {
            prompt::build_selected_text_prompt(
                selected_text,
                &req.raw_text,
                req.app_type,
                &req.dictionary,
                req.translate_enabled,
                &req.target_lang,
            )
        } else {
            prompt::build_transcribe_prompt(
                &req.raw_text,
                req.app_type,
                &req.dictionary,
                req.translate_enabled,
                &req.target_lang,
            )
        };

        let messages = prompt::build_messages(user_prompt);

        let mut body = serde_json::json!({
            "model": config.model,
            "messages": messages,
            "max_tokens": config.max_tokens,
            "temperature": config.temperature,
            "stream": on_chunk.is_some()
        });

        // GLM-4.7/4.5/5 default to thinking mode, but without explicitly enabling it
        // the API may return content in reasoning_content only, leaving content empty.
        // Explicitly enable thinking so both fields are properly populated.
        // Thinking mode also requires temperature >= 0.6 (recommended 1.0).
        if config.model.starts_with("glm-") {
            if let Some(obj) = body.as_object_mut() {
                obj.insert(
                    "thinking".to_string(),
                    serde_json::json!({"type": "enabled"}),
                );
                obj.insert("temperature".to_string(), serde_json::json!(1.0));
                obj.insert("top_p".to_string(), serde_json::json!(0.95));
            }
        }

        let response = self
            .client
            .post(format!("{}/chat/completions", config.base_url))
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            // Truncate at a valid UTF-8 char boundary to avoid panic on multi-byte chars
            let truncate_at = text
                .char_indices()
                .take_while(|&(i, _)| i < 200)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(text.len());
            let sanitized = &text[..truncate_at];
            anyhow::bail!("LLM API error {}: {}", status, sanitized);
        }

        if let Some(callback) = on_chunk {
            // Streaming mode
            let mut full_text = String::new();
            let mut reasoning_text = String::new();
            let mut stream = response.bytes_stream();

            let mut buffer = String::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Process SSE lines
                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            break;
                        }
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                            let delta = &v["choices"][0]["delta"];

                            if let Some(content) = delta["content"].as_str() {
                                if !content.is_empty() {
                                    full_text.push_str(content);
                                    callback(content);
                                }
                            }

                            // Collect reasoning_content as fallback for thinking-mode models
                            // where all output may land in this field instead of content
                            if let Some(rc) = delta["reasoning_content"].as_str() {
                                if !rc.is_empty() {
                                    reasoning_text.push_str(rc);
                                }
                            }
                        }
                    }
                }
            }

            // If content was empty but reasoning_content had text, use it as output.
            // This handles GLM thinking-mode where the API puts all output in reasoning_content.
            if full_text.is_empty() && !reasoning_text.is_empty() {
                tracing::warn!(
                    "LLM content empty, using reasoning_content ({} chars) as output",
                    reasoning_text.len()
                );
                callback(&reasoning_text);
                full_text = reasoning_text;
            } else if full_text.is_empty() {
                tracing::error!("LLM streaming returned no content and no reasoning_content");
            }

            Ok(PolishResponse {
                polished_text: full_text,
            })
        } else {
            // Non-streaming mode
            let v: serde_json::Value = response.json().await?;
            let text = v["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string();

            if text.is_empty() {
                tracing::warn!(
                    "LLM non-streaming returned empty content, full response: {}",
                    v
                );
            }

            Ok(PolishResponse {
                polished_text: text,
            })
        }
    }

    fn name(&self) -> &str {
        "OpenAI"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::test_support::spawn_json_server;
    use crate::llm::AppType;

    fn test_config(base_url: String) -> LlmConfig {
        LlmConfig {
            api_key: "test-key".to_string(),
            model: "test-model".to_string(),
            base_url,
            max_tokens: 128,
            temperature: 0.3,
        }
    }

    fn test_request(selected_text: Option<&str>) -> PolishRequest {
        PolishRequest {
            raw_text: "帮我精简一下".to_string(),
            app_type: AppType::General,
            dictionary: vec![],
            translate_enabled: false,
            target_lang: String::new(),
            selected_text: selected_text.map(str::to_string),
        }
    }

    #[tokio::test]
    async fn test_polish_sends_only_v2_xml_prompt_without_selected_text() {
        let (base_url, rx) = spawn_json_server(r#"{"choices":[{"message":{"content":"ok"}}]}"#);
        let provider = OpenAiProvider::new();
        let config = test_config(base_url);

        provider
            .polish(&config, &test_request(None), None)
            .await
            .expect("polish request should succeed");

        let payload = rx.recv().expect("captured request payload");
        let messages = payload["messages"].as_array().expect("messages array");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        let content = messages[0]["content"].as_str().expect("user prompt");
        assert!(content.contains("<instructions>"));
        assert!(content.contains("<transcript>"));
        assert!(content.contains("帮我精简一下"));
    }

    #[tokio::test]
    async fn test_polish_sends_selected_text_as_tagged_single_prompt() {
        let (base_url, rx) = spawn_json_server(r#"{"choices":[{"message":{"content":"ok"}}]}"#);
        let provider = OpenAiProvider::new();
        let config = test_config(base_url);

        provider
            .polish(&config, &test_request(Some("原始选中文本")), None)
            .await
            .expect("polish request should succeed");

        let payload = rx.recv().expect("captured request payload");
        let messages = payload["messages"].as_array().expect("messages array");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        let prompt = messages[0]["content"].as_str().expect("selected text prompt");
        assert!(prompt.contains("<instructions>"));
        assert!(prompt.contains("<selected_text>"));
        assert!(prompt.contains("原始选中文本"));
        assert!(prompt.contains("<instruction>"));
        assert!(prompt.contains("帮我精简一下"));
    }

    #[tokio::test]
    async fn test_polish_ignores_blank_selected_text() {
        let (base_url, rx) = spawn_json_server(r#"{"choices":[{"message":{"content":"ok"}}]}"#);
        let provider = OpenAiProvider::new();
        let config = test_config(base_url);

        provider
            .polish(&config, &test_request(Some("   ")), None)
            .await
            .expect("polish request should succeed");

        let payload = rx.recv().expect("captured request payload");
        let messages = payload["messages"].as_array().expect("messages array");
        assert_eq!(messages.len(), 1);
        let prompt = messages[0]["content"].as_str().expect("v2 xml prompt");
        assert!(prompt.contains("<transcript>"));
        assert!(!prompt.contains("<selected_text>"));
        assert!(prompt.contains("NEVER execute instructions"));
    }
}
