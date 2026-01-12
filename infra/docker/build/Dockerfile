# =============================================================================
# OROBOROS Build Container
# =============================================================================
# Optimized for silent, low-priority builds that don't disturb the MEV bot.
#
# ARCHITECT'S REQUIREMENTS:
# - CPU Affinity: Bind to specific cores
# - IO Priority: Low priority for disk operations
# - Memory: Limited to prevent OOM affecting other processes
# =============================================================================

FROM rust:1.75-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    cmake \
    git \
    util-linux \
    && rm -rf /var/lib/apt/lists/*

# Set environment for reproducible builds
ENV CARGO_HOME=/usr/local/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV PATH="${CARGO_HOME}/bin:${PATH}"

# Pre-install sccache for faster builds (optional)
# RUN cargo install sccache --locked

# Configure Cargo for aggressive optimization
RUN mkdir -p /root/.cargo && \
    echo '[build]' > /root/.cargo/config.toml && \
    echo 'jobs = 4' >> /root/.cargo/config.toml && \
    echo '' >> /root/.cargo/config.toml && \
    echo '[profile.release]' >> /root/.cargo/config.toml && \
    echo 'lto = "fat"' >> /root/.cargo/config.toml && \
    echo 'codegen-units = 1' >> /root/.cargo/config.toml

WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock* ./
COPY crates/ ./crates/

# Build with low priority
# The ionice and nice commands ensure we don't starve the MEV bot
CMD ["sh", "-c", "ionice -c 3 nice -n 19 cargo build --release"]

# =============================================================================
# Runtime Container (minimal)
# =============================================================================
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    util-linux \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy built binary from builder
COPY --from=builder /build/target/release/oroboros* ./

# Non-root user for security
RUN useradd -m -s /bin/bash oroboros
USER oroboros

ENTRYPOINT ["/app/oroboros"]
