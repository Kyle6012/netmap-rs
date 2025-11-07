#!/bin/bash

# Netmap Installation Script for Linux
# This script helps install the netmap C library required for netmap-rs

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running as root
check_root() {
    if [[ $EUID -ne 0 ]]; then
        print_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

# Detect OS
detect_os() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        OS=$ID
        VERSION=$VERSION_ID
    else
        print_error "Cannot detect OS"
        exit 1
    fi
    print_status "Detected OS: $OS $VERSION"
}

# Install dependencies
install_dependencies() {
    print_status "Installing dependencies..."
    
    case $OS in
        ubuntu|debian)
            apt-get update
            apt-get install -y build-essential git linux-headers-$(uname -r)
            ;;
        centos|rhel|fedora)
            yum groupinstall -y "Development Tools"
            yum install -y git kernel-devel-$(uname -r)
            ;;
        arch|manjaro)
            pacman -S --noconfirm base-devel git linux-headers
            ;;
        *)
            print_warning "Unknown OS: $OS"
            print_warning "Please install build-essential, git, and kernel headers manually"
            read -p "Press Enter to continue after installing dependencies..."
            ;;
    esac
    
    print_success "Dependencies installed"
}

# Download and build netmap
build_netmap() {
    print_status "Downloading netmap..."
    
    if [[ -d netmap ]]; then
        print_warning "netmap directory already exists, removing..."
        rm -rf netmap
    fi
    
    git clone https://github.com/luigirizzo/netmap.git
    cd netmap/LINUX
    
    print_status "Configuring netmap..."
    ./configure
    
    print_status "Building netmap..."
    make
    
    print_status "Installing netmap..."
    make install
    
    print_success "Netmap built and installed"
    
    # Return to original directory
    cd ../..
}

# Load kernel module
load_module() {
    print_status "Loading netmap kernel module..."
    
    # Try to remove existing module first
    modprobe -r netmap 2>/dev/null || true
    
    # Load the module
    insmod netmap/LINUX/netmap.ko
    
    # Verify it's loaded
    if [[ -c /dev/netmap ]]; then
        print_success "Netmap kernel module loaded successfully"
    else
        print_error "Failed to load netmap kernel module"
        print_error "You may need to unload your network driver first:"
        print_error "  rmmod <your_nic_driver>"
        print_error "  insmod netmap.ko"
        print_error "  insmod <your_nic_driver>"
        exit 1
    fi
}

# Test installation
test_installation() {
    print_status "Testing netmap installation..."
    
    # Create a simple test program
    cat > test_netmap.c << 'EOF'
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/poll.h>
#include <net/netmap_user.h>

int main() {
    struct nm_desc *d;
    
    d = nm_open("netmap:lo", NULL, 0, NULL);
    if (d == NULL) {
        perror("nm_open");
        return 1;
    }
    
    printf("Successfully opened netmap:lo\n");
    nm_close(d);
    return 0;
}
EOF
    
    # Compile and run test
    if gcc -o test_netmap test_netmap.c -I/usr/local/include -L/usr/local/lib -lnetmap 2>/dev/null; then
        print_success "Test program compiled successfully"
        if ./test_netmap; then
            print_success "Netmap installation test passed"
            rm -f test_netmap test_netmap.c
        else
            print_warning "Test program failed to run (this may be normal if not running as root)"
            rm -f test_netmap test_netmap.c
        fi
    else
        print_warning "Failed to compile test program"
        rm -f test_netmap.c
    fi
}

# Print usage instructions
print_usage() {
    print_success "Netmap installation completed!"
    echo
    print_status "To use netmap-rs, add this to your Cargo.toml:"
    echo '  netmap-rs = { version = "0.3", features = ["sys"] }'
    echo
    print_status "Before running your application, load the kernel module:"
    echo "  sudo insmod /path/to/netmap.ko"
    echo
    print_status "If netmap is installed in a non-standard location, set:"
    echo "  export NETMAP_LOCATION=/path/to/netmap/installation"
    echo
    print_status "To run applications that use netmap, you typically need root privileges:"
    echo "  sudo ./your_netmap_application"
}

# Main installation function
main() {
    echo "==================================="
    echo " Netmap Installation Script"
    echo "==================================="
    echo
    
    check_root
    detect_os
    
    read -p "Do you want to proceed with installation? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_status "Installation cancelled"
        exit 0
    fi
    
    install_dependencies
    build_netmap
    load_module
    test_installation
    print_usage
}

# Handle command line arguments
case "${1:-install}" in
    install)
        main
        ;;
    build)
        check_root
        detect_os
        build_netmap
        ;;
    load)
        check_root
        load_module
        ;;
    test)
        test_installation
        ;;
    *)
        echo "Usage: $0 [install|build|load|test]"
        echo "  install - Full installation (default)"
        echo "  build   - Only build netmap"
        echo "  load    - Only load kernel module"
        echo "  test    - Only test installation"
        exit 1
        ;;
esac