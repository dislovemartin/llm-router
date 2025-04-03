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

//! Cache module for caching LLM responses
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use bytes::Bytes;
use serde_json::Value;
use http::{Response, StatusCode};
use http_body_util::{Full, combinators::BoxBody, BodyExt};
use sha2::{Sha256, Digest};
use base64::engine::{general_purpose, Engine};
use log::{debug, info};

use crate::error::GatewayApiError;
use crate::metrics::{CACHE_HIT_COUNT, CACHE_MISS_COUNT};

/// A response cache entry
struct CacheEntry {
    body_bytes: Bytes,
    status: StatusCode,
    headers: http::HeaderMap,
    expires_at: Instant,
}

/// A simple response cache
pub struct ResponseCache {
    entries: RwLock<HashMap<String, CacheEntry>>,
    ttl: Duration,
    max_size: usize,
}

impl ResponseCache {
    pub fn new(ttl_seconds: u64, max_size: usize) -> Self {
        info!("Initializing response cache with TTL {} seconds, max size {} entries", ttl_seconds, max_size);
        Self {
            entries: RwLock::new(HashMap::with_capacity(max_size)),
            ttl: Duration::from_secs(ttl_seconds),
            max_size,
        }
    }

    /// Generate a cache key from a request
    pub fn generate_key(&self, body: &Value, path: &str) -> String {
        // Remove fields that shouldn't affect caching 
        let mut cache_body = body.clone();
        
        // If there's a nim-llm-router field, remove it
        if let Some(obj) = cache_body.as_object_mut() {
            obj.remove("nim-llm-router");
            
            // Remove fields that might change between requests but don't affect the response
            obj.remove("stream");
            obj.remove("stream_options");
            
            // Keep only fields that affect the response
            let fields_to_keep = vec!["messages", "model", "temperature", "top_p", "max_tokens", "frequency_penalty", "presence_penalty", "stop"];
            obj.retain(|key, _| fields_to_keep.contains(&key.as_str()));
        }
        
        // Create key from path and sanitized body
        let key_data = format!("{}:{}", path, serde_json::to_string(&cache_body).unwrap_or_default());
        
        // Hash the key data to get a fixed-length key
        let mut hasher = Sha256::new();
        hasher.update(key_data.as_bytes());
        let result = hasher.finalize();
        
        general_purpose::STANDARD.encode(result)
    }

    /// Check if request should be cached based on its properties
    pub fn is_cacheable(&self, body: &Value) -> bool {
        // Don't cache streaming responses
        if body.get("stream").map_or(false, |v| v.as_bool() == Some(true)) {
            return false;
        }
        
        // Don't cache if specifically disabled
        if body.get("cache").map_or(false, |v| v.as_bool() == Some(false)) {
            return false;
        }
        
        // Must have a low temperature to be deterministic
        if let Some(temp) = body.get("temperature").and_then(|v| v.as_f64()) {
            if temp > 0.01 {
                return false;
            }
        }
        
        // Check if temperature is close to zero
        if let Some(top_p) = body.get("top_p").and_then(|v| v.as_f64()) {
            if top_p < 0.999 {
                return false;
            }
        }
        
        true
    }

    /// Get a cached response if available
    pub async fn get(&self, key: &str) -> Option<Response<BoxBody<Bytes, GatewayApiError>>> {
        let entries = self.entries.read().await;
        if let Some(entry) = entries.get(key) {
            if Instant::now() < entry.expires_at {
                // Create a new response from the cached entry
                let mut builder = Response::builder()
                    .status(entry.status);
                
                // Add headers from cache
                for (key, value) in &entry.headers {
                    builder = builder.header(key, value);
                }
                
                // Create the response body
                let response = builder
                    .body(Full::from(entry.body_bytes.clone())
                        .map_err(|_| GatewayApiError::Other {
                            message: "Failed to create response body".to_string(),
                        })
                        .boxed())
                    .unwrap_or_else(|_| {
                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Full::from(Bytes::from("Cache error"))
                                .map_err(|_| GatewayApiError::Other {
                                    message: "Failed to create error response body".to_string(),
                                })
                                .boxed())
                            .unwrap()
                    });
                
                debug!("Cache hit for key: {}", key);
                CACHE_HIT_COUNT.inc();
                return Some(response);
            }
        }
        debug!("Cache miss for key: {}", key);
        CACHE_MISS_COUNT.inc();
        None
    }

    /// Store a response in the cache
    pub async fn set(&self, key: &str, response: Response<BoxBody<Bytes, GatewayApiError>>) -> Result<(), GatewayApiError> {
        // Don't cache error responses
        let status = response.status();
        if !status.is_success() {
            return Ok(());
        }
        
        // Decompose the response to get parts and body
        let (parts, body) = response.into_parts();
        
        // Convert the body to bytes
        let mut body_bytes = Vec::new();
        let mut body_stream = body;
        
        // Read the body bytes
        while let Some(chunk) = body_stream.frame().await.transpose()? {
            if let Some(data) = chunk.data_ref() {
                body_bytes.extend_from_slice(data);
            }
        }
        
        let body_bytes = Bytes::from(body_bytes);
        
        // Create a cache entry
        let entry = CacheEntry {
            body_bytes: body_bytes.clone(),
            status: parts.status,
            headers: parts.headers,
            expires_at: Instant::now() + self.ttl,
        };
        
        let mut entries = self.entries.write().await;
        
        // If we're at max capacity, remove the oldest entry
        if entries.len() >= self.max_size && !entries.contains_key(key) {
            if let Some((oldest_key, _)) = entries.iter()
                .min_by_key(|(_, entry)| entry.expires_at) {
                let oldest_key = oldest_key.clone();
                entries.remove(&oldest_key);
                debug!("Removed oldest cache entry with key: {}", oldest_key);
            }
        }
        
        entries.insert(key.to_string(), entry);
        debug!("Added entry to cache with key: {}, size: {}", key, body_bytes.len());
        
        Ok(())
    }

    /// Clean expired cache entries
    pub async fn clean_expired(&self) {
        let now = Instant::now();
        let mut entries = self.entries.write().await;
        
        let initial_count = entries.len();
        
        // Remove expired entries
        entries.retain(|_, entry| entry.expires_at > now);
        
        let removed = initial_count - entries.len();
        if removed > 0 {
            info!("Cleaned {} expired cache entries, remaining count: {}", removed, entries.len());
        } else {
            debug!("No expired cache entries to clean, current count: {}", entries.len());
        }
    }
    
    /// Get current cache stats
    pub async fn get_stats(&self) -> (usize, usize) {
        let entries = self.entries.read().await;
        let total = entries.len();
        let active = entries.values().filter(|entry| entry.expires_at > Instant::now()).count();
        (active, total)
    }
} 