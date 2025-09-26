# 1. Detect the Operating System
# Check for Windows_NT environment variable, which is set on Windows
ifeq ($(OS),Windows_NT)
    # Windows settings
    RUNNER := target\release\runner.exe
    # Use backslash for Windows paths in dependencies for consistency
    RUNNER_SRC_DIR := runner\src

	KERNEL_DEBUG := target\x86_64-unknown-none\debug\blog_os
	KERNEL_RELEASE := target\x86_64-unknown-none\release\blog_os
	KERNEL_SRC_DIR := kernel\src
else
    # Linux/Unix-like settings
    RUNNER := target/release/runner
    # Use forward slash for Unix paths
    RUNNER_SRC_DIR := runner/src
endif

# ---

## Variables for Dependencies

# Find all source files (*.rs) recursively in the runner/src directory
RUNNER_SOURCES := $(wildcard $(RUNNER_SRC_DIR)/*.rs) $(wildcard $(RUNNER_SRC_DIR)/**/*.rs)

# List of critical files that, if changed, require a full recompilation
RUNNER_MANIFESTS := Cargo.toml runner/Cargo.toml

# The main list of dependencies for the runner executable
RUNNER_DEPS := $(RUNNER_SOURCES) $(RUNNER_MANIFESTS)

# Find all source files (*.rs) recursively in the runner/src directory
KERNEL_SOURCES := $(wildcard $(KERNEL_SRC_DIR)/*.rs) $(wildcard $(KERNEL_SRC_DIR)/**/*.rs)

# List of critical files that, if changed, require a full recompilation
KERNEL_MANIFESTS := Cargo.toml kernel/Cargo.toml

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
	@cargo build -p runner --release

$(KERNEL_DEBUG): $(KERNEL_DEPS)
	@echo Detected changes in kernel dependencies. Recompiling...
	@cargo build -p blog_os --target x86_64-unknown-none

$(KERNEL_RELEASE): $(KERNEL_DEPS)
	@echo Detected changes in kernel dependencies. Recompiling...
	@cargo build -p blog_os --target x86_64-unknown-none --release


.PHONY: run
run: $(KERNEL_DEPS) $(RUNNER)
	@cargo run -p blog_os --target x86_64-unknown-none

.PHONY: test
test: $(KERNEL_DEPS) $(RUNNER)
	@cargo test -p blog_os --target x86_64-unknown-none

.PHONY: clean
clean:
	@cargo clean

.PHONY: build-debug
build-debug: $(KERNEL_DEBUG) $(RUNNER)
	@$(RUNNER) --target target --build $(KERNEL_DEBUG)
.PHONY: build-release
build-release: $(KERNEL_RELEASE) $(RUNNER)
	@$(RUNNER) --target target --build $(KERNEL_RELEASE)


