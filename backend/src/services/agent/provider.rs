use serde::{Deserialize, Serialize};
use async_trait::async_trait;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderConfig {
    Anthropic { api_key: String, model: String },
    OpenAI { api_key: String, model: String },
    Databricks { api_key: String, endpoint: String, model: String },
    LocalGGUF { model_path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: String,
}

#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn generate(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<AgentResponse, Box<dyn std::error::Error + Send + Sync>>;
}

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn generate(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<AgentResponse, Box<dyn std::error::Error + Send + Sync>> {
        // Convert messages to Anthropic format
        let anthropic_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": anthropic_messages,
            "max_tokens": 4096,
        });

        if let Some(tools) = tools {
            body["tools"] = serde_json::json!(tools);
        }

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Anthropic API error: {}", error_text).into());
        }

        let response_json: serde_json::Value = response.json().await?;

        // Parse response
        let content = response_json["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|c| c["text"].as_str())
            .unwrap_or("")
            .to_string();

        let tool_calls = if let Some(content_array) = response_json["content"].as_array() {
            content_array
                .iter()
                .filter(|c| c["type"] == "tool_use")
                .map(|c| ToolCall {
                    id: c["id"].as_str().unwrap_or("").to_string(),
                    name: c["name"].as_str().unwrap_or("").to_string(),
                    arguments: c["input"].clone(),
                })
                .collect()
        } else {
            vec![]
        };

        let finish_reason = response_json["stop_reason"]
            .as_str()
            .unwrap_or("end_turn")
            .to_string();

        Ok(AgentResponse {
            content,
            tool_calls,
            finish_reason,
        })
    }
}

pub struct OpenAIProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn generate(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<AgentResponse, Box<dyn std::error::Error + Send + Sync>> {
        let openai_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": openai_messages,
        });

        if let Some(tools) = tools {
            body["tools"] = serde_json::json!(tools);
        }

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("OpenAI API error: {}", error_text).into());
        }

        let response_json: serde_json::Value = response.json().await?;

        let choice = &response_json["choices"][0];
        let message = &choice["message"];

        let content = message["content"].as_str().unwrap_or("").to_string();

        let tool_calls = if let Some(calls) = message["tool_calls"].as_array() {
            calls
                .iter()
                .map(|c| {
                    let function = &c["function"];
                    ToolCall {
                        id: c["id"].as_str().unwrap_or("").to_string(),
                        name: function["name"].as_str().unwrap_or("").to_string(),
                        arguments: serde_json::from_str(function["arguments"].as_str().unwrap_or("{}"))
                            .unwrap_or(serde_json::json!({})),
                    }
                })
                .collect()
        } else {
            vec![]
        };

        let finish_reason = choice["finish_reason"]
            .as_str()
            .unwrap_or("stop")
            .to_string();

        Ok(AgentResponse {
            content,
            tool_calls,
            finish_reason,
        })
    }
}

pub fn create_provider(config: ProviderConfig) -> Box<dyn LLMProvider> {
    match config {
        ProviderConfig::Anthropic { api_key, model } => {
            Box::new(AnthropicProvider::new(api_key, model))
        }
        ProviderConfig::OpenAI { api_key, model } => {
            Box::new(OpenAIProvider::new(api_key, model))
        }
        ProviderConfig::Databricks { .. } => {
            // TODO: Implement Databricks provider
            Box::new(AnthropicProvider::new(
                "dummy".to_string(),
                "claude-3-5-sonnet-20241022".to_string(),
            ))
        }
        ProviderConfig::LocalGGUF { .. } => {
            // TODO: Implement local GGUF provider
            Box::new(AnthropicProvider::new(
                "dummy".to_string(),
                "claude-3-5-sonnet-20241022".to_string(),
            ))
        }
    }
}
