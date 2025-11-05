#!/usr/bin/env python3
"""
Codex MCP Real-World Test Suite
Tests actual MCP server functionality and identifies issues
"""

import json
import subprocess
import sys
import time
from datetime import datetime

class Colors:
    CYAN = '\033[96m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    RED = '\033[91m'
    MAGENTA = '\033[95m'
    RESET = '\033[0m'

def print_header(text):
    print(f"\n{Colors.CYAN}=== {text} ==={Colors.RESET}")

def print_success(text):
    print(f"{Colors.GREEN}[OK] {text}{Colors.RESET}")

def print_warning(text):
    print(f"{Colors.YELLOW}[WARN] {text}{Colors.RESET}")

def print_error(text):
    print(f"{Colors.RED}[ERROR] {text}{Colors.RESET}")

def print_info(text):
    print(f"{Colors.MAGENTA}   {text}{Colors.RESET}")

# Test results tracking
test_results = []

def run_test(test_name, test_func):
    """Run a test and track results"""
    print_header(test_name)
    try:
        result = test_func()
        if result:
            print_success(f"{test_name}: PASS")
            test_results.append((test_name, "PASS", None))
        else:
            print_warning(f"{test_name}: FAIL")
            test_results.append((test_name, "FAIL", "Test returned False"))
    except Exception as e:
        print_error(f"{test_name}: ERROR")
        print_info(f"Error: {str(e)}")
        test_results.append((test_name, "ERROR", str(e)))

# ===== Test 1: Codex CLI Version =====
def test_codex_version():
    """Check if Codex CLI is installed and accessible"""
    try:
        result = subprocess.run(
            ["codex", "--version"],
            capture_output=True,
            text=True,
            timeout=5
        )
        version = result.stdout.strip()
        print_info(f"Version: {version}")
        
        if "0.47.0-alpha.1" in version:
            print_success("Correct version detected")
            return True
        else:
            print_warning(f"Unexpected version: {version}")
            return False
    except FileNotFoundError:
        print_error("Codex CLI not found in PATH")
        return False
    except Exception as e:
        print_error(f"Error: {e}")
        return False

# ===== Test 2: MCP Server List =====
def test_mcp_server_list():
    """Check if MCP servers are registered"""
    try:
        result = subprocess.run(
            ["codex", "mcp", "list"],
            capture_output=True,
            text=True,
            timeout=10
        )
        output = result.stdout
        
        required_servers = ["codex-agent", "playwright", "web-search"]
        found_servers = []
        
        for server in required_servers:
            if server in output:
                found_servers.append(server)
                print_success(f"Found: {server}")
            else:
                print_error(f"Missing: {server}")
        
        return len(found_servers) == len(required_servers)
    
    except Exception as e:
        print_error(f"Error: {e}")
        return False

# ===== Test 3: Config File Validation =====
def test_config_file():
    """Validate config.toml syntax and content"""
    try:
        import os
        config_path = os.path.expanduser("~/.codex/config.toml")
        
        if not os.path.exists(config_path):
            print_error(f"Config file not found: {config_path}")
            return False
        
        with open(config_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # Check for required sections
        checks = [
            ('model = "gpt-5-codex', "Model configuration"),
            ('[mcp_servers.codex-agent]', "codex-agent MCP server"),
            ('command = "codex"', "Codex command"),
        ]
        
        all_passed = True
        for pattern, description in checks:
            if pattern in content:
                print_success(f"{description}: Found")
            else:
                print_error(f"{description}: NOT FOUND")
                all_passed = False
        
        return all_passed
    
    except Exception as e:
        print_error(f"Error: {e}")
        return False

# ===== Test 4: MCP Server Startup =====
def test_mcp_server_startup():
    """Test if MCP server can start (brief check)"""
    try:
        print_info("Starting MCP server for 3 seconds...")
        
        proc = subprocess.Popen(
            ["codex", "mcp-server"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        
        time.sleep(3)
        
        # Check if still running
        if proc.poll() is None:
            print_success("MCP server started successfully")
            proc.terminate()
            proc.wait(timeout=5)
            return True
        else:
            print_error("MCP server exited early")
            stderr = proc.stderr.read()
            if stderr:
                print_info(f"Error output: {stderr[:200]}")
            return False
    
    except Exception as e:
        print_error(f"Error: {e}")
        try:
            proc.terminate()
        except:
            pass
        return False

# ===== Test 5: NPM Warnings Check =====
def test_npm_configuration():
    """Check for npm configuration warnings"""
    try:
        result = subprocess.run(
            ["npx", "--version"],
            capture_output=True,
            text=True,
            timeout=5
        )
        
        warnings = []
        if "Unknown project config" in result.stderr:
            for line in result.stderr.split('\n'):
                if "Unknown project config" in line:
                    warnings.append(line.strip())
        
        if warnings:
            print_warning(f"Found {len(warnings)} npm configuration warnings:")
            for w in warnings[:5]:
                print_info(w)
            print_warning("These are from .npmrc using pnpm-specific config")
            return True  # Warning but not critical
        else:
            print_success("No npm warnings")
            return True
    
    except Exception as e:
        print_error(f"Error: {e}")
        return False

# ===== Test 6: Model Configuration =====
def test_model_configuration():
    """Verify model is correctly configured"""
    try:
        import os
        config_path = os.path.expanduser("~/.codex/config.toml")
        
        with open(config_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        if 'model = "gpt-5-codex' in content:
            print_success("Model: gpt-5-codex-medium (correct)")
            return True
        elif 'model = "gpt-4o"' in content:
            print_error("Model: gpt-4o (should be gpt-5-codex)")
            return False
        else:
            print_warning("Model configuration unclear")
            return False
    
    except Exception as e:
        print_error(f"Error: {e}")
        return False

# ===== Main Execution =====
if __name__ == "__main__":
    print_header("Codex MCP Real-World Test Suite")
    print_info(f"Date: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print_info(f"Codex Version: 0.47.0-alpha.1")
    print("")
    
    # Run all tests
    run_test("Test 1: Codex CLI Version", test_codex_version)
    run_test("Test 2: MCP Server List", test_mcp_server_list)
    run_test("Test 3: Config File Validation", test_config_file)
    run_test("Test 4: MCP Server Startup", test_mcp_server_startup)
    run_test("Test 5: NPM Configuration", test_npm_configuration)
    run_test("Test 6: Model Configuration", test_model_configuration)
    
    # Summary
    print_header("Test Results Summary")
    
    passed = sum(1 for _, status, _ in test_results if status == "PASS")
    failed = sum(1 for _, status, _ in test_results if status == "FAIL")
    errors = sum(1 for _, status, _ in test_results if status == "ERROR")
    total = len(test_results)
    
    print(f"\n{Colors.GREEN}Passed: {passed}/{total}{Colors.RESET}")
    print(f"{Colors.YELLOW}Failed: {failed}/{total}{Colors.RESET}")
    print(f"{Colors.RED}Errors: {errors}/{total}{Colors.RESET}")
    
    if failed > 0 or errors > 0:
        print_header("Issues Found")
        for name, status, error in test_results:
            if status in ["FAIL", "ERROR"]:
                print(f"{Colors.RED}  [X] {name}: {error if error else 'See details above'}{Colors.RESET}")
    
    # Calculate score
    score = (passed / total) * 100
    print(f"\n{Colors.MAGENTA}Overall Score: {score:.1f}%{Colors.RESET}")
    
    if score == 100:
        print(f"{Colors.GREEN}[SUCCESS] All tests passed!{Colors.RESET}")
    elif score >= 80:
        print(f"{Colors.YELLOW}[PARTIAL] Most tests passed, but some issues need attention{Colors.RESET}")
    else:
        print(f"{Colors.RED}[FAILURE] Multiple issues found, please review{Colors.RESET}")
    
    sys.exit(0 if score >= 80 else 1)

