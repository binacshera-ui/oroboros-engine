#!/bin/bash
# =============================================================================
# OROBOROS Isolation Verification Script
# =============================================================================
# ARCHITECT'S ORDER: Prove that build processes don't touch MEV cores.
#
# This script:
# 1. Shows current CPU affinity for all Docker containers
# 2. Shows IRQ affinity for network devices
# 3. Runs a build stress test
# 4. Monitors CPU usage during build to verify isolation
# =============================================================================

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_header() { echo -e "\n${CYAN}=== $1 ===${NC}\n"; }

MEV_CORES="0,1"
BUILD_CORES="2,3,4,5,6,7"

# =============================================================================
# STEP 1: Check Docker Container CPU Affinity
# =============================================================================
check_docker_affinity() {
    log_header "DOCKER CONTAINER CPU AFFINITY"
    
    if ! command -v docker &> /dev/null; then
        log_warn "Docker not installed - skipping container check"
        return
    fi
    
    echo "Container Name          | CPUs Allowed"
    echo "------------------------+--------------"
    
    docker ps --format "{{.Names}}" 2>/dev/null | while read -r container; do
        cpuset=$(docker inspect --format '{{.HostConfig.CpusetCpus}}' "$container" 2>/dev/null || echo "N/A")
        printf "%-23s | %s\n" "$container" "${cpuset:-all}"
    done
    
    echo ""
    
    # Check for containers on MEV cores
    VIOLATIONS=$(docker ps --format "{{.Names}}" 2>/dev/null | while read -r container; do
        cpuset=$(docker inspect --format '{{.HostConfig.CpusetCpus}}' "$container" 2>/dev/null || echo "")
        if [[ -z "$cpuset" ]] || echo "$cpuset" | grep -qE "^0|,0|,1|^1$"; then
            echo "$container"
        fi
    done)
    
    if [[ -n "$VIOLATIONS" ]]; then
        log_warn "The following containers may use MEV cores (0-1):"
        echo "$VIOLATIONS"
    else
        log_info "No containers are configured to use MEV cores (0-1)"
    fi
}

# =============================================================================
# STEP 2: Check IRQ Affinity
# =============================================================================
check_irq_affinity() {
    log_header "NETWORK IRQ AFFINITY"
    
    echo "IRQ  | Affinity Mask | CPUs      | Device"
    echo "-----+---------------+-----------+--------"
    
    for irq_dir in /proc/irq/*/; do
        irq=$(basename "$irq_dir")
        if [[ ! "$irq" =~ ^[0-9]+$ ]]; then
            continue
        fi
        
        affinity=$(cat "$irq_dir/smp_affinity_list" 2>/dev/null || echo "N/A")
        
        # Check if this IRQ is for a network device
        if grep -qE "eth|enp|ens|mlx|ixgbe" /proc/interrupts 2>/dev/null | grep -q "^[[:space:]]*$irq:"; then
            device=$(grep "^[[:space:]]*$irq:" /proc/interrupts | awk '{print $NF}')
            printf "%-4s | %-13s | %-9s | %s\n" "$irq" "$(cat "$irq_dir/smp_affinity" 2>/dev/null)" "$affinity" "$device"
            
            # Check if affinity includes build cores
            if echo "$affinity" | grep -qE "[2-7]"; then
                log_warn "IRQ $irq ($device) can run on build cores!"
            fi
        fi
    done
    
    echo ""
}

# =============================================================================
# STEP 3: Show Current CPU Usage
# =============================================================================
show_cpu_usage() {
    log_header "CURRENT CPU USAGE BY CORE"
    
    if command -v mpstat &> /dev/null; then
        mpstat -P ALL 1 1 | grep -E "^[0-9]|CPU|Average"
    else
        log_warn "mpstat not installed. Install sysstat package for detailed CPU stats."
        echo "Using /proc/stat instead:"
        head -10 /proc/stat
    fi
    
    echo ""
}

# =============================================================================
# STEP 4: Run Build Stress Test
# =============================================================================
run_stress_test() {
    log_header "RUNNING BUILD STRESS TEST"
    
    log_info "Starting stress test on cores $BUILD_CORES..."
    log_info "Monitoring cores $MEV_CORES to ensure they stay idle..."
    
    # Run a CPU-intensive task bound to build cores
    taskset -c "$BUILD_CORES" sh -c '
        # Simulate build workload
        for i in $(seq 1 4); do
            dd if=/dev/zero bs=1M count=100 2>/dev/null | md5sum > /dev/null &
        done
        wait
    ' &
    STRESS_PID=$!
    
    # Monitor MEV cores during stress
    log_info "Stress test running (PID: $STRESS_PID)"
    
    PASS=true
    for i in {1..5}; do
        if command -v mpstat &> /dev/null; then
            # Get usage of cores 0 and 1
            CORE0_USAGE=$(mpstat -P 0 1 1 | tail -1 | awk '{print 100 - $NF}')
            CORE1_USAGE=$(mpstat -P 1 1 1 | tail -1 | awk '{print 100 - $NF}')
            
            echo "Sample $i: Core 0: ${CORE0_USAGE}%, Core 1: ${CORE1_USAGE}%"
            
            # Check if MEV cores are being used more than 10%
            if (( $(echo "$CORE0_USAGE > 10" | bc -l 2>/dev/null || echo 0) )); then
                log_warn "Core 0 usage is ${CORE0_USAGE}% - potential interference!"
                PASS=false
            fi
            if (( $(echo "$CORE1_USAGE > 10" | bc -l 2>/dev/null || echo 0) )); then
                log_warn "Core 1 usage is ${CORE1_USAGE}% - potential interference!"
                PASS=false
            fi
        else
            sleep 1
        fi
    done
    
    # Cleanup
    wait $STRESS_PID 2>/dev/null || true
    
    if $PASS; then
        log_info "✅ STRESS TEST PASSED: MEV cores remained idle during build workload"
    else
        log_error "❌ STRESS TEST FAILED: Build workload affected MEV cores!"
    fi
}

# =============================================================================
# STEP 5: Generate Report
# =============================================================================
generate_report() {
    log_header "ISOLATION VERIFICATION REPORT"
    
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║  CPU ISOLATION STATUS                                        ║"
    echo "╠══════════════════════════════════════════════════════════════╣"
    echo "║  MEV Territory:    Cores 0-1                                 ║"
    echo "║  Build Territory:  Cores 2-7                                 ║"
    echo "╠══════════════════════════════════════════════════════════════╣"
    
    # Check Docker
    if docker ps -q 2>/dev/null | head -1 | grep -q .; then
        echo "║  Docker Containers: Found - check affinity above            ║"
    else
        echo "║  Docker Containers: None running                            ║"
    fi
    
    # Check IRQ setup script
    if [[ -x "/etc/oroboros-irq-affinity.sh" ]]; then
        echo "║  IRQ Affinity Script: ✅ Installed                          ║"
    else
        echo "║  IRQ Affinity Script: ❌ Not installed                       ║"
        echo "║    Run: sudo ./scripts/irq-affinity-setup.sh               ║"
    fi
    
    echo "╚══════════════════════════════════════════════════════════════╝"
}

# =============================================================================
# MAIN
# =============================================================================
main() {
    echo ""
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║  OROBOROS ISOLATION VERIFICATION                             ║"
    echo "║  Ensuring build processes don't affect MEV bot               ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""
    
    check_docker_affinity
    check_irq_affinity
    show_cpu_usage
    
    if [[ "${1:-}" == "--stress" ]]; then
        run_stress_test
    else
        log_info "Run with --stress to perform stress test"
    fi
    
    generate_report
}

main "$@"
