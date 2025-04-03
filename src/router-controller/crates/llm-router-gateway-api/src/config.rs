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

//! Configuration for the LLM Router Gateway API
use std::fs;
use std::sync::Arc;
use std::env;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use log::{info, warn, error, debug};
use serde::{Deserialize, Serialize};

use crate::error::ConfigError;

/// Server configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerConfig {
    /// Host to bind to
    #[serde(default = "default_host")]
    pub host: String,
    
    /// Port to listen on
    #[serde(default = "default_port")]
    pub port: u16,
    
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub request_timeout: u64,
    
    /// Connection pool size
    #[serde(default = "default_connection_pool_size")]
    pub connection_pool_size: usize,
}

/// Security configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SecurityConfig {
    /// API keys for authentication
    #[serde(default)]
    pub api_keys: Option<Vec<String>>,
    
    /// Rate limiting configuration
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,
}

/// Rate limiting configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests allowed per second
    pub requests_per_second: f64,
    
    /// Maximum burst size
    pub burst_size: u32,
    
    /// Whether to apply rate limiting per IP
    #[serde(default = "default_true")]
    pub per_ip: bool,
}

/// Observability configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ObservabilityConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
    
    /// Whether to output logs in JSON format
    #[serde(default)]
    pub json_logging: bool,
}

/// Caching configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachingConfig {
    /// Whether caching is enabled
    #[serde(default)]
    pub enabled: bool,
    
    /// TTL for cached responses in seconds
    pub ttl_seconds: Option<u64>,
    
    /// Maximum number of items in cache
    pub max_size: Option<usize>,
}

/// Retry configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retries
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    
    /// Initial backoff in milliseconds
    #[serde(default = "default_initial_backoff")]
    pub initial_backoff_ms: u64,
}

/// Circuit breaker configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Whether circuit breaking is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    /// Number of failures before tripping the circuit
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: usize,
    
    /// Reset timeout in seconds
    #[serde(default = "default_reset_timeout")]
    pub reset_timeout_secs: u64,
}

/// Main router configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RouterConfig {
    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,
    
    /// Security configuration
    #[serde(default)]
    pub security: SecurityConfig,
    
    /// Observability configuration
    #[serde(default)]
    pub observability: ObservabilityConfig,
    
    /// Caching configuration
    #[serde(default)]
    pub caching: CachingConfig,
    
    /// Retry configuration
    #[serde(default)]
    pub retry: RetryConfig,
    
    /// Circuit breaker configuration
    #[serde(default)]
    pub circuit_breaker: CircuitBreakerConfig,
    
    /// Load balancing strategy (round_robin, random, first)
    #[serde(default = "default_load_balancing_strategy")]
    pub load_balancing_strategy: String,
    
    /// Routing policies
    pub policies: Vec<Policy>,
}

/// Routing policy configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Policy {
    /// Policy name
    pub name: String,
    
    /// Triton model URL for this policy
    pub url: String,
    
    /// LLMs available under this policy
    pub llms: Vec<Llm>,
}

/// LLM configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Llm {
    /// LLM name
    pub name: String,
    
    /// API base URL
    pub api_base: String,
    
    /// API key for authentication
    pub api_key: String,
    
    /// Model identifier
    pub model: String,
}

/// Configuration manager for hot-reloading
pub struct ConfigManager {
    config_path: String,
    config: RwLock<RouterConfig>,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub async fn new(config_path: &str) -> Result<Self> {
        let config_path = config_path.to_string();
        let config = RouterConfig::load_config(&config_path)?;
        
        let manager = Self {
            config_path: config_path.clone(),
            config: RwLock::new(config),
        };
        
        // Start background task for hot reloading if enabled
        if env::var("CONFIG_HOT_RELOAD").unwrap_or_default() == "true" {
            let config_path_clone = config_path.clone();
            let config_manager = Arc::new(manager.clone());
            
            tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    match RouterConfig::load_config(&config_path_clone) {
                        Ok(new_config) => {
                            let mut config = config_manager.config.write().await;
                            *config = new_config;
                            info!("Configuration reloaded successfully");
                        }
                        Err(e) => {
                            error!("Failed to reload configuration: {}", e);
                        }
                    }
                }
            });
            
            info!("Configuration hot-reloading enabled");
        }
        
        Ok(manager)
    }
    
    /// Get a clone of the current configuration
    pub async fn get_config(&self) -> RouterConfig {
        self.config.read().await.clone()
    }
}

