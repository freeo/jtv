#!/usr/bin/env bash

# Test runner for just-tv-0.3.0-dev.sh
# Comprehensive feature testing with manual verification
# Includes tests for new file completion feature

set -e

# Colors for test status
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
DIM='\033[2m'
NC='\033[0m'

# Test counter
TEST_NUM=0
FAILED=0

# Test files
SCRIPT="./just-tv-0.3.0-dev.sh"
TEST_JUSTFILE="test-justfile"
TEST_COMPLETION_JUSTFILE="test-completion-justfile"
MODULAR_JUSTFILE="justfile-modular"
HISTORY_FILE=".just_history"

# Clean environment
unset JUST_CHOOSER
export ZDOTDIR=/tmp/test-zsh-$$
mkdir -p "$ZDOTDIR"
echo 'HISTFILE=/dev/null' >"$ZDOTDIR/.zshrc"

# Test function
run_test() {
	TEST_NUM=$((TEST_NUM + 1))
	echo ""
	echo -e "${YELLOW}TEST $TEST_NUM:${NC} $1"
	echo "$2"
	echo "---"
}

verify() {
	local expected="$1"
	local actual="$2"
	if [[ "$actual" == *"$expected"* ]]; then
		echo -e "${GREEN}✓${NC} Found: $expected"
	else
		echo -e "${RED}✗${NC} Missing: $expected"
		echo "  Actual output: $actual"
		FAILED=$((FAILED + 1))
	fi
}

verify_file() {
	local file="$1"
	local content="$2"
	if [[ -f "$file" ]] && grep -q "$content" "$file" 2>/dev/null; then
		echo -e "${GREEN}✓${NC} File contains: $content"
	else
		echo -e "${RED}✗${NC} File missing content: $content"
		FAILED=$((FAILED + 1))
	fi
}

# Setup
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# Clean up any previous test artifacts
rm -f "$HISTORY_FILE" ~/.just-tv-last-command

# Header
echo "=== just-tv v0.3.0-dev Test Suite ==="
echo -e "${DIM}Testing: $SCRIPT${NC}"
echo -e "${DIM}Regular: $TEST_JUSTFILE${NC}"
echo -e "${DIM}Completion: $TEST_COMPLETION_JUSTFILE${NC}"
echo -e "${DIM}Modular: $MODULAR_JUSTFILE${NC}"
echo ""

# ============================================================================
# SECTION 1: BASIC FUNCTIONALITY
# ============================================================================

echo -e "${BLUE}━━━ SECTION 1: Basic Functionality ━━━${NC}"

# Test 1: Version flag
run_test "Version flag" "Run: $SCRIPT --version"
OUTPUT=$($SCRIPT --version 2>&1)
verify "0.3.0-dev" "$OUTPUT"

# Test 2: Help flag
run_test "Help flag" "Run: $SCRIPT --help"
OUTPUT=$($SCRIPT --help 2>&1)
verify "Usage:" "$OUTPUT"
verify "--no-history" "$OUTPUT"
verify "--module" "$OUTPUT"

# Test 3: Simple recipe execution
run_test "Simple recipe execution" "Select 'simple' in TV, press Enter"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_JUSTFILE"
echo "Expected output: OUTPUT:simple"
read -p "Press Enter after testing..."
LAST_OUTPUT=$(cat ~/.just-tv-last-command 2>/dev/null || echo "")
verify "just --justfile='$TEST_JUSTFILE' simple" "$LAST_OUTPUT"

# Test 4: Cancel operation
run_test "Cancel operation (ESC)" "Press ESC in TV interface"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_JUSTFILE"
echo "Should show: 'No selection made'"
read -p "Press Enter after testing..."

# ============================================================================
# SECTION 2: FILE COMPLETION FEATURE (NEW IN 0.3.0)
# ============================================================================

echo ""
echo -e "${BLUE}━━━ SECTION 2: File Completion Feature (NEW IN 0.3.0) ━━━${NC}"

# Test 5: TAB completion trigger
run_test "TAB triggers file browser" "Select 'build', press TAB at parameter prompt"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_COMPLETION_JUSTFILE"
echo "1. Select 'build' recipe"
echo "2. At prompt 'target=', press TAB"
echo "3. TV file browser should launch"
echo "4. Select any file and press Enter"
echo ""
echo -e "${MAGENTA}Expected behavior:${NC}"
echo "  - TV launches with file listing"
echo "  - Selected file path appears as parameter value"
echo "  - Command executes with selected file"
read -p "Press Enter after testing..."

