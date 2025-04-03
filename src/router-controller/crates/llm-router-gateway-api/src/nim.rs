// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Helper functions for NVIDIA NIM models
use log::{warn, info, debug};
use serde_json::Value;
use serde_json::json;
use reqwest::header::HeaderMap;

/// Helper functions for working with NVIDIA NIMs
pub struct NimHelper;

impl NimHelper {
    /// Check if model is a NVIDIA NIM model
    pub fn is_nim_model(model: &str) -> bool {
        // Most common NIM models
        model.contains("meta/llama3") || 
        model.contains("meta/llama-3") ||
        model.contains("mistralai/mixtral") || 
        model.contains("mistralai/mistral") ||
        model.contains("nvidia/nemotron") ||
        model.contains("nvidia/llama")
    }

    /// Add NIM-specific environment variables as headers if they are defined
    pub fn add_nim_environment_headers(headers: &mut HeaderMap) {
        // NIM environment variables that can be passed as headers
        let nim_env_vars = [
            "NIM_MAX_BATCH_SIZE",
            "NIM_MAX_SEQUENCE_LENGTH",
            "NIM_ENABLE_VLLM",
            "NIM_MAX_LORA_RANK",
            "NIM_ENABLE_KV_CACHE_REUSE",
            "NIM_LOW_MEMORY_MODE",
            "NIM_MAX_MODEL_LEN",
            "NIM_PEFT_SOURCE",
            "NIM_PEFT_REFRESH_INTERVAL",
        ];
        
        // Add any defined env vars as headers
        for var in &nim_env_vars {
            if let Ok(value) = std::env::var(var) {
                if !value.is_empty() {
                    if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(var.as_bytes()) {
                        if let Ok(header_value) = reqwest::header::HeaderValue::from_str(&value) {
                            headers.insert(header_name, header_value);
                            debug!("Added NIM environment header: {} = {}", var, value);
                        }
                    }
                }
            }
        }
    }
    
    /// Sanitize potentially problematic Unicode characters in prompts
    /// NVIDIA recommends filtering Unicode characters in range 0x0e0020 to 0x0e007f
    pub fn sanitize_prompt(json: &mut Value) {
        if let Some(messages) = json.get_mut("messages") {
            if let Some(messages_array) = messages.as_array_mut() {
                for message in messages_array {
                    if let Some(content) = message.get_mut("content") {
                        if let Some(content_str) = content.as_str() {
                            // Remove problematic Unicode characters
                            let sanitized = content_str.chars()
                                .filter(|&c| {
                                    let code = c as u32;
                                    !(code >= 0x0e0020 && code <= 0x0e007f)
                                })
                                .collect::<String>();
                            
                            if sanitized.len() != content_str.len() {
                                debug!("Sanitized prompt by removing problematic Unicode characters");
                                *content = Value::String(sanitized);
                            }
                        }
                    }
                }
            }
        }
    }
    