// Make ConfigManager cloneable
impl Clone for ConfigManager {
    fn clone(&self) -> Self {
        // We create a new RwLock but with the same contents
        let config = self.config.try_read()
            .map(|config| config.clone())
            .unwrap_or_else(|_| {
                warn!("Failed to read config for clone operation, using default");
                RouterConfig::default()
            });
            
        Self {
            config_path: self.config_path.clone(),
            config: RwLock::new(config),
        }
    }
}

impl RouterConfig {
    /// Load configuration from file
    pub fn load_config(path: &str) -> Result<RouterConfig> {
        info!("Loading configuration from {}", path);
        let content = fs::read_to_string(path).map_err(|e| ConfigError::FileError {
            path: path.to_string(),
            error: e.to_string(),
        })?;
        
        // Parse YAML
        let mut config: RouterConfig = serde_yaml::from_str(&content).map_err(|e| ConfigError::ParseError {
            message: format!("Failed to parse YAML: {}", e),
        })?;
        
        // Handle environment variable overrides
        config.apply_env_overrides();
        
        // Validate configuration
        validate_config(&config)?;
        
        // Apply environment variable substitution in API keys
        config.resolve_env_vars();
        
        Ok(config)
    }

    /// Apply environment variable overrides to configuration
    fn apply_env_overrides(&mut self) {
        // Server configuration
        if let Ok(host) = env::var("LLM_ROUTER__SERVER__HOST") {
            self.server.host = host;
        }
        
        if let Ok(port) = env::var("LLM_ROUTER__SERVER__PORT").and_then(|p| p.parse().map_err(|_| env::VarError::NotPresent)) {
            self.server.port = port;
        }
        
        if let Ok(timeout) = env::var("LLM_ROUTER__SERVER__REQUEST_TIMEOUT").and_then(|t| t.parse().map_err(|_| env::VarError::NotPresent)) {
            self.server.request_timeout = timeout;
        }
        
        // Security
        if let Ok(api_keys) = env::var("LLM_ROUTER__SECURITY__API_KEYS") {
            self.security.api_keys = Some(api_keys.split(',').map(|s| s.trim().to_string()).collect());
        }
        
        // Rate limiting
        if let Ok(rps) = env::var("LLM_ROUTER__SECURITY__RATE_LIMIT__REQUESTS_PER_SECOND").and_then(|r| r.parse().map_err(|_| env::VarError::NotPresent)) {
            if self.security.rate_limit.is_none() {
                self.security.rate_limit = Some(RateLimitConfig {
                    requests_per_second: rps,
                    burst_size: 50,
                    per_ip: true,
                });
            } else {
                self.security.rate_limit.as_mut().unwrap().requests_per_second = rps;
            }
        }
        
        // Logging
        if let Ok(log_level) = env::var("LLM_ROUTER__OBSERVABILITY__LOG_LEVEL") {
            self.observability.log_level = log_level;
        }
        
        if let Ok(json_logging) = env::var("LLM_ROUTER__OBSERVABILITY__JSON_LOGGING") {
            self.observability.json_logging = json_logging == "true";
        }
    }
    
    /// Resolve environment variables in API keys
    fn resolve_env_vars(&mut self) {
        for policy in &mut self.policies {
            for llm in &mut policy.llms {
                // If API key is an environment variable reference (${VAR_NAME})
                if llm.api_key.starts_with("${") && llm.api_key.ends_with("}") {
                    let env_var = &llm.api_key[2..llm.api_key.len()-1];
                    match env::var(env_var) {
                        Ok(value) => {
                            debug!("Resolved environment variable {} for LLM {}", env_var, llm.name);
                            llm.api_key = value;
                        }
                        Err(_) => {
                            warn!("Failed to resolve environment variable {} for LLM {}", env_var, llm.name);
                        }
                    }
                }
            }
        }
    }