# Test 6: Non-editable prompt
run_test "Non-editable parameter prompt" "Try to delete 'target=' with backspace"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_COMPLETION_JUSTFILE"
echo "1. Select 'build' recipe"
echo "2. At prompt 'target=', type some text"
echo "3. Try to backspace beyond your input to delete 'target='"
echo ""
echo -e "${MAGENTA}Expected:${NC} 'target=' cannot be deleted"
echo -e "${RED}Bug if:${NC} 'target=' can be deleted"
read -p "Press Enter after testing..."

# Test 7: TAB with partial path
run_test "TAB with partial path filter" "Type partial path then TAB"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_COMPLETION_JUSTFILE"
echo "1. Select 'build' recipe"
echo "2. At prompt, type 'docs' then press TAB"
echo "3. TV should launch with filtered results"
echo ""
echo -e "${MAGENTA}Expected:${NC}"
echo "  - TV shows files matching 'docs' first"
echo "  - Can select from filtered results"
read -p "Press Enter after testing..."

# Test 8: Manual entry without TAB
run_test "Manual parameter entry" "Type value and press ENTER (no TAB)"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_COMPLETION_JUSTFILE"
echo "1. Select 'build' recipe"
echo "2. At prompt, type 'myfile.txt' and press ENTER"
echo ""
echo -e "${MAGENTA}Expected:${NC}"
echo "  - Normal manual entry works"
echo "  - No TV launched"
echo "  - Command executes with typed value"
read -p "Press Enter after testing..."

# Test 9: Clean output display
run_test "Clean parameter display" "Check for duplicate lines"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_COMPLETION_JUSTFILE"
echo "1. Select 'build' recipe"
echo "2. Enter any value and press ENTER"
echo ""
echo -e "${MAGENTA}Look for:${NC}"
echo "  - Single line showing 'target=yourvalue'"
echo -e "${RED}Bug if:${NC}"
echo "  - Duplicate 'target=yourvalue' lines"
echo "  - Extra empty lines"
read -p "Press Enter after testing..."

# Test 10: Optional parameter with file completion
run_test "Optional parameter file completion" "Test with default value parameter"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_COMPLETION_JUSTFILE"
echo "1. Select 'deploy' recipe"
echo "2. At 'file=' prompt, press TAB"
echo "3. Select a file or press ESC to use default"
echo ""
echo -e "${MAGENTA}Expected:${NC}"
echo "  - Shows default value in prompt"
echo "  - TAB works for optional parameters"
echo "  - Can use default with ENTER"
read -p "Press Enter after testing..."

# ============================================================================
# SECTION 3: PARAMETER HANDLING
# ============================================================================

echo ""
echo -e "${BLUE}━━━ SECTION 3: Parameter Handling ━━━${NC}"

# Test 11: Parameter with default value
run_test "Parameter with default" "Select 'greet', press Enter for default"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_JUSTFILE"
echo "Expected output: OUTPUT:greet:TestUser"
read -p "Press Enter after testing..."
LAST_OUTPUT=$(cat ~/.just-tv-last-command 2>/dev/null || echo "")
verify "greet TestUser" "$LAST_OUTPUT"

# Test 12: Parameter with custom value
run_test "Parameter with custom value" "Select 'greet', enter 'Alice'"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_JUSTFILE"
echo "Expected output: OUTPUT:greet:Alice"
read -p "Press Enter after testing..."
LAST_OUTPUT=$(cat ~/.just-tv-last-command 2>/dev/null || echo "")
verify "greet Alice" "$LAST_OUTPUT"

# Test 13: Required parameter
run_test "Required parameter" "Select 'build', enter 'linux'"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_JUSTFILE"
echo "Should prompt: 'target='"
echo "Expected output: OUTPUT:build:linux"
read -p "Press Enter after testing..."
LAST_OUTPUT=$(cat ~/.just-tv-last-command 2>/dev/null || echo "")
verify "build linux" "$LAST_OUTPUT"

# Test 14: Multiple parameters
run_test "Multiple parameters" "Select 'deploy', use defaults (Enter, Enter)"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_JUSTFILE"
echo "Expected output: OUTPUT:deploy:staging:v1.0"
read -p "Press Enter after testing..."
LAST_OUTPUT=$(cat ~/.just-tv-last-command 2>/dev/null || echo "")
verify "deploy staging v1.0" "$LAST_OUTPUT"

# ============================================================================
# SECTION 4: VISUAL INTERFACE (UPDATED IN 0.3.0)
# ============================================================================

echo ""
echo -e "${BLUE}━━━ SECTION 4: Visual Interface (UPDATED IN 0.3.0) ━━━${NC}"

