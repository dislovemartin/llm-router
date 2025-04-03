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

//! Health check functionality for Kubernetes readiness and liveness probes
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use http::{Request, Response, StatusCode};
use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full, BodyExt};
use serde::Serialize;
use log::{info, debug, warn};
use reqwest::Client;

use crate::error::GatewayApiError;
use crate::config::RouterConfig;
use crate::circuitbreaker::CircuitBreakerRegistry;

/// Health status information
#[derive(Serialize)]
struct HealthStatus {
    status: String,
    triton_status: Option<bool>,
    llm_providers: HashMap<String, bool>,
    uptime_seconds: u64,
    version: String,
    circuit_breakers: HashMap<String, String>,
}

/// System-wide data for uptime tracking
static mut START_TIME: Option<u64> = None;

/// Initialize health check system
pub fn initialize_health_check() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    // Store startup time safely
    unsafe {
        START_TIME = Some(now);
    }
    
    info!("Health check system initialized");
}

/// Handle health check requests
pub async fn health_check<B>(
    req: Request<B>,
    config: Arc<RouterConfig>,
    client: &Client,
    circuit_breakers: Option<&CircuitBreakerRegistry>,
) -> Result<Response<BoxBody<Bytes, GatewayApiError>>, GatewayApiError> {
    // Basic health check just returns OK
    let basic = req.uri().path() == "/health";
    
    // Readiness probe checks Triton server and LLM providers
    let readiness = req.uri().path() == "/health/readiness";
    
    if basic {
        let json = serde_json::json!({
            "status": "OK",
            "version": env!("CARGO_PKG_VERSION"),
        });
        
        let bytes = Bytes::from(serde_json::to_vec(&json)?);
        let body = Full::from(bytes)
            .map_err(|_| GatewayApiError::Other {
                message: "Failed to create response body".to_string(),
            })
            .boxed();
        
        debug!("Basic health check: OK");
        
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(body)?)
    } else if readiness {
        debug!("Processing readiness health check");
        
        let mut status = HealthStatus {
            status: "OK".to_string(),
            triton_status: None,
            llm_providers: HashMap::new(),
            uptime_seconds: calculate_uptime(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            circuit_breakers: HashMap::new(),
        };
        
        // Check circuit breakers if available
        if let Some(breakers) = circuit_breakers {
            let breaker_statuses = breakers.get_all_breakers().await;
            for (endpoint, state) in breaker_statuses {
                let state_str = match state {
                    crate::circuitbreaker::CircuitState::Closed => "closed",
                    crate::circuitbreaker::CircuitState::HalfOpen => "half-open",
                    crate::circuitbreaker::CircuitState::Open => {
                        status.status = "Degraded".to_string();
                        "open"
                    },
                };
                status.circuit_breakers.insert(endpoint, state_str.to_string());
            }
        }
        
        // Check Triton server
        if !config.policies.is_empty() {
            let policy = &config.policies[0];
            match client.get(&policy.url).timeout(std::time::Duration::from_secs(2)).send().await {
                Ok(resp) => {
                    let success = resp.status().is_success();
                    status.triton_status = Some(success);
                    if !success {
                        status.status = "Degraded".to_string();
                        warn!("Triton server health check failed with status: {}", resp.status());
                    }
                },
                Err(e) => {
                    status.triton_status = Some(false);
                    status.status = "Degraded".to_string();
                    warn!("Triton server health check failed: {}", e);
                }
            }
        }
        
        // Check a sample of LLM providers
        let mut checked_providers = std::collections::HashSet::new();
        for policy in &config.policies {
            for llm in &policy.llms {
                // Only check each provider once
                let provider_key = llm.api_base.clone();
                if checked_providers.contains(&provider_key) {
                    continue;
                }
                
                checked_providers.insert(provider_key.clone());
                
                // Try to access provider health endpoint
                let health_url = format!("{}/health", llm.api_base.trim_end_matches('/'));
                match client.get(&health_url)
                    .timeout(std::time::Duration::from_secs(2))
                    .header("Authorization", format!("Bearer {}", llm.api_key))
                    .send().await 
                {
                    Ok(resp) => {
                        let is_healthy = resp.status().is_success();
                        status.llm_providers.insert(provider_key, is_healthy);
                        if !is_healthy {
                            status.status = "Degraded".to_string();
                            warn!("LLM provider health check failed with status: {}", resp.status());
                        }
                    },
                    Err(e) => {
                        status.llm_providers.insert(provider_key, false);
                        status.status = "Degraded".to_string();
                        warn!("LLM provider health check failed: {}", e);
                    }
                }
            }
        }
        
        // Set overall status
        if status.triton_status == Some(false) {
            status.status = "Critical".to_string();
            warn!("Health check status: Critical - Triton server is down");
        } else if status.status == "Degraded" {
            info!("Health check status: Degraded - Some components are not fully operational");
        } else {
            debug!("Health check status: OK - All components are operational");
        }
        
        let json = serde_json::to_vec(&status)?;
        let body = Full::from(Bytes::from(json))
            .map_err(|_| GatewayApiError::Other {
                message: "Failed to create response body".to_string(),
            })
            .boxed();
        
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(body)?)
    } else {
        Err(GatewayApiError::InvalidRequest {
            message: "Unknown health check endpoint".to_string(),
        })
    }
}

/// Calculate system uptime
fn calculate_uptime() -> u64 {
    let start_time = unsafe { START_TIME.unwrap_or(0) };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    now.saturating_sub(start_time)
} 