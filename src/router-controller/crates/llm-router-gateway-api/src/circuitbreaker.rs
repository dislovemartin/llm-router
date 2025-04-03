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

//! Circuit breaker to prevent cascading failures from LLM services
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use log::{warn, info, debug};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    Closed,    // Normal operation - requests allowed
    Open,      // Circuit tripped - requests blocked
    HalfOpen,  // Testing if service is recovered
}

/// Circuit breaker for a specific LLM service endpoint
pub struct CircuitBreaker {
    state: RwLock<CircuitState>,
    failure_threshold: usize,
    reset_timeout: Duration,
    half_open_timeout: Duration,
    failure_count: RwLock<usize>,
    last_failure_time: RwLock<Option<Instant>>,
    half_open_time: RwLock<Option<Instant>>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, reset_timeout_secs: u64) -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            failure_threshold,
            reset_timeout: Duration::from_secs(reset_timeout_secs),
            half_open_timeout: Duration::from_secs(5),
            failure_count: RwLock::new(0),
            last_failure_time: RwLock::new(None),
            half_open_time: RwLock::new(None),
        }
    }

    /// Check if the circuit is allowing requests
    pub async fn is_request_allowed(&self) -> bool {
        let state = *self.state.read().await;
        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if reset timeout has expired
                if let Some(last_failure) = *self.last_failure_time.read().await {
                    if last_failure.elapsed() > self.reset_timeout {
                        // Try half-open state
                        let mut state = self.state.write().await;
                        *state = CircuitState::HalfOpen;
                        *self.half_open_time.write().await = Some(Instant::now());
                        debug!("Circuit breaker state changed to Half-Open");
                        return true;
                    }
                }
                false
            },
            CircuitState::HalfOpen => {
                // In half-open state, only allow one test request
                if let Some(half_open_time) = *self.half_open_time.read().await {
                    half_open_time.elapsed() > self.half_open_timeout
                } else {
                    true
                }
            }
        }
    }

    /// Record a successful request
    pub async fn record_success(&self) {
        let state = *self.state.read().await;
        if state == CircuitState::HalfOpen {
            // Reset circuit back to closed on success
            *self.state.write().await = CircuitState::Closed;
            *self.failure_count.write().await = 0;
            info!("Circuit breaker reset to Closed state after successful test request");
        } else if state == CircuitState::Closed {
            // Reset failure count on success in closed state
            *self.failure_count.write().await = 0;
        }
    }

    /// Record a failed request
    pub async fn record_failure(&self) {
        let current_time = Instant::now();
        *self.last_failure_time.write().await = Some(current_time);

        let state = *self.state.read().await;
        match state {
            CircuitState::Closed => {
                let mut count = self.failure_count.write().await;
                *count += 1;
                
                // Trip the circuit if failure threshold is reached
                if *count >= self.failure_threshold {
                    *self.state.write().await = CircuitState::Open;
                    warn!("Circuit breaker tripped to Open state after {} consecutive failures", *count);
                }
            },
            CircuitState::HalfOpen => {
                // Trip back to open on failure in half-open
                *self.state.write().await = CircuitState::Open;
                warn!("Circuit breaker returned to Open state from Half-Open due to failed test request");
            },
            CircuitState::Open => {
                // Already open, just record the failure
                debug!("Failure recorded while circuit is already Open");
            }
        }
    }
    
    /// Get the current state of the circuit breaker
    pub async fn get_state(&self) -> CircuitState {
        *self.state.read().await
    }
}

/// Manages circuit breakers for multiple LLM endpoints
pub struct CircuitBreakerRegistry {
    circuit_breakers: RwLock<HashMap<String, Arc<CircuitBreaker>>>,
    failure_threshold: usize,
    reset_timeout_secs: u64,
}

impl CircuitBreakerRegistry {
    pub fn new(failure_threshold: usize, reset_timeout_secs: u64) -> Self {
        Self {
            circuit_breakers: RwLock::new(HashMap::new()),
            failure_threshold,
            reset_timeout_secs,
        }
    }

    /// Get or create a circuit breaker for a specific LLM endpoint
    pub async fn get_circuit_breaker(&self, endpoint: &str) -> Arc<CircuitBreaker> {
        let breakers = self.circuit_breakers.read().await;
        if let Some(breaker) = breakers.get(endpoint) {
            return breaker.clone();
        }
        drop(breakers); // Release read lock before acquiring write lock
        
        // Need to create a new circuit breaker
        let mut breakers = self.circuit_breakers.write().await;
        // Double-check in case another thread created it while we were waiting for the write lock
        if let Some(breaker) = breakers.get(endpoint) {
            return breaker.clone();
        }
        
        let breaker = Arc::new(CircuitBreaker::new(
            self.failure_threshold,
            self.reset_timeout_secs
        ));
        breakers.insert(endpoint.to_string(), breaker.clone());
        info!("Created new circuit breaker for endpoint: {}", endpoint);
        breaker
    }
    
    /// Get all registered circuit breakers with their status
    pub async fn get_all_breakers(&self) -> HashMap<String, CircuitState> {
        let breakers = self.circuit_breakers.read().await;
        let mut result = HashMap::new();
        
        for (endpoint, breaker) in breakers.iter() {
            result.insert(endpoint.clone(), breaker.get_state().await);
        }
        
        result
    }
} 