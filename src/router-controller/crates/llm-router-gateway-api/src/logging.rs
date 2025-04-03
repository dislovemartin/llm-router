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

//! Logging configuration for the LLM Router Gateway API
use env_logger::{Builder, Env};
use log::LevelFilter;
use std::io::Write;
use chrono::Local;

use crate::config::ObservabilityConfig;

/// Set up logging based on configuration
pub fn setup_logging(config: &ObservabilityConfig) {
    // Parse log level from config
    let log_level = match config.log_level.to_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    };
    
    // Create logger builder
    let mut builder = Builder::from_env(Env::default());
    
    if config.json_logging {
        // JSON structured logging
        builder.format(|buf, record| {
            let now = Local::now();
            let json = serde_json::json!({
                "timestamp": now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                "level": record.level().to_string(),
                "target": record.target().to_string(),
                "message": record.args().to_string(),
                "module": record.module_path().unwrap_or(""),
                "file": record.file().unwrap_or(""),
                "line": record.line().unwrap_or(0),
            });
            
            writeln!(buf, "{}", json)
        });
    } else {
        // Standard colored logging
        builder.format(|buf, record| {
            let now = Local::now();
            writeln!(
                buf,
                "{} [{}] [{}:{}] {}: {}",
                now.format("%Y-%m-%dT%H:%M:%S%.3f"),
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.target(),
                record.args()
            )
        });
    }
    
    // Set default log level
    builder.filter_level(log_level);
    
    // Apply configuration
    builder.init();
    
    log::info!(
        "Logging initialized with level {}, JSON formatting: {}",
        config.log_level,
        config.json_logging
    );
} 