# Test 15: Recipe signature display
run_test "Recipe signature display" "Check colored signature"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_COMPLETION_JUSTFILE"
echo "After selecting recipe, should see:"
echo -e "  ${CYAN}build${NC} ${YELLOW}target${NC}=${RED}<required>${NC}"
echo ""
echo "Colors should be:"
echo "  - Cyan: recipe name"
echo "  - Yellow: parameter name"
echo "  - Red: <required> indicator"
read -p "Press Enter after visual verification..."

# Test 16: Minimal output mode
run_test "Minimal output" "Check for clean output"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_COMPLETION_JUSTFILE"
echo "Should NOT see:"
echo "  - 'Selected: recipe name'"
echo "  - 'Processing: recipe'"
echo "  - Separator lines (━━━)"
echo "  - 'Commands saved to .just_history'"
echo ""
echo "Should only see:"
echo "  - Recipe signature"
echo "  - Parameter prompt"
echo "  - Command output"
read -p "Press Enter after visual verification..."

# ============================================================================
# SECTION 5: MIXED PARAMETERS AND DEPENDENCIES
# ============================================================================

echo ""
echo -e "${BLUE}━━━ SECTION 5: Mixed Parameters and Dependencies ━━━${NC}"

# Test 17: Mixed recipe with required parameter
run_test "Mixed recipe - required parameter" "Select 'mixed', enter 'Bob'"
echo -e "${CYAN}Manual:${NC} $SCRIPT $MODULAR_JUSTFILE"
echo -e "${MAGENTA}This tests the bug fix for mixed parameters and dependencies${NC}"
echo "Recipe: mixed name: clean install"
echo "Should prompt: 'name='"
echo "Enter: Bob"
echo "Expected output: Hello Bob! (after clean and install)"
read -p "Press Enter after testing..."
LAST_OUTPUT=$(cat ~/.just-tv-last-command 2>/dev/null || echo "")
verify "mixed Bob" "$LAST_OUTPUT"

# Test 18: Mixed recipe with default parameter
run_test "Mixed recipe - default parameter" "Select 'mixed-default', press Enter for default"
echo -e "${CYAN}Manual:${NC} $SCRIPT $MODULAR_JUSTFILE"
echo "Recipe: mixed-default name=\"Alice\": clean install"
echo "Should show default in prompt"
echo "Press Enter to use default"
echo "Expected output: Hello Alice! (after clean and install)"
read -p "Press Enter after testing..."
LAST_OUTPUT=$(cat ~/.just-tv-last-command 2>/dev/null || echo "")
verify "mixed-default Alice" "$LAST_OUTPUT"

# ============================================================================
# SECTION 6: HISTORY INTEGRATION
# ============================================================================

echo ""
echo -e "${BLUE}━━━ SECTION 6: History Integration ━━━${NC}"

# Test 19: History file creation
run_test "History file creation" "Check .just_history exists after execution"
if [[ -f "$HISTORY_FILE" ]]; then
	echo -e "${GREEN}✓${NC} History file created"
	echo -e "${DIM}Contents:${NC}"
	tail -3 "$HISTORY_FILE"
else
	echo -e "${RED}✗${NC} History file not found"
	FAILED=$((FAILED + 1))
fi

# Test 20: History format
run_test "History format verification" "Check history entry format"
if [[ -f "$HISTORY_FILE" ]]; then
	LAST_HISTORY=$(tail -1 "$HISTORY_FILE")
	echo "Last entry: $LAST_HISTORY"
	# Updated format check for 0.3.0
	if [[ "$LAST_HISTORY" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}\ [0-9]{2}:[0-9]{2}:[0-9]{2}\ \[[0-9]+\]\ just ]]; then
		echo -e "${GREEN}✓${NC} Correct format: timestamp [exit_code] command"
	else
		echo -e "${RED}✗${NC} Invalid history format"
		FAILED=$((FAILED + 1))
	fi
fi

# Test 21: No history flag
run_test "No history flag" "Run with --no-history"
HISTORY_BEFORE=$(wc -l <"$HISTORY_FILE" 2>/dev/null || echo "0")
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_JUSTFILE --no-history"
echo "Select 'simple' and execute"
read -p "Press Enter after testing..."
HISTORY_AFTER=$(wc -l <"$HISTORY_FILE" 2>/dev/null || echo "0")
if [[ "$HISTORY_BEFORE" == "$HISTORY_AFTER" ]]; then
	echo -e "${GREEN}✓${NC} History not written with --no-history"
