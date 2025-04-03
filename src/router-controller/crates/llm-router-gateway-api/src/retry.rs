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

//! Retry functionality with exponential backoff
use std::future::Future;
use std::time::Duration;
use log::debug;
use tokio::time::sleep;

use crate::metrics::track_retry;

/// Retry a fallible async operation with exponential backoff
///
/// # Arguments
/// * `operation` - Async function to retry
/// * `max_retries` - Maximum number of retry attempts
/// * `initial_backoff_ms` - Initial backoff time in milliseconds
/// 
/// # Returns
/// Result from the operation, or the last error if all retries fail
pub async fn with_retry<F, Fut, T, E>(
    operation: F,
    max_retries: u32,
    initial_backoff_ms: u64,
    llm_name: &str,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut attempt = 0;
    let mut backoff_ms = initial_backoff_ms;
    
    loop {
        let result = operation().await;
        
        if result.is_ok() || attempt >= max_retries {
            return result;
        }
        
        attempt += 1;
        track_retry(llm_name);
        
        // Calculate next backoff with exponential increase and jitter
        let jitter = (rand::random::<f64>() * 0.1 + 0.95) * backoff_ms as f64;
        backoff_ms = (backoff_ms * 2).min(5000); // Cap at 5 seconds
        
        debug!(
            "Retry {}/{} for LLM {}, waiting {}ms before next attempt",
            attempt, max_retries, llm_name, jitter as u64
        );
        
        sleep(Duration::from_millis(jitter as u64)).await;
    }
}

/// Helper function to decide if an error is retryable
///
/// # Arguments
/// * `status_code` - HTTP status code from the failed request
/// 
/// # Returns
/// `true` if the error is considered retryable
pub fn is_retryable_error(status_code: u16) -> bool {
    match status_code {
        // Server errors are usually retryable
        500 | 502 | 503 | 504 => true,
        
        // Rate limit errors are retryable
        429 => true, 
        
        // Other status codes are not retryable
        _ => false,
    }
}

/// Helper to decide if a reqwest error is retryable
///
/// # Arguments
/// * `error` - The reqwest error
/// 
/// # Returns
/// `true` if the error is considered retryable
pub fn is_reqwest_error_retryable(error: &reqwest::Error) -> bool {
    // Connection errors, timeout errors are retryable
    if error.is_connect() || error.is_timeout() {
        return true;
    }
    
    // Check status code if it's an HTTP error
    if let Some(status) = error.status() {
        return is_retryable_error(status.as_u16());
    }
    
    // Request errors (failure to send) are retryable
    error.is_request()
} 