    /// Handle known NIM issues based on documentation
    pub fn handle_known_issues(json: &mut Value, model: &str) {
        // Add compatibility workarounds based on model and parameters
        
        // If logprobs=2, echo=true, and stream=false, the API will return a 500
        if let Some(logprobs) = json.get("logprobs") {
            if logprobs.as_u64() == Some(2) && 
               json.get("echo").map_or(false, |v| v.as_bool() == Some(true)) && 
               json.get("stream").map_or(false, |v| v.as_bool() == Some(false)) {
                
                // Modify the request to avoid the known 500 error
                if let Some(obj) = json.as_object_mut() {
                    obj.insert("stream".to_string(), Value::Bool(true));
                    warn!("Modified request to use streaming to avoid known NIM issue with logprobs=2, echo=true, stream=false");
                }
            }
        }
        
        // Mixtral 8x7B Instruct v0.1 doesn't support function calling with vLLM
        if model.contains("mixtral-8x7b") && json.get("functions").is_some() {
            warn!("Mixtral 8x7B may not support function calling with vLLM profiles");
        }
        
        // For Mixtral with LoRA on vLLM, set NIM_MAX_LORA_RANK=256
        if model.contains("mixtral") && json.get("lora_adapters").is_some() {
            std::env::set_var("NIM_MAX_LORA_RANK", "256");
            info!("Set NIM_MAX_LORA_RANK=256 for Mixtral with LoRA");
        }
        
        // For L40s with Llama 3.1 70B Instruct, avoid vLLM LoRA TP8
        if model.contains("llama-3.1-70b") && json.get("lora_adapters").is_some() {
            // This is handled via warning only since we can't change hardware config from here
            warn!("Using Llama 3.1 70B with LoRA - Note that LoRA A10G TP8 for both vLLM and TRTLLM is not supported due to insufficient memory");
        }
        
        // Handle sequence length issues for models with OOB 8K
        // Set NIM_MAX_MODEL_LEN if needed for larger contexts
        if json.get("max_tokens").is_some() && !std::env::var("NIM_MAX_MODEL_LEN").is_ok() {
            // Only set for models that support longer contexts
            if model.contains("llama-3") {
                std::env::set_var("NIM_MAX_MODEL_LEN", "8192");
                debug!("Set NIM_MAX_MODEL_LEN=8192 for context window");
            } else if model.contains("nemotron-4-340b") {
                std::env::set_var("NIM_MAX_MODEL_LEN", "128000");
                debug!("Set NIM_MAX_MODEL_LEN=128000 for Nemotron 4 340B");
            }
        }
    }
    
    /// Check if model has known issues with vGPU
    pub fn has_vgpu_issues(model: &str) -> bool {
        // Known models with vGPU issues
        if model.contains("llama-3.1-70b") || model.contains("llama-3.1-405b") {
            // Check if we're running on vGPU
            if let Ok(gpu_info) = std::process::Command::new("nvidia-smi")
                .args(&["--query-gpu=name", "--format=csv,noheader"])
                .output() {
                    
                let output = String::from_utf8_lossy(&gpu_info.stdout);
                if output.contains("vGPU") {
                    warn!("Detected vGPU with model {}. Setting NIM_LOW_MEMORY_MODE=1 to avoid OOM errors", model);
                    std::env::set_var("NIM_LOW_MEMORY_MODE", "1");
                    return true;
                }
            }
        }
        false
    }
    
    /// Configure environment for NIM based on model
    pub fn configure_for_model(model: &str) {
        if Self::is_nim_model(model) {
            // Check vGPU issues
            Self::has_vgpu_issues(model);
            
            // For local builds
            if model.contains("nemotron-4-340b") {
                warn!("Nemotron 4 340B does not support buildable TRT-LLM profiles");
            }
            
            // Set cache directory if not already set
            if std::env::var("NIM_CACHE_PATH").is_err() {
                if let Some(home) = dirs::home_dir() {
                    let cache_path = home.join(".cache").join("nim-cache");
                    std::env::set_var("NIM_CACHE_PATH", cache_path.to_string_lossy().to_string());
                    info!("Set NIM_CACHE_PATH to {}", cache_path.to_string_lossy());
                }
            }
        }
    }
}