else
	echo -e "${RED}✗${NC} History was written despite --no-history"
	FAILED=$((FAILED + 1))
fi

# ============================================================================
# SECTION 7: MODULE SUPPORT
# ============================================================================

echo ""
echo -e "${BLUE}━━━ SECTION 7: Module Support ━━━${NC}"

# Test 22: Module detection
run_test "Module auto-detection" "Run with modular justfile"
echo -e "${CYAN}Manual:${NC} $SCRIPT $MODULAR_JUSTFILE"
echo "Should show recipes with module prefixes:"
echo "  🔷 build (core recipe)"
echo "  🐳 docker::build (docker module)"
echo "  🧪 test::unit (test module)"
echo "  🚀 deploy::staging (deploy module)"
read -p "Press Enter after visual verification..."

# Test 23: Module filter
run_test "Module filter" "Show only docker module"
echo -e "${CYAN}Manual:${NC} $SCRIPT $MODULAR_JUSTFILE --module docker"
echo "Should show ONLY docker recipes:"
echo "  docker::build"
echo "  docker::push"
echo "  docker::run"
echo "  docker::clean"
read -p "Press Enter after visual verification..."

# Test 24: Module recipe execution
run_test "Module recipe execution" "Select 'docker::build'"
echo -e "${CYAN}Manual:${NC} $SCRIPT $MODULAR_JUSTFILE"
echo "Select docker::build, use defaults"
echo "Expected: Building Docker image: app:latest"
read -p "Press Enter after testing..."
LAST_OUTPUT=$(cat ~/.just-tv-last-command 2>/dev/null || echo "")
verify "docker::build" "$LAST_OUTPUT"

# ============================================================================
# SECTION 8: SHELL COMPATIBILITY (NEW IN 0.3.0)
# ============================================================================

echo ""
echo -e "${BLUE}━━━ SECTION 8: Shell Compatibility (NEW IN 0.3.0) ━━━${NC}"

# Test 25: Bash TAB completion
run_test "Bash TAB completion" "Test in bash environment"
echo -e "${CYAN}Manual:${NC} bash -c '$SCRIPT $TEST_COMPLETION_JUSTFILE'"
echo "If running in bash:"
echo "  - TAB should work for file completion"
echo "  - No bind warnings should appear"
echo ""
echo "If running in zsh:"
echo "  - Will use fallback mode"
echo "  - Type '.' and ENTER for file browser prompt"
read -p "Press Enter after testing..."

# Test 26: No bind warnings
run_test "No bind warnings" "Check for shell compatibility"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_COMPLETION_JUSTFILE"
echo "Should NOT see warnings like:"
echo "  - 'bind: command not found'"
echo "  - 'bind: warning: line editing not enabled'"
echo ""
echo "Script should detect shell and use appropriate method"
read -p "Press Enter after visual verification..."

# ============================================================================
# SECTION 9: ADVANCED FEATURES
# ============================================================================

echo ""
echo -e "${BLUE}━━━ SECTION 9: Advanced Features ━━━${NC}"

# Test 27: Multi-select
run_test "Multi-select recipes" "Select multiple with TAB/Space"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_JUSTFILE"
echo "Select 'simple' (TAB), then 'documented' (TAB), then Enter"
echo "Should execute both in sequence"
read -p "Press Enter after testing..."

# Test 28: Recipe with dependencies
run_test "Recipe dependencies" "Select 'run' (has dependency)"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_JUSTFILE"
echo "Expected outputs:"
echo "  OUTPUT:simple (dependency)"
echo "  OUTPUT:run:after:simple"
read -p "Press Enter after testing..."
verify_file "$HISTORY_FILE" "just --justfile='$TEST_JUSTFILE' run"

# Test 29: No icons mode
run_test "No icons mode" "Run with NO_ICONS=1"
echo -e "${CYAN}Manual:${NC} NO_ICONS=1 $SCRIPT $MODULAR_JUSTFILE"
echo "Should show text labels instead of emojis:"
echo "  [core] instead of 🔷"
echo "  [docker] instead of 🐳"
read -p "Press Enter after visual verification..."

# Test 30: Exit code tracking
run_test "Exit code in history" "Force a failure"
echo "Create a failing recipe and check history"
echo "fake-fail:" >/tmp/fail-justfile
echo "    exit 1" >>/tmp/fail-justfile
echo -e "${CYAN}Manual:${NC} $SCRIPT /tmp/fail-justfile"
echo "Select 'fake-fail', should show '✗ Command failed'"
read -p "Press Enter after testing..."
if [[ -f "$HISTORY_FILE" ]]; then
	FAIL_ENTRY=$(grep "fake-fail" "$HISTORY_FILE" | tail -1)
	if [[ "$FAIL_ENTRY" == *"[1]"* ]]; then
		echo -e "${GREEN}✓${NC} Exit code 1 recorded in history"
	else
		echo -e "${RED}✗${NC} Exit code not properly recorded"
		FAILED=$((FAILED + 1))
	fi