    /// Get policy by name
    pub fn get_policy_by_name(&self, name: &str) -> Option<Policy> {
        self.policies
            .iter()
            .find(|policy| policy.name.trim() == name.trim())
            .cloned()
    }

    /// Get policy by index
    pub fn get_policy_by_index(&self, index: usize) -> Option<Policy> {
        self.policies.get(index).cloned()
    }

    /// Create a sanitized version of the configuration (hiding API keys)
    pub fn sanitized(&self) -> Self {
        let sanitized_policies = self
            .policies
            .iter()
            .map(|policy| {
                let sanitized_llms = policy
                    .llms
                    .iter()
                    .map(|llm| Llm {
                        api_key: "[REDACTED]".to_string(),
                        ..llm.clone()
                    })
                    .collect();
                Policy {
                    llms: sanitized_llms,
                    ..policy.clone()
                }
            })
            .collect();

        RouterConfig {
            policies: sanitized_policies,
            ..self.clone()
        }
    }
}

impl Policy {
    /// Get LLM by name
    pub fn get_llm_by_name(&self, name: &str) -> Option<Llm> {
        self.llms
            .iter()
            .find(|llm| llm.name.trim() == name.trim())
            .cloned()
    }

    /// Get LLM by index
    pub fn get_llm_by_index(&self, index: usize) -> Option<Llm> {
        self.llms.get(index).cloned()
    }

    /// Get LLM name by index
    pub fn get_llm_name_by_index(&self, index: usize) -> Option<String> {
        self.llms.get(index).map(|llm| llm.name.clone())
    }
    
    /// Get all LLMs with the same logical name
    pub fn get_llms_by_name(&self, name: &str) -> Vec<&Llm> {
        self.llms
            .iter()
            .filter(|llm| llm.name.trim() == name.trim())
            .collect()
    }
}

// Default implementations for optional configuration parameters
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            request_timeout: default_timeout(),
            connection_pool_size: default_connection_pool_size(),
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            api_keys: None,
            rate_limit: None,
        }
    }
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            json_logging: false,
        }
    }
}

impl Default for CachingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ttl_seconds: Some(300), // 5 minutes
            max_size: Some(1000),   // 1000 entries
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_backoff_ms: default_initial_backoff(),
        }
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            failure_threshold: default_failure_threshold(),
            reset_timeout_secs: default_reset_timeout(),
        }
    }
}

// Default values for configuration parameters
fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8084
}

fn default_timeout() -> u64 {
    60
}

fn default_connection_pool_size() -> usize {
    100
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_max_retries() -> u32 {
    2
}

fn default_initial_backoff() -> u64 {
    100
}

fn default_failure_threshold() -> usize {
    5
}

fn default_reset_timeout() -> u64 {
    30
}

fn default_true() -> bool {
    true
}

fn default_load_balancing_strategy() -> String {
    "round_robin".to_string()
}

pub type Result<T> = std::result::Result<T, ConfigError>;

/// Validate configuration
fn validate_config(config: &RouterConfig) -> Result<()> {
    for policy in &config.policies {
        if policy.name.is_empty() {
            return Err(ConfigError::MissingPolicyField {
                policy: policy.name.clone(),
                field: "name".to_string(),
            });
        }

        for llm in &policy.llms {
            if llm.api_base.is_empty() {
                return Err(ConfigError::MissingLlmField {
                    llm: llm.name.clone(),
                    field: "api_base".to_string(),
                });
            }
            if llm.model.is_empty() {
                return Err(ConfigError::MissingLlmField {
                    llm: llm.name.clone(),
                    field: "model".to_string(),
                });
            }
            if llm.api_key.is_empty() {
                return Err(ConfigError::MissingLlmField {
                    llm: llm.name.clone(),
                    field: "api_key".to_string(),
                });
            }
        }
    }
    Ok(())
}

// Default implementation for RouterConfig
impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            security: SecurityConfig::default(),
            observability: ObservabilityConfig::default(),
            caching: CachingConfig::default(),
            retry: RetryConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            load_balancing_strategy: default_load_balancing_strategy(),
            policies: Vec::new(),
        }
    }
}
