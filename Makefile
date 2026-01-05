# FCM Receiver Server - Build Makefile
# Cross-platform build targets

BINARY_NAME = fcm_recv
VERSION = $(shell grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)

# Output directory
DIST_DIR = dist

# Default target
.PHONY: all
all: build

# Development build
.PHONY: build
build:
	cargo build --release

# Build for current platform
.PHONY: release
release: clean
	cargo build --release
	mkdir -p $(DIST_DIR)
	cp target/release/$(BINARY_NAME) $(DIST_DIR)/

# ===== Cross-platform builds =====

# Install cross (required for cross-compilation)
# Using version 0.2.4 for rustc < 1.92 compatibility
.PHONY: install-cross
install-cross:
	cargo install cross --version 0.2.4

# Build for Linux x86_64 (most servers)
.PHONY: linux
linux:
	cross build --release --target x86_64-unknown-linux-gnu
	mkdir -p $(DIST_DIR)
	cp target/x86_64-unknown-linux-gnu/release/$(BINARY_NAME) $(DIST_DIR)/$(BINARY_NAME)-linux-amd64

# Build for Linux ARM64 (Raspberry Pi, AWS Graviton)
.PHONY: linux-arm
linux-arm:
	cross build --release --target aarch64-unknown-linux-gnu
	mkdir -p $(DIST_DIR)
	cp target/aarch64-unknown-linux-gnu/release/$(BINARY_NAME) $(DIST_DIR)/$(BINARY_NAME)-linux-arm64

# Build for macOS x86_64
.PHONY: macos
macos:
	cargo build --release --target x86_64-apple-darwin
	mkdir -p $(DIST_DIR)
	cp target/x86_64-apple-darwin/release/$(BINARY_NAME) $(DIST_DIR)/$(BINARY_NAME)-darwin-amd64

# Build for macOS ARM64 (M1/M2/M3)
.PHONY: macos-arm
macos-arm:
	cargo build --release --target aarch64-apple-darwin
	mkdir -p $(DIST_DIR)
	cp target/aarch64-apple-darwin/release/$(BINARY_NAME) $(DIST_DIR)/$(BINARY_NAME)-darwin-arm64

# Build for Windows x86_64
.PHONY: windows
windows:
	cross build --release --target x86_64-pc-windows-gnu
	mkdir -p $(DIST_DIR)
	cp target/x86_64-pc-windows-gnu/release/$(BINARY_NAME).exe $(DIST_DIR)/$(BINARY_NAME)-windows-amd64.exe

# Build all platforms
.PHONY: all-platforms
all-platforms: linux linux-arm macos macos-arm windows
	@echo "All builds complete in $(DIST_DIR)/"
	@ls -la $(DIST_DIR)/

# ===== Testing =====

.PHONY: test
test:
	cargo test

.PHONY: test-api
test-api:
	./test_api.sh

# ===== Utilities =====

.PHONY: clean
clean:
	cargo clean
	rm -rf $(DIST_DIR)

.PHONY: run
run:
	cargo run --bin $(BINARY_NAME)

.PHONY: run-release
run-release:
	cargo run --release --bin $(BINARY_NAME)

# Check code
.PHONY: check
check:
	cargo check
	cargo clippy

# Format code
.PHONY: fmt
fmt:
	cargo fmt

# Show binary size
.PHONY: size
size: build
	@ls -lh target/release/$(BINARY_NAME)

# Create distribution archive
.PHONY: dist
dist: release
	cd $(DIST_DIR) && tar -czvf $(BINARY_NAME)-$(VERSION)-$(shell uname -s)-$(shell uname -m).tar.gz $(BINARY_NAME)
	@echo "Created: $(DIST_DIR)/$(BINARY_NAME)-$(VERSION)-$(shell uname -s)-$(shell uname -m).tar.gz"

# Docker build (optional)
.PHONY: docker
docker:
	docker build -t $(BINARY_NAME):$(VERSION) .
	docker tag $(BINARY_NAME):$(VERSION) $(BINARY_NAME):latest

.PHONY: help
help:
	@echo "FCM Receiver Server - Build Targets"
	@echo ""
	@echo "Development:"
	@echo "  make build        - Build debug version"
	@echo "  make release      - Build release version"
	@echo "  make run          - Run development server"
	@echo "  make test         - Run tests"
	@echo "  make test-api     - Run API tests"
	@echo ""
	@echo "Cross-platform (requires 'cross' tool):"
	@echo "  make install-cross - Install cross-compilation tool"
	@echo "  make linux        - Build for Linux x86_64"
	@echo "  make linux-arm    - Build for Linux ARM64"
	@echo "  make macos        - Build for macOS x86_64"
	@echo "  make macos-arm    - Build for macOS ARM64 (M1/M2)"
	@echo "  make windows      - Build for Windows x86_64"
	@echo "  make all-platforms - Build all platforms"
	@echo ""
	@echo "Utilities:"
	@echo "  make clean        - Clean build artifacts"
	@echo "  make check        - Run cargo check and clippy"
	@echo "  make fmt          - Format code"
	@echo "  make dist         - Create distribution archive"
	@echo "  make size         - Show binary size"
