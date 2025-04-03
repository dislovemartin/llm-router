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

//! LLM Router Gateway API
//! 
//! This crate provides a gateway for routing requests to LLM providers
//! based on selection criteria.

pub mod auth;
pub mod cache;
pub mod circuitbreaker;
pub mod client;
pub mod config;
pub mod error;
pub mod health;
pub mod loadbalance;
pub mod logging;
pub mod metrics;
pub mod nim;
pub mod proxy;
pub mod ratelimit;
pub mod retry;
pub mod stream;
pub mod triton;
