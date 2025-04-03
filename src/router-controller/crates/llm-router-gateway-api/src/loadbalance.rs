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

//! Load balancing functionality for distributing requests among multiple LLM instances
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashMap;
use rand::seq::SliceRandom;
use rand::thread_rng;
use log::{info, debug};

use crate::config::Llm;
use crate::metrics::track_load_balancer_selection;

/// Strategies for load balancing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalancingStrategy {
    /// Select instances in a round-robin fashion
    RoundRobin,
    /// Select instances randomly
    Random,
    /// Always select the first instance (no load balancing)
    First,
}

/// Load balancer for handling multiple instances of the same logical LLM
pub struct LoadBalancer {
    strategy: LoadBalancingStrategy,
    counters: HashMap<String, AtomicUsize>,
}

impl LoadBalancer {
    /// Create a new load balancer with the specified strategy
    pub fn new(strategy: LoadBalancingStrategy) -> Self {
        info!("Creating load balancer with strategy: {:?}", strategy);
        Self {
            strategy,
            counters: HashMap::new(),
        }
    }

    /// Select an LLM instance from multiple options with the same logical name
    pub fn select_instance<'a>(&mut self, llm_name: &str, instances: &'a [Llm]) -> &'a Llm {
        if instances.is_empty() {
            panic!("Cannot select instance from empty list");
        }

        if instances.len() == 1 {
            // If there's only one instance, no need for load balancing
            return &instances[0];
        }

        // Different strategies for instance selection
        let selected_index = match self.strategy {
            LoadBalancingStrategy::RoundRobin => self.round_robin(llm_name, instances.len()),
            LoadBalancingStrategy::Random => self.random(instances.len()),
            LoadBalancingStrategy::First => 0,
        };

        // Get the selected instance
        let selected = &instances[selected_index];
        debug!(
            "Load balancer selected instance {}/{} for LLM '{}': {}",
            selected_index + 1,
            instances.len(),
            llm_name,
            selected.api_base
        );

        // Track metrics for the selection
        track_load_balancer_selection(llm_name, &selected.api_base);

        selected
    }

    /// Round-robin selection
    fn round_robin(&mut self, key: &str, count: usize) -> usize {
        // Get or create counter for this LLM
        let counter = self.counters.entry(key.to_string()).or_insert_with(|| {
            AtomicUsize::new(0)
        });

        // Get current value and increment
        let current = counter.fetch_add(1, Ordering::Relaxed);
        current % count
    }

    /// Random selection
    fn random(&self, count: usize) -> usize {
        let mut rng = thread_rng();
        // Use slice random to select a random index
        (0..count).collect::<Vec<_>>().choose(&mut rng).copied().unwrap_or(0)
    }
}

/// Create a new load balancer with the specified strategy
pub fn create_load_balancer(strategy_name: &str) -> LoadBalancer {
    let strategy = match strategy_name.to_lowercase().as_str() {
        "round_robin" => LoadBalancingStrategy::RoundRobin,
        "random" => LoadBalancingStrategy::Random,
        "first" => LoadBalancingStrategy::First,
        _ => {
            info!("Unknown load balancing strategy '{}', defaulting to round_robin", strategy_name);
            LoadBalancingStrategy::RoundRobin
        }
    };

    LoadBalancer::new(strategy)
} 