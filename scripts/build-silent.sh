#!/bin/bash
# =============================================================================
# OROBOROS Silent Build Script
# =============================================================================
# THE CAGE OF SILENCE: Builds without disturbing the MEV bot.
#
# ARCHITECT'S REQUIREMENTS:
# - CPU Affinity: Bind to specific cores only
# - IO Priority: Best-effort (lowest)
# - Nice Level: 19 (lowest priority)
# - Memory: Controlled allocation
#
# Usage:
#   ./scripts/build-silent.sh [build|test|bench|all]
# =============================================================================

set -euo pipefail

# Configuration
ALLOWED_CORES="${OROBOROS_BUILD_CORES:-2,3,4,5,6,7}"  # Default: cores 2-7
NICE_LEVEL="${OROBOROS_BUILD_NICE:-19}"                # Lowest priority
IO_CLASS="${OROBOROS_BUILD_IO_CLASS:-3}"               # Best-effort (idle)
MAX_JOBS="${OROBOROS_BUILD_JOBS:-4}"                   # Parallel jobs

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're root (needed for taskset/ionice)
check_privileges() {
    if [[ $EUID -ne 0 ]]; then
        log_warn "Not running as root. CPU affinity and IO priority may not be enforced."
        log_warn "For full isolation, run with: sudo $0 $*"
        return 1
    fi
    return 0
}

# Build with resource constraints
run_constrained() {
    local cmd="$*"
    
    if check_privileges; then
        # Full constraints available
        log_info "Running with full resource constraints"
        exec taskset -c "${ALLOWED_CORES}" \
             ionice -c "${IO_CLASS}" \
             nice -n "${NICE_LEVEL}" \
             bash -c "${cmd}"
    else
        # Best effort without root
        log_info "Running with limited constraints (nice only)"
        exec nice -n "${NICE_LEVEL}" bash -c "${cmd}"
    fi
}

# Build command
do_build() {
    log_info "Starting silent build..."
    log_info "CPU Cores: ${ALLOWED_CORES}"
    log_info "Nice Level: ${NICE_LEVEL}"
    log_info "IO Class: ${IO_CLASS}"
    
    run_constrained "CARGO_BUILD_JOBS=${MAX_JOBS} cargo build --release --workspace"
}

# Test command
do_test() {
    log_info "Running tests..."
    run_constrained "cargo test --release --workspace --no-fail-fast"
}

# Benchmark command
do_bench() {
    log_info "Running benchmarks..."
    log_info "NOTE: Benchmarks need consistent CPU state for accurate results"
    
    # For benchmarks, we want dedicated cores but still not affect MEV
    run_constrained "cargo bench --workspace -- --noplot"
}

# Full CI pipeline
do_all() {
    log_info "Running full CI pipeline..."
    
    local commands="
        echo '=== CHECKING FORMAT ===' &&
        cargo fmt --check &&
        echo '=== CHECKING LINTS ===' &&
        cargo clippy --workspace --all-targets -- -D warnings &&
        echo '=== BUILDING ===' &&
        CARGO_BUILD_JOBS=${MAX_JOBS} cargo build --release --workspace &&
        echo '=== TESTING ===' &&
        cargo test --release --workspace &&
        echo '=== CHECKING DOCS ===' &&
        cargo doc --no-deps --workspace &&
        echo '=== BENCHMARKING ===' &&
        cargo bench --workspace -- --noplot &&
        echo '=== ALL CHECKS PASSED ==='
    "
    
    run_constrained "${commands}"
}

# Validate MEV bot is not affected
validate_isolation() {
    log_info "Validating build isolation..."
    
    # Check CPU usage during build
    local test_cmd="
        cargo build --release -p oroboros_core &
        BUILD_PID=\$!
        
        # Monitor for 5 seconds
        for i in {1..5}; do
            # Get CPU usage of core 0-1 (MEV cores)
            CORE_USAGE=\$(mpstat -P 0,1 1 1 | tail -n 2 | awk '{sum+=\$3} END {print sum}')
            echo \"MEV cores usage: \${CORE_USAGE}%\"
            
            if (( \$(echo \"\${CORE_USAGE} > 20\" | bc -l) )); then
                echo 'WARNING: Build is affecting MEV cores!'
                kill \$BUILD_PID 2>/dev/null
                exit 1
            fi
        done
        
        wait \$BUILD_PID
        echo 'Build completed without affecting MEV cores'
    "
    
    run_constrained "${test_cmd}"
}

# Help
show_help() {
    echo "OROBOROS Silent Build Script"
    echo ""
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  build     - Build all crates in release mode"
    echo "  test      - Run all tests"
    echo "  bench     - Run benchmarks"
    echo "  all       - Run full CI pipeline"
    echo "  validate  - Test that build doesn't affect MEV bot"
    echo "  help      - Show this help"
    echo ""
    echo "Environment Variables:"
    echo "  OROBOROS_BUILD_CORES    - CPU cores to use (default: 2,3,4,5,6,7)"
    echo "  OROBOROS_BUILD_NICE     - Nice level (default: 19)"
    echo "  OROBOROS_BUILD_IO_CLASS - IO class (default: 3/idle)"
    echo "  OROBOROS_BUILD_JOBS     - Parallel jobs (default: 4)"
}

# Main
case "${1:-build}" in
    build)
        do_build
        ;;
    test)
        do_test
        ;;
    bench)
        do_bench
        ;;
    all)
        do_all
        ;;
    validate)
        validate_isolation
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        log_error "Unknown command: $1"
        show_help
        exit 1
        ;;
esac
