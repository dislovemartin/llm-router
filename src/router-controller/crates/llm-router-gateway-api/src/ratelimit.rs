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

//! Rate limiting for the LLM Router Gateway API
use std::convert::TryFrom;
use std::num::NonZeroU32;
use std::sync::Arc;
use log::debug;

use governor::{Quota, RateLimiter, clock::DefaultClock};
use governor::state::{InMemoryState, NotKeyed};

use crate::config::RouterConfig;

/// Create a rate limiter
pub fn create_rate_limiter(config: &RouterConfig) -> Option<Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>> {
    // If rate limiting is disabled or not configured, return None
    let rate_limit = match &config.security.rate_limit {
        Some(rl) => rl,
        None => return None,
    };
    
    // Build rate limiter
    let rate = (rate_limit.requests_per_second as u32).max(1);
    let burst = (rate_limit.burst_size as u32).max(1);
    
    // Create NonZero values
    let rate_nz = NonZeroU32::try_from(rate).unwrap_or(NonZeroU32::new(1).unwrap());
    let burst_nz = NonZeroU32::try_from(burst).unwrap_or(NonZeroU32::new(1).unwrap());
    
    // Configure quota
    let quota = Quota::per_second(rate_nz).allow_burst(burst_nz);
    
    debug!(
        "Configuring rate limiting with {} requests per second, burst size of {}{}",
        rate,
        burst,
        if rate_limit.per_ip { ", per IP" } else { "" }
    );
    
    Some(Arc::new(RateLimiter::direct(quota)))
} 