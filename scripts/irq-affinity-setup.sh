#!/bin/bash
# =============================================================================
# OROBOROS IRQ Affinity Setup
# =============================================================================
# PROBLEM: Network card interrupts (IRQs) can land on any CPU core.
# If the MEV bot's network traffic gets processed on a build core,
# we lose money.
#
# SOLUTION: Pin network IRQs to cores 0-1 (MEV territory).
# Build processes run on cores 2-7.
#
# MUST RUN AS ROOT.
# =============================================================================

set -euo pipefail

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_header() { echo -e "\n${CYAN}=== $1 ===${NC}"; }

# Check root
if [[ $EUID -ne 0 ]]; then
    log_error "This script must be run as root"
    exit 1
fi

# =============================================================================
# STEP 1: Analyze current IRQ distribution
# =============================================================================
log_header "CURRENT IRQ DISTRIBUTION"

echo "Top 20 IRQs by interrupt count:"
echo "IRQ | CPU0 | CPU1 | CPU2 | CPU3 | ... | Device"
echo "----+------+------+------+------+-----+--------"

# Show current interrupt distribution
head -30 /proc/interrupts

# =============================================================================
# STEP 2: Find network device IRQs
# =============================================================================
log_header "NETWORK DEVICE IRQs"

# Common network device names
NET_DEVICES=$(ls /sys/class/net | grep -v lo)
log_info "Network devices found: $NET_DEVICES"

# Find IRQs for each network device
for dev in $NET_DEVICES; do
    log_info "Device: $dev"
    
    # Try to find IRQs via /proc/interrupts
    IRQS=$(grep -E "$dev|eth|enp|ens" /proc/interrupts | awk '{print $1}' | tr -d ':' || true)
    
    if [[ -n "$IRQS" ]]; then
        echo "  IRQs: $IRQS"
    else
        log_warn "  No IRQs found in /proc/interrupts for $dev"
    fi
    
    # Check MSI-X vectors if available
    if [[ -d "/sys/class/net/$dev/device/msi_irqs" ]]; then
        MSI_IRQS=$(ls /sys/class/net/$dev/device/msi_irqs 2>/dev/null || true)
        if [[ -n "$MSI_IRQS" ]]; then
            echo "  MSI-X IRQs: $MSI_IRQS"
        fi
    fi
done

# =============================================================================
# STEP 3: Show current affinity settings
# =============================================================================
log_header "CURRENT IRQ AFFINITY"

echo "Format: IRQ -> CPU mask (hex) -> CPU list"
echo ""

