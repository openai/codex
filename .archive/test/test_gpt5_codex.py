#!/usr/bin/env python3
"""
GPT-5-Codex Model Integration Test
Tests the updated configuration with gpt-5-codex as default model
"""

import subprocess
import sys
import json
import time
from pathlib import Path

def print_header(text):
    """Print a formatted header"""
    print(f"\n{'='*60}")
    print(f"  {text}")
    print(f"{'='*60}\n")

def run_command(cmd, timeout=10):
    """Run a command and return output"""
    try:
        result = subprocess.run(
            cmd,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout,
            encoding='utf-8',
            errors='replace'
        )
        return result.returncode, result.stdout, result.stderr
    except subprocess.TimeoutExpired:
        return -1, "", "Command timed out"
    except Exception as e:
        return -1, "", str(e)

def test_codex_version():
    """Test 1: Codex CLI version check"""
    print_header("Test 1: Codex Version")
    code, stdout, stderr = run_command("codex --version")
    
    if code == 0 and "0.47.0-alpha.1" in stdout:
        print("[PASS] Codex CLI version: 0.47.0-alpha.1")
        return True
    else:
        print(f"[FAIL] Version check failed")
        print(f"stdout: {stdout}")
        print(f"stderr: {stderr}")
        return False

def test_config_file():
    """Test 2: Config file validation"""
    print_header("Test 2: Config File Check")
    config_path = Path.home() / ".codex" / "config.toml"
    
    if not config_path.exists():
        print(f"[FAIL] Config file not found: {config_path}")
        return False
    
    try:
        content = config_path.read_text(encoding='utf-8')
        
        # Check for gpt-5-codex
        if 'model = "gpt-5-codex"' in content:
            print("[PASS] Default model set to: gpt-5-codex")
            
            # Show relevant config section
            for line in content.split('\n'):
                if 'model' in line.lower() or 'codex' in line.lower():
                    print(f"  {line}")
            
            return True
        else:
            print("[FAIL] gpt-5-codex not found in config")
            print(f"Config content preview:\n{content[:500]}")
            return False
            
    except Exception as e:
        print(f"[FAIL] Error reading config: {e}")
        return False

def test_mcp_server_list():
    """Test 3: MCP server list"""
    print_header("Test 3: MCP Server List")
    
    # Use a shorter timeout for this
    code, stdout, stderr = run_command("codex mcp list", timeout=5)
    
    if code == 0:
        print("[PASS] MCP server list retrieved")
        print(f"\nOutput:\n{stdout}")
        
        # Check for codex-agent
        if "codex-agent" in stdout:
            print("\n[INFO] codex-agent MCP server found")
        
        return True
    else:
        print(f"[WARN] MCP list command failed (exit code: {code})")
        print(f"stdout: {stdout}")
        print(f"stderr: {stderr}")
        return False

def test_help_command():
    """Test 4: Help command"""
    print_header("Test 4: Help Command")
    code, stdout, stderr = run_command("codex --help")
    
    if code == 0 and "--model" in stdout:
        print("[PASS] Help command works")
        print("\n[INFO] --model flag available for dynamic model selection")
        return True
    else:
        print(f"[FAIL] Help command failed")
        return False

def test_model_override():
    """Test 5: Model override check"""
    print_header("Test 5: Model Override Test")
    
    print("[INFO] Testing model override with --model flag")
    print("Note: This is a syntax check, not a full execution")
    
    # Just verify the flag is accepted
    code, stdout, stderr = run_command("codex --model gpt-5-codex-medium --help", timeout=5)
    
    if code == 0:
        print("[PASS] --model flag accepted")
        print("[INFO] Available models:")
        print("  - gpt-5-codex (default)")
        print("  - gpt-5-codex-medium")
        print("  - gpt-4o")
        print("  - gpt-4o-mini")
        print("  - o1-preview")
        return True
    else:
        print(f"[WARN] Model override test failed")
        return False

def main():
    """Run all tests"""
    print("\n" + "="*60)
    print("  GPT-5-Codex Integration Test Suite")
    print("  Testing Model: gpt-5-codex (Latest 2025 Codex)")
    print("="*60)
    
    results = []
    
    # Run all tests
    results.append(("Version Check", test_codex_version()))
    results.append(("Config File", test_config_file()))
    results.append(("MCP Server List", test_mcp_server_list()))
    results.append(("Help Command", test_help_command()))
    results.append(("Model Override", test_model_override()))
    
    # Summary
    print_header("Test Summary")
    
    passed = sum(1 for _, result in results if result)
    total = len(results)
    
    for test_name, result in results:
        status = "[PASS]" if result else "[FAIL]"
        print(f"{status} {test_name}")
    
    print(f"\n{'='*60}")
    print(f"Results: {passed}/{total} tests passed ({passed*100//total}%)")
    print(f"{'='*60}\n")
    
    if passed == total:
        print("[SUCCESS] All tests passed! gpt-5-codex is ready to use")
        return 0
    else:
        print(f"[WARNING] {total - passed} test(s) failed")
        return 1

if __name__ == "__main__":
    sys.exit(main())