/// Sanitize input for NIM models to prevent issues with Unicode
pub fn sanitize_input(input: &mut Value) {
    // Only process object inputs
    let obj = match input.as_object_mut() {
        Some(obj) => obj,
        None => return,
    };

    // Process messages for chat completions
    if let Some(messages) = obj.get_mut("messages") {
        if let Some(messages_array) = messages.as_array_mut() {
            for message in messages_array {
                if let Some(message_obj) = message.as_object_mut() {
                    if let Some(content) = message_obj.get_mut("content") {
                        if let Some(content_str) = content.as_str() {
                            // Replace problematic unicode with standard ASCII where possible
                            let cleaned = content_str
                                .replace('\u{2018}', "'") // Left single quotation mark
                                .replace('\u{2019}', "'") // Right single quotation mark
                                .replace('\u{201C}', "\"") // Left double quotation mark
                                .replace('\u{201D}', "\"") // Right double quotation mark
                                .replace('\u{2013}', "-") // En dash
                                .replace('\u{2014}', "--") // Em dash
                                .replace('\u{2026}', "..."); // Ellipsis
                            
                            if cleaned != content_str {
                                debug!("Sanitized unicode characters in message content");
                                *content = Value::String(cleaned);
                            }
                        }
                    }
                }
            }
        }
    }

    // Process prompt for completions
    if let Some(prompt) = obj.get_mut("prompt") {
        if let Some(prompt_str) = prompt.as_str() {
            // Clean the same characters
            let cleaned = prompt_str
                .replace('\u{2018}', "'")
                .replace('\u{2019}', "'")
                .replace('\u{201C}', "\"")
                .replace('\u{201D}', "\"")
                .replace('\u{2013}', "-")
                .replace('\u{2014}', "--")
                .replace('\u{2026}', "...");
            
            if cleaned != prompt_str {
                debug!("Sanitized unicode characters in prompt");
                *prompt = Value::String(cleaned);
            }
        }
    }
}

/// Configure NIM-specific environment variables
pub fn configure_nim_environment(model: &str) {
    // Add model-specific environment variables
    if model.contains("llama-3.1") {
        std::env::set_var("NIM_ENABLE_PAGED_ATTENTION", "1");
        std::env::set_var("NIM_ENABLE_TENSOR_PARALLEL", "1");
    } else if model.contains("mistral") {
        std::env::set_var("NIM_ENABLE_PAGED_ATTENTION", "1");
    } else if model.contains("llava") {
        std::env::set_var("NIM_ENABLE_MULTIMODAL", "1");
    }
    
    // Set generic NIM optimizations
    std::env::set_var("NIM_USE_FAST_TOKENIZER", "1");
}

/// Get model parameters suitable for NIM models
pub fn get_model_parameters(model: &str) -> Value {
    let mut params = json!({});
    
    // Configure model-specific parameters
    if model.contains("llama-3.1") {
        params = json!({
            "temperature": 0.7,
            "top_p": 0.95,
            "max_tokens": 1024
        });
    } else if model.contains("mixtral") {
        params = json!({
            "temperature": 0.7,
            "top_p": 0.9,
            "max_tokens": 2048
        });
    } else if model.contains("mistral") {
        params = json!({
            "temperature": 0.7,
            "top_p": 0.9,
            "max_tokens": 1024
        });
    }
    
    params
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_sanitize_input() {
        // Test chat completion input sanitization
        let mut input = json!({
            "messages": [
                {
                    "role": "user",
                    "content": "Hello, let's test \u{201C}fancy quotes\u{201D} and ellipsis\u{2026}"
                }
            ]
        });
        
        sanitize_input(&mut input);
        
        let content = input["messages"][0]["content"].as_str().unwrap();
        assert_eq!(content, "Hello, let's test \"fancy quotes\" and ellipsis...");
        
        // Test completion input sanitization
        let mut input = json!({
            "prompt": "Testing an em dash\u{2014}and en dash\u{2013}in text"
        });
        
        sanitize_input(&mut input);
        
        let prompt = input["prompt"].as_str().unwrap();
        assert_eq!(prompt, "Testing an em dash--and en dash-in text");
    }
    
    #[test]
    fn test_model_parameters() {
        let llama_params = get_model_parameters("meta/llama-3.1-8b-instruct");
        assert_eq!(llama_params["max_tokens"], 1024);
        
        let mixtral_params = get_model_parameters("mistralai/mixtral-8x7b-instruct-v0.1");
        assert_eq!(mixtral_params["max_tokens"], 2048);
        
        let mistral_params = get_model_parameters("mistralai/mistral-7b-instruct-v0.1");
        assert_eq!(mistral_params["max_tokens"], 1024);
    }
} 