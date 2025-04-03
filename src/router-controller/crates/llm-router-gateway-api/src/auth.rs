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

//! Authentication middleware for the LLM Router Gateway API
use std::sync::Arc;
use std::task::{Context, Poll};
use futures::future::BoxFuture;
use hyper::{Request, Response, StatusCode};
use tower::{Layer, Service};
use log::debug;

use crate::config::RouterConfig;
use crate::error::GatewayApiError;

/// Layer for API key authentication
#[derive(Clone)]
pub struct ApiKeyLayer {
    config: Arc<RouterConfig>,
}

impl ApiKeyLayer {
    /// Create a new API key authentication layer
    pub fn new(config: Arc<RouterConfig>) -> Self {
        Self { config }
    }
}

impl<S> Layer<S> for ApiKeyLayer {
    type Service = ApiKeyService<S>;

    fn layer(&self, service: S) -> Self::Service {
        ApiKeyService {
            inner: service,
            config: self.config.clone(),
        }
    }
}

/// Service for API key authentication
#[derive(Clone)]
pub struct ApiKeyService<S> {
    inner: S,
    config: Arc<RouterConfig>,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for ApiKeyService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<GatewayApiError> + Send,
    ReqBody: Send + 'static,
    ResBody: Send + 'static,
{
    type Response = S::Response;
    type Error = GatewayApiError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        // Skip authentication for health and metrics endpoints
        let path = req.uri().path();
        if path.starts_with("/health") || path == "/metrics" {
            let inner = self.inner.clone();
            let mut inner = std::mem::replace(&mut self.inner, inner);
            let future = inner.call(req);
            return Box::pin(async move {
                future.await.map_err(Into::into)
            });
        }

        // Get configured API keys
        let api_keys = match &self.config.security.api_keys {
            Some(keys) if !keys.is_empty() => keys.clone(),
            _ => {
                // No API keys configured, skip authentication
                let inner = self.inner.clone();
                let mut inner = std::mem::replace(&mut self.inner, inner);
                let future = inner.call(req);
                return Box::pin(async move {
                    future.await.map_err(Into::into)
                });
            }
        };

        // Extract API key from Authorization header
        let auth_header = req.headers().get("Authorization");
        let api_key = match auth_header {
            Some(header) => {
                let header_str = match header.to_str() {
                    Ok(s) => s,
                    Err(_) => {
                        return Box::pin(async {
                            Err(GatewayApiError::InvalidRequest {
                                message: "Invalid Authorization header format".to_string(),
                            })
                        });
                    }
                };

                // Expected format: "Bearer sk-..."
                if let Some(token) = header_str.strip_prefix("Bearer ") {
                    token.trim().to_string()
                } else {
                    // Also support raw API keys without Bearer prefix
                    header_str.trim().to_string()
                }
            }
            None => {
                // Check for API key as a query parameter
                if let Some(query) = req.uri().query() {
                    if let Some(api_key_param) = query
                        .split('&')
                        .find(|param| param.starts_with("api_key=") || param.starts_with("api-key="))
                    {
                        if let Some(key) = api_key_param.split('=').nth(1) {
                            key.to_string()
                        } else {
                            return Box::pin(async {
                                Err(GatewayApiError::InvalidRequest {
                                    message: "API key parameter is empty".to_string(),
                                })
                            });
                        }
                    } else {
                        return Box::pin(async {
                            Err(GatewayApiError::InvalidRequest {
                                message: "Missing API key in Authorization header or query parameter".to_string(),
                            })
                        });
                    }
                } else {
                    return Box::pin(async {
                        Err(GatewayApiError::InvalidRequest {
                            message: "Missing API key in Authorization header or query parameter".to_string(),
                        })
                    });
                }
            }
        };

        // Validate API key
        if !api_keys.contains(&api_key) {
            debug!("Invalid API key provided");
            return Box::pin(async {
                Err(GatewayApiError::ClientError {
                    status: StatusCode::UNAUTHORIZED,
                    message: "Invalid API key".to_string(),
                    error_type: "invalid_api_key".to_string(),
                })
            });
        }

        debug!("API key authentication successful");

        // API key is valid, proceed to inner service
        let inner = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, inner);
        let future = inner.call(req);
        
        Box::pin(async move {
            future.await.map_err(Into::into)
        })
    }
} 