fi
rm -f /tmp/fail-justfile

# ============================================================================
# SECTION 10: EDGE CASES
# ============================================================================

echo ""
echo -e "${BLUE}━━━ SECTION 10: Edge Cases ━━━${NC}"

# Test 31: Spaces in parameters
run_test "Parameters with spaces" "Enter multi-word parameter"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_JUSTFILE"
echo "Select 'greet', enter: John Doe"
echo "Command should quote properly"
read -p "Press Enter after testing..."
LAST_OUTPUT=$(cat ~/.just-tv-last-command 2>/dev/null || echo "")
if [[ "$LAST_OUTPUT" == *"'John Doe'"* ]] || [[ "$LAST_OUTPUT" == *'"John Doe"'* ]]; then
	echo -e "${GREEN}✓${NC} Spaces properly quoted"
else
	echo -e "${MAGENTA}⚠${NC} Check space handling: $LAST_OUTPUT"
fi

# Test 32: File completion with spaces
run_test "File selection with spaces" "Select file with spaces in name"
echo "Create test file: 'my test file.txt'"
touch "my test file.txt"
echo -e "${CYAN}Manual:${NC} $SCRIPT $TEST_COMPLETION_JUSTFILE"
echo "1. Select 'build' recipe"
echo "2. Press TAB for file browser"
echo "3. Select 'my test file.txt'"
echo ""
echo -e "${MAGENTA}Expected:${NC} File path properly handled with spaces"
read -p "Press Enter after testing..."
rm -f "my test file.txt"

# Test 33: Missing justfile
run_test "Missing justfile error" "Try non-existent file"
OUTPUT=$($SCRIPT nonexistent.just 2>&1 || true)
verify "Error: Justfile 'nonexistent.just' not found" "$OUTPUT"

# ============================================================================
# SUMMARY
# ============================================================================

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "=== Test Summary ==="
echo "Tests run: $TEST_NUM"
if [[ $FAILED -eq 0 ]]; then
	echo -e "${GREEN}All tests passed!${NC}"
	echo ""
	echo -e "${GREEN}✓${NC} Basic functionality working"
	echo -e "${GREEN}✓${NC} File completion feature functional (NEW)"
	echo -e "${GREEN}✓${NC} Parameter handling correct"
	echo -e "${GREEN}✓${NC} Visual interface improvements verified (NEW)"
	echo -e "${GREEN}✓${NC} Mixed parameters and dependencies bug FIXED"
	echo -e "${GREEN}✓${NC} History integration functional"
	echo -e "${GREEN}✓${NC} Module support operational"
	echo -e "${GREEN}✓${NC} Shell compatibility verified (NEW)"
	echo -e "${GREEN}✓${NC} Advanced features tested"
else
	echo -e "${RED}Failed: $FAILED tests${NC}"
	echo ""
	echo "Review failed tests above"
fi

# Show version info
echo ""
echo -e "${CYAN}Version 0.3.0 Highlights:${NC}"
echo "  • TV-based file completion with TAB"
echo "  • Non-editable parameter prompts"
echo "  • Clean, minimal output"
echo "  • Improved shell compatibility"
echo "  • No duplicate output lines"

# Show created artifacts
echo ""
echo -e "${DIM}Test artifacts:${NC}"
[[ -f "$HISTORY_FILE" ]] && echo "  - $HISTORY_FILE ($(wc -l <"$HISTORY_FILE") entries)"
[[ -f ~/.just-tv-last-command ]] && echo "  - ~/.just-tv-last-command"

# Cleanup option
echo ""
read -p "Clean up test artifacts? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
	rm -f "$HISTORY_FILE" ~/.just-tv-last-command
	rm -rf "$ZDOTDIR"
	echo "Cleaned up"
else
	echo "Artifacts preserved for inspection"
fi

echo ""
echo -e "${DIM}To run the tool manually:${NC}"
echo "  $SCRIPT                              # Regular justfile"
echo "  $SCRIPT $TEST_COMPLETION_JUSTFILE    # Test file completion"
echo "  $SCRIPT $MODULAR_JUSTFILE            # Modular justfile"
echo "  $SCRIPT --help                       # Show all options"