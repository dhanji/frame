use super::provider::{LLMProvider, Message, ToolCall};
use super::tools::ToolRegistry;
use serde_json::Value;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    pub tool: String,
    pub args: Value,
    pub result: Value,
}

pub struct AgentEngine {
    provider: Box<dyn LLMProvider>,
    tool_registry: Arc<ToolRegistry>,
    max_context_tokens: usize,
}

impl AgentEngine {
    pub fn new(provider: Box<dyn LLMProvider>, tool_registry: Arc<ToolRegistry>) -> Self {
        Self::with_context_limit(provider, tool_registry, 100000)
    }

    pub fn with_context_limit(provider: Box<dyn LLMProvider>, tool_registry: Arc<ToolRegistry>, max_tokens: usize) -> Self {
        Self {
            provider,
            tool_registry,
            max_context_tokens: max_tokens,
        }
    }

    pub async fn process_message(
        &self,
        user_message: String,
        conversation_history: Vec<Message>,
    ) -> Result<(String, Vec<ToolCallInfo>), Box<dyn std::error::Error + Send + Sync>> {
        let mut messages = conversation_history;
        let mut all_tool_calls = Vec::new();
        
        // Add system message if this is the first message
        if messages.is_empty() {
            messages.push(Message {
                role: "system".to_string(),
                content: "You are a helpful AI assistant. Always format your responses using Markdown for better readability. Use headers, lists, code blocks, and emphasis where appropriate.".to_string(),
            });
        }
        
        messages.push(Message {
            role: "user".to_string(),
            content: user_message,
        });

        // Check if we need to summarize context
        if self.estimate_tokens(&messages) > self.max_context_tokens {
            messages = self.summarize_context(messages).await?;
        }

        let mut iterations = 0;
        let max_iterations = 25; // Allow more iterations for complex tasks

        loop {
            iterations += 1;
            if iterations > max_iterations {
                return Err("Max iterations reached".into());
            }

            // Get tool definitions in Anthropic format
            let tools = self.tool_registry.to_anthropic_format();

            // Call LLM
            let response = self.provider.generate(messages.clone(), Some(tools)).await?;

            // If no tool calls, return the response
            if response.tool_calls.is_empty() {
                log::info!("Agent completed after {} iterations with {} tool calls", iterations, all_tool_calls.len());
                return Ok((response.content, all_tool_calls));
            }

            log::info!("Iteration {}: Agent making {} tool calls", iterations, response.tool_calls.len());

            // Execute tool calls
            let mut tool_results = Vec::new();
            for tool_call in &response.tool_calls {
                let result = self.execute_tool(tool_call).await?;
                tool_results.push((tool_call.id.clone(), tool_call.name.clone(), result));
            }

            // Track tool calls for response
            for (_, tool_name, result) in &tool_results {
                all_tool_calls.push(ToolCallInfo {
                    tool: tool_name.clone(),
                    args: serde_json::json!({}), // We don't have args here, would need to track from tool_call
                    result: result.clone(),
                });
            }

            // Add assistant message with tool calls
            messages.push(Message {
                role: "assistant".to_string(),
                content: response.content.clone(),
            });

            // Add tool results as user messages
            for (tool_id, tool_name, result) in tool_results {
                messages.push(Message {
                    role: "user".to_string(),
                    content: format!(
                        "Tool '{}' (id: {}) returned: {}",
                        tool_name,
                        tool_id,
                        serde_json::to_string_pretty(&result)?
                    ),
                });
            }
        }
    }

    async fn execute_tool(
        &self,
        tool_call: &ToolCall,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let tool = self
            .tool_registry
            .get(&tool_call.name)
            .ok_or(format!("Tool '{}' not found", tool_call.name))?;

        tool.execute(tool_call.arguments.clone()).await
    }

    /// Estimate token count (rough approximation: 1 token ≈ 4 characters)
    fn estimate_tokens(&self, messages: &[Message]) -> usize {
        let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
        total_chars / 4
    }

    /// Summarize conversation history when approaching token limit
    async fn summarize_context(
        &self,
        messages: Vec<Message>,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Context window approaching limit, summarizing conversation...");

        // Keep the first message (system context) and last few messages
        let keep_recent = 5;
        let messages_len = messages.len();

        if messages_len <= keep_recent + 1 {
            return Ok(messages);
        }

        // Extract messages to summarize (middle portion)
        let to_summarize = &messages[1..messages_len.saturating_sub(keep_recent)];
        let recent = &messages[messages_len.saturating_sub(keep_recent)..];

        // Create summarization prompt
        let conversation_text = to_summarize
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let summary_prompt = format!(
            "Please provide a concise summary of the following conversation, \
             preserving key information, decisions, and context:\n\n{}",
            conversation_text
        );

        // Generate summary (without tools)
        let summary_response = self
            .provider
            .generate(vec![Message {
                role: "user".to_string(),
                content: summary_prompt,
            }], None)
            .await?;

        // Construct new message history with summary
        let mut new_messages = vec![Message {
            role: "assistant".to_string(),
            content: format!("[Previous conversation summary]: {}", summary_response.content),
        }];
        new_messages.extend_from_slice(recent);

        Ok(new_messages)
    }
}
