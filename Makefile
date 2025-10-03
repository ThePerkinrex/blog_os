# 1. Detect the Operating System
# Check for Windows_NT environment variable, which is set on Windows
RUNNER_BASE := runner
KERNEL_BASE := kernel

ifeq ($(OS),Windows_NT)
    # Windows settings
    RUNNER := $(RUNNER_BASE)\target\release\runner.exe
    # Use backslash for Windows paths in dependencies for consistency
    RUNNER_SRC_DIR := $(RUNNER_BASE)\src

	KERNEL_DEBUG := $(KERNEL_BASE)\target\x86_64-unknown-none\debug\blog_os
	KERNEL_RELEASE := $(KERNEL_BASE)\target\x86_64-unknown-none\release\blog_os
	KERNEL_SRC_DIR := $(KERNEL_BASE)\src
else
    # Linux/Unix-like settings
    RUNNER := $(RUNNER_BASE)/target/release/runner
    # Use forward slash for Unix paths
    RUNNER_SRC_DIR := $(RUNNER_BASE)/src

	KERNEL_DEBUG := $(KERNEL_BASE)/target/x86_64-unknown-none/debug/blog_os
	KERNEL_RELEASE := $(KERNEL_BASE)/target/x86_64-unknown-none/release/blog_os
	KERNEL_SRC_DIR := $(KERNEL_BASE)/src
endif

# ---

## Variables for Dependencies

# Find all source files (*.rs) recursively in the runner/src directory
RUNNER_SOURCES := $(wildcard $(RUNNER_SRC_DIR)/*.rs) $(wildcard $(RUNNER_SRC_DIR)/**/*.rs)

# List of critical files that, if changed, require a full recompilation
RUNNER_MANIFESTS := $(RUNNER_BASE)/Cargo.toml

# The main list of dependencies for the runner executable
RUNNER_DEPS := $(RUNNER_SOURCES) $(RUNNER_MANIFESTS)

# Find all source files (*.rs) recursively in the runner/src directory
KERNEL_SOURCES := $(wildcard $(KERNEL_SRC_DIR)/*.rs) $(wildcard $(KERNEL_SRC_DIR)/**/*.rs)

# List of critical files that, if changed, require a full recompilation
KERNEL_MANIFESTS := $(KERNEL_BASE)/Cargo.toml

# The main list of dependencies for the runner executable
KERNEL_DEPS := $(KERNEL_SOURCES) $(KERNEL_MANIFESTS)


# ---

# Ensure that if the target file is deleted or missing, the rule is run.
# The dependencies will ensure it only runs when necessary.
.PHONY: all
all: $(RUNNER)

## Rule to Build the Runner

# The target is the runner executable, which depends on all source files and manifests
$(RUNNER): $(RUNNER_DEPS)
	@echo Detected changes in runner dependencies. Recompiling...
	@cd $(RUNNER_BASE) && cargo build --release

$(KERNEL_DEBUG): $(KERNEL_DEPS)
	@echo Detected changes in kernel dependencies. Recompiling...
	@cd $(KERNEL_BASE) && cargo build

$(KERNEL_RELEASE): $(KERNEL_DEPS)
	@echo Detected changes in kernel dependencies. Recompiling...
	@cd $(KERNEL_BASE) && cargo build --release


.PHONY: run
run: $(KERNEL_DEPS) $(RUNNER)
	@cd $(KERNEL_BASE) && cargo run

.PHONY: run-gdb
run-gdb: $(KERNEL_DEPS) $(RUNNER)
	@cd $(KERNEL_BASE) && GDB_LISTEN=true cargo run

.PHONY: test
test: $(KERNEL_DEPS) $(RUNNER)
	@cd $(KERNEL_BASE) && cargo test --lib && cargo test --bin blog_os

.PHONY: clean
clean:
	@cd $(RUNNER_BASE) && cargo clean
	@cd $(KERNEL_BASE) && cargo clean

.PHONY: fmt
fmt:
	@cd $(RUNNER_BASE) && cargo fmt
	@cd $(KERNEL_BASE) && cargo fmt
	

.PHONY: build-debug
build-debug: $(KERNEL_DEBUG) $(RUNNER)
	@$(RUNNER) --target $(KERNEL_BASE)/target --build $(KERNEL_DEBUG)
.PHONY: build-release
build-release: $(KERNEL_RELEASE) $(RUNNER)
	@$(RUNNER) --target $(KERNEL_BASE)/target --build $(KERNEL_RELEASE)