for irq_dir in /proc/irq/*/; do
    irq=$(basename "$irq_dir")
    if [[ "$irq" =~ ^[0-9]+$ ]]; then
        affinity=$(cat "$irq_dir/smp_affinity" 2>/dev/null || echo "N/A")
        affinity_list=$(cat "$irq_dir/smp_affinity_list" 2>/dev/null || echo "N/A")
        
        # Only show if it's a potentially important IRQ
        if grep -q "eth\|enp\|ens\|nvme\|mlx" "$irq_dir/../interrupts" 2>/dev/null; then
            echo "IRQ $irq: mask=$affinity, cpus=$affinity_list"
        fi
    fi
done

# =============================================================================
# STEP 4: Pin network IRQs to MEV cores (0-1)
# =============================================================================
log_header "PINNING NETWORK IRQs TO CORES 0-1"

# CPU mask for cores 0-1: binary 0011 = hex 0x3
MEV_CPU_MASK="3"
MEV_CPU_LIST="0-1"

pin_irq_to_mev() {
    local irq=$1
    local name=$2
    
    if [[ -f "/proc/irq/$irq/smp_affinity" ]]; then
        echo "$MEV_CPU_MASK" > "/proc/irq/$irq/smp_affinity" 2>/dev/null || {
            log_warn "Failed to set affinity for IRQ $irq ($name)"
            return 1
        }
        log_info "Pinned IRQ $irq ($name) to CPUs $MEV_CPU_LIST"
    fi
}

# Find and pin all network-related IRQs
while IFS= read -r line; do
    if echo "$line" | grep -qE "eth|enp|ens|mlx|ixgbe|i40e|ice"; then
        irq=$(echo "$line" | awk '{print $1}' | tr -d ':')
        name=$(echo "$line" | awk '{print $NF}')
        pin_irq_to_mev "$irq" "$name"
    fi
done < /proc/interrupts

# =============================================================================
# STEP 5: Pin NVMe IRQs to build cores (optional - depends on setup)
# =============================================================================
log_header "NVMe IRQ CONFIGURATION"

# NVMe can stay on build cores since MEV bot is network-bound, not disk-bound
BUILD_CPU_MASK="fc"  # binary 11111100 = cores 2-7
BUILD_CPU_LIST="2-7"

while IFS= read -r line; do
    if echo "$line" | grep -qE "nvme"; then
        irq=$(echo "$line" | awk '{print $1}' | tr -d ':')
        name=$(echo "$line" | awk '{print $NF}')
        
        if [[ -f "/proc/irq/$irq/smp_affinity" ]]; then
            echo "$BUILD_CPU_MASK" > "/proc/irq/$irq/smp_affinity" 2>/dev/null || {
                log_warn "Failed to set affinity for IRQ $irq ($name)"
                continue
            }
            log_info "Pinned IRQ $irq ($name) to CPUs $BUILD_CPU_LIST (build territory)"
        fi
    fi
done < /proc/interrupts

# =============================================================================
# STEP 6: Verify new configuration
# =============================================================================
log_header "VERIFICATION"

echo ""
echo "Network IRQs should now be pinned to CPUs 0-1:"
echo ""

for irq_dir in /proc/irq/*/; do
    irq=$(basename "$irq_dir")
    if [[ "$irq" =~ ^[0-9]+$ ]]; then
        affinity_list=$(cat "$irq_dir/smp_affinity_list" 2>/dev/null || echo "N/A")
        
        # Check network IRQs
        if grep -qE "eth|enp|ens|mlx" /proc/interrupts | grep -q "^[[:space:]]*$irq:"; then
            echo "IRQ $irq: CPUs $affinity_list"
        fi
    fi
done

# =============================================================================
# STEP 7: Create persistent configuration
# =============================================================================
log_header "CREATING PERSISTENT CONFIGURATION"

PERSISTENT_SCRIPT="/etc/oroboros-irq-affinity.sh"

cat > "$PERSISTENT_SCRIPT" << 'SCRIPT'
#!/bin/bash
# Auto-generated by OROBOROS IRQ setup
# Run this at boot to restore IRQ affinity

MEV_CPU_MASK="3"  # Cores 0-1

while IFS= read -r line; do
    if echo "$line" | grep -qE "eth|enp|ens|mlx|ixgbe|i40e|ice"; then
        irq=$(echo "$line" | awk '{print $1}' | tr -d ':')
        echo "$MEV_CPU_MASK" > "/proc/irq/$irq/smp_affinity" 2>/dev/null
    fi
done < /proc/interrupts
SCRIPT

chmod +x "$PERSISTENT_SCRIPT"
log_info "Created persistent script: $PERSISTENT_SCRIPT"
log_info "Add to /etc/rc.local or systemd to run at boot"

# =============================================================================
# STEP 8: Summary
# =============================================================================
log_header "SUMMARY"

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  IRQ AFFINITY CONFIGURATION                                  ║"
echo "╠══════════════════════════════════════════════════════════════╣"
echo "║  MEV Bot Territory:                                          ║"
echo "║    - CPUs: 0-1                                               ║"
echo "║    - Network IRQs pinned here                                ║"
echo "║                                                              ║"
echo "║  Build Territory:                                            ║"
echo "║    - CPUs: 2-7                                               ║"
echo "║    - Rust compilation runs here                              ║"
echo "║    - NVMe IRQs pinned here (disk I/O for build)              ║"
echo "╠══════════════════════════════════════════════════════════════╣"
echo "║  IMPORTANT: Re-run this script after:                        ║"
echo "║    - System reboot                                           ║"
echo "║    - Network driver reload                                   ║"
echo "║    - Adding new network interfaces                           ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

log_info "IRQ affinity setup complete."
