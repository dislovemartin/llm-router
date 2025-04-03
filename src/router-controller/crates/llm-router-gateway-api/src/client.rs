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

//! HTTP client configuration and utilities
use std::time::Duration;
use reqwest::{Client, ClientBuilder};
use log::info;

use crate::config::RouterConfig;

/// Create and configure an HTTP client for downstream requests
pub fn create_http_client(config: &RouterConfig) -> Client {
    // Get timeout from configuration or use a default
    let timeout = Duration::from_secs(config.server.request_timeout);
    
    // Create client builder with basic configuration
    let builder = ClientBuilder::new()
        .timeout(timeout)
        .pool_max_idle_per_host(config.server.connection_pool_size)
        .pool_idle_timeout(Duration::from_secs(90))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(format!("llm-router-gateway/{}", env!("CARGO_PKG_VERSION")))
        .tcp_keepalive(Duration::from_secs(60))
        .brotli(true)
        .gzip(true)
        .deflate(true);
    
    // Build the client
    let client = builder.build().expect("Failed to build HTTP client");
    
    info!(
        "Created HTTP client with timeout {}s, connection pool size {}",
        timeout.as_secs(),
        config.server.connection_pool_size
    );
    
    client
} 