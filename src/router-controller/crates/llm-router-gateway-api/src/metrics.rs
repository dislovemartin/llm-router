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

use lazy_static::lazy_static;
use prometheus::{
    register_histogram, register_histogram_vec, register_int_counter, register_int_counter_vec,
    register_gauge, Histogram, HistogramVec, IntCounter, IntCounterVec, Gauge,
};
use serde_json::Value;

lazy_static! {
    pub static ref NUM_REQUESTS: IntCounter =
        register_int_counter!("num_requests", "Total number of requests")
            .expect("Failed to create num_requests counter");

    pub static ref REQUESTS_PER_POLICY: IntCounterVec = register_int_counter_vec!(
        "requests_per_policy",
        "Total number of requests per policy",
        &["policy"]
    )
    .expect("Failed to create requests_per_policy counter vector");

    pub static ref REQUESTS_PER_MODEL: IntCounterVec = register_int_counter_vec!(
        "requests_per_model",
        "Total number of requests per model",
        &["model"]
    )
    .expect("Failed to create requests_per_model counter vector");

    pub static ref REQUEST_LATENCY: Histogram = register_histogram!(
        "request_latency_seconds",
        "Latency of processing requests in seconds"
    )
    .expect("Failed to create request_latency histogram");

    pub static ref REQUEST_SUCCESS: IntCounter =
        register_int_counter!("request_success_total", "Total successful requests")
            .expect("Failed to create request_success counter");

    pub static ref REQUEST_FAILURE: IntCounterVec = register_int_counter_vec!(
        "request_failure_total",
        "Total failed requests, broken down by error type (4XX, 5XX, other)",
        &["error_type"]
    )
    .expect("Failed to create request_failure counter vector");

    pub static ref ROUTING_POLICY_USAGE: IntCounterVec = register_int_counter_vec!(
        "routing_policy_usage",
        "Number of times each routing policy was used",
        &["routing_policy"]
    )
    .expect("Failed to create routing_policy_usage counter vector");

    pub static ref MODEL_SELECTION_TIME: Histogram = register_histogram!(
        "model_selection_time_seconds",
        "Time (in seconds) taken for model selection (e.g., by Triton)"
    )
    .expect("Failed to create model_selection_time histogram");

    pub static ref LLM_RESPONSE_TIME: HistogramVec = register_histogram_vec!(
        "llm_response_time_seconds",
        "Response time (in seconds) for each LLM",
        &["llm"]
    )
    .expect("Failed to create llm_response_time histogram vector");

    pub static ref TOKEN_USAGE: IntCounterVec = register_int_counter_vec!(
        "llm_token_usage",
        "Token usage per LLM category",
        &["llm_name", "category"]
    )
    .unwrap();

    pub static ref PROXY_OVERHEAD_LATENCY: Histogram = register_histogram!(
        "proxy_overhead_latency_seconds",
        "Overhead latency of the proxy, calculated as overall latency minus model selection and LLM response time"
    )
    .expect("Failed to create proxy_overhead_latency histogram");

    // New metrics for retries, load balancing, circuit breakers, and caching
    pub static ref RETRY_COUNT: IntCounterVec = register_int_counter_vec!(
        "llm_retry_count",
        "Number of retries per LLM",
        &["llm_name"]
    )
    .expect("Failed to create retry_count counter vector");

    pub static ref CACHE_HIT_COUNT: IntCounter =
        register_int_counter!("cache_hit_count", "Total number of cache hits")
            .expect("Failed to create cache_hit_count counter");

    pub static ref CACHE_MISS_COUNT: IntCounter =
        register_int_counter!("cache_miss_count", "Total number of cache misses")
            .expect("Failed to create cache_miss_count counter");

    pub static ref CACHE_SIZE: Gauge =
        register_gauge!("cache_size", "Current number of entries in the cache")
            .expect("Failed to create cache_size gauge");

    pub static ref CIRCUIT_BREAKER_OPEN: IntCounterVec = register_int_counter_vec!(
        "circuit_breaker_open",
        "Number of times circuit breaker opened per endpoint",
        &["endpoint"]
    )
    .expect("Failed to create circuit_breaker_open counter vector");

    pub static ref CIRCUIT_BREAKER_STATUS: IntCounterVec = register_int_counter_vec!(
        "circuit_breaker_status",
        "Status of circuit breakers (0=closed, 1=half-open, 2=open)",
        &["endpoint", "status"]
    )
    .expect("Failed to create circuit_breaker_status counter vector");

    pub static ref LOAD_BALANCER_USAGE: IntCounterVec = register_int_counter_vec!(
        "load_balancer_usage",
        "Number of times each instance was selected by the load balancer",
        &["llm_name", "api_base"]
    )
    .expect("Failed to create load_balancer_usage counter vector");
}

pub fn track_token_usage(json: &Value, llm_name: &str) {
    if let Some(usage) = json.get("usage") {
        if let Some(prompt) = usage["prompt_tokens"].as_u64() {
            TOKEN_USAGE
                .with_label_values(&[llm_name, "prompt"])
                .inc_by(prompt);
        }
        if let Some(completion) = usage["completion_tokens"].as_u64() {
            TOKEN_USAGE
                .with_label_values(&[llm_name, "completion"])
                .inc_by(completion);
        }
        if let Some(total) = usage["total_tokens"].as_u64() {
            TOKEN_USAGE
                .with_label_values(&[llm_name, "total"])
                .inc_by(total);
        }
    }
}

/// Track a retry for a specific LLM
pub fn track_retry(llm_name: &str) {
    RETRY_COUNT.with_label_values(&[llm_name]).inc();
}

/// Update circuit breaker status metrics
pub fn update_circuit_breaker_status(endpoint: &str, status: &str) {
    // Reset all statuses for this endpoint
    CIRCUIT_BREAKER_STATUS.with_label_values(&[endpoint, "closed"]).reset();
    CIRCUIT_BREAKER_STATUS.with_label_values(&[endpoint, "half-open"]).reset();
    CIRCUIT_BREAKER_STATUS.with_label_values(&[endpoint, "open"]).reset();
    
    // Set the current status
    CIRCUIT_BREAKER_STATUS.with_label_values(&[endpoint, status]).inc();
    
    // If newly opened, increment the open counter
    if status == "open" {
        CIRCUIT_BREAKER_OPEN.with_label_values(&[endpoint]).inc();
    }
}

/// Update cache size metric
pub fn update_cache_size(size: usize) {
    CACHE_SIZE.set(size as f64);
}

/// Track load balancer selection
pub fn track_load_balancer_selection(llm_name: &str, api_base: &str) {
    LOAD_BALANCER_USAGE.with_label_values(&[llm_name, api_base]).inc();
}
