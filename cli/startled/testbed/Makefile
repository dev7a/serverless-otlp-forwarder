# Color variables
GREEN=\033[0;32m
YELLOW=\033[1;33m
RED=\033[0;31m
RESET=\033[0m

.PHONY: help test-runtime test-all-runtimes report clean check

# Help target (default)
help:
	@echo "Available targets:"
	@echo "  $(YELLOW)test-runtime$(RESET)         Run benchmark for a specific runtime and memory configuration"
	@echo "  $(YELLOW)test-all-runtimes$(RESET)    Run benchmarks for all runtimes and memory configurations."
	@echo "  $(YELLOW)report$(RESET)               Generate benchmark report."
	@echo "  $(YELLOW)install$(RESET)              Install the startled CLI."
	@echo "  $(YELLOW)clean$(RESET)                Remove all generated result and report files."

# Memory configurations
MEMORY_CONFIGS := 128 512 1024

# Default number of concurrent executions and requests
CONCURRENCY ?= 10
REQUESTS ?= 100
STACK_NAME ?= benchmark
BASE_URL ?= /benchmark/reports

# Result and report directories (override with env vars if needed)
RESULT_DIR ?= results
REPORTS_DIR ?= reports

# List of runtimes to test
RUNTIMES := rust node python

# Default runtime for individual test (can override)
RUNTIME ?= rust


# Run benchmarks for all runtimes and memory configurations
test-all-runtimes: check
	@for runtime in $(RUNTIMES); do \
		for memory in $(MEMORY_CONFIGS); do \
			$(MAKE) test-runtime RUNTIME=$$runtime MEMORY_CONFIG=$$memory; \
		done; \
	done

# Run benchmark for a specific runtime and memory configuration
# Usage: make test-runtime RUNTIME=rust MEMORY_CONFIG=128

test-runtime:
	@if [ -z "$(MEMORY_CONFIG)" ]; then \
		echo "$(RED)Error: MEMORY_CONFIG is not set. Usage: make test-runtime RUNTIME=rust MEMORY_CONFIG=128$(RESET)"; \
		exit 1; \
	fi
	@echo "$(GREEN)> Running test for $(MEMORY_CONFIG) MB with $(RUNTIME) runtime...$(RESET)"
	@startled stack benchmark \
		-m "$(MEMORY_CONFIG)" \
		-c "$(CONCURRENCY)" \
		-n "$(REQUESTS)" \
		-s "$(RUNTIME)" \
		-d "/tmp/$(STACK_NAME)/$(RESULT_DIR)" \
		--proxy proxy --parallel

# Generate benchmark report
report:
	@echo "$(GREEN)Generating report...$(RESET)"
	@startled report -d "/tmp/$(STACK_NAME)/$(RESULT_DIR)" -o "/tmp/$(STACK_NAME)/$(REPORTS_DIR)" \
		--base-url "$(BASE_URL)" \
		--readme testbed.md 
	@echo "$(GREEN)Report generated successfully.$(RESET)"

# Install the startled CLI
install:
	@echo "$(GREEN)Installing startled CLI...$(RESET)"
	@cargo install --path ..

# Clean result and report directories
clean: check
	@echo "$(RED)Warning: This will delete all benchmark results and reports.$(RESET)\n"
	@echo "\t- results directory: /tmp/$(STACK_NAME)/$(RESULT_DIR)"
	@echo "\t- reports directory: /tmp/$(STACK_NAME)/$(REPORTS_DIR)\n"
	@read -p "Are you sure you want to proceed? (y/n) " confirm; \
	if [ "$$confirm" != "y" ]; then \
		echo "$(GREEN)Operation cancelled.$(RESET)"; \
		exit 1; \
	fi
	@echo "$(GREEN)Cleaning results and reports directory...$(RESET)"
	@echo "Removing:"
	@echo "\t- /tmp/$(STACK_NAME)/$(REPORTS_DIR)"
	@echo "\t- /tmp/$(STACK_NAME)/$(RESULT_DIR)"
	@echo "in 5 seconds..."
	@sleep 5
	@rm -rf "/tmp/$(STACK_NAME)/$(REPORTS_DIR)"
	@rm -rf "/tmp/$(STACK_NAME)/$(RESULT_DIR)"

check:
	@if echo "$(RESULT_DIR)" | grep -qE '(^/|(\.\.))'; then \
		echo "$(RED)Invalid RESULT_DIR: must be a relative path and not contain '..'.$(RESET)"; \
		exit 1; \
	fi
	@if echo "$(REPORTS_DIR)" | grep -qE '(^/|(\.\.))'; then \
		echo "$(RED)Invalid REPORTS_DIR: must be a relative path and not contain '..'.$(RESET)"; \
		exit 1; \
	fi
	@echo "$(GREEN)All directory inputs are safe.$(RESET)"
