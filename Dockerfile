# ---- Build Stage ----
# Use a specific Rust version for reproducibility
FROM rust:1.77 AS builder

WORKDIR /usr/src/app

# --- Optimized Dependency Caching for Workspace ---
# 1. Copy workspace manifests and lockfile
COPY llm-router/src/router-controller/Cargo.toml llm-router/src/router-controller/Cargo.lock ./

# 2. Copy individual crate manifests (essential for workspace dependency resolution)
#    We need to copy the manifest for the crate we intend to build, and potentially others it depends on within the workspace.
COPY llm-router/src/router-controller/crates/llm-router-gateway-api/Cargo.toml ./crates/llm-router-gateway-api/

# 3. Create dummy main.rs for the target crate to allow dependency fetching
RUN mkdir -p crates/llm-router-gateway-api/src && \
    echo "fn main() {}" > crates/llm-router-gateway-api/src/main.rs

# 4. Build only dependencies for the entire workspace context
#    This leverages Cargo's build cache effectively.
RUN cargo build --release --locked --package llm-router-gateway-api --bins
# Note: Building deps for the specific package might be sufficient if inter-workspace deps are simple.
# Alternative (builds all workspace deps): RUN cargo build --release --locked

# 5. Remove dummy source file(s) *after* dependency build
RUN rm -rf crates/llm-router-gateway-api/src

# --- Copy Full Source Code ---
# Copy the source code for the specific crate we are building
COPY llm-router/src/router-controller/crates/llm-router-gateway-api/src ./crates/llm-router-gateway-api/src
# Copy any other workspace crates needed for the build (if applicable)
# COPY llm-router/src/router-controller/crates/other-crate/src ./crates/other-crate/src

# Copy configuration if it's used during build (unlikely but possible)
# COPY llm-router/config ./config

# --- Build the Application Binary ---
# Build the specific package's binary
RUN cargo build --release --locked --package llm-router-gateway-api --bins

# ---- Runtime Stage ----
FROM debian:stable-slim AS runtime

WORKDIR /usr/local/bin

# Install runtime dependencies like ca-certificates
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
# Adjust the binary name based on Cargo.toml inspection
COPY --from=builder /usr/src/app/target/release/llm-router-gateway-api .

# Copy runtime configuration files (assuming they are at the project root relative to build context)
COPY llm-router/config ./config
# Copy other runtime assets if needed
# COPY llm-router/static ./static

# Set environment variables (adjust as needed)
# Example path
ENV RUST_LOG="info"
ENV CONFIG_PATH="/usr/local/bin/config/default.toml"

# Expose necessary ports (verify based on application needs)
EXPOSE 8080

# Define the entry point with the correct binary name
CMD ["/usr/local/bin/llm-router-gateway-api"] 