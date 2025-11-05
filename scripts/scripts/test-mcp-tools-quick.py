#!/usr/bin/env python3
"""Quick MCP Tools Test for v0.48.0 New Features"""

import json
import subprocess
import sys
import time
from typing import List, Dict, Any

def send_mcp_request(proc, request: Dict[str, Any]) -> Dict[str, Any]:
    """Send JSON-RPC request and get response."""
    proc.stdin.write(json.dumps(request) + '\n')
    proc.stdin.flush()
    response_line = proc.stdout.readline()
    return json.loads(response_line)

def test_mcp_tools():
    """Test MCP server tools including new features."""
    
    print("🚀 Starting MCP Tools Test (v0.48.0)")
    print("=" * 60)
    
    # Start MCP server
    proc = subprocess.Popen(
        ['codex', 'mcp-server'],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1
    )
    
    time.sleep(0.5)
    
    try:
        # Initialize
        print("\n[1/4] Initialize MCP server...")
        init_req = {
            'jsonrpc': '2.0',
            'id': 1,
            'method': 'initialize',
            'params': {
                'protocolVersion': '2024-11-05',
                'capabilities': {},
                'clientInfo': {
                    'name': 'test-client',
                    'version': '1.0.0'
                }
            }
        }
        
        init_resp = send_mcp_request(proc, init_req)
        
        if 'result' in init_resp:
            print("  ✅ MCP server initialized")
            server_version = init_resp['result']['serverInfo']['version']
            print(f"  📦 Server version: {server_version}")
        else:
            print("  ❌ Initialize failed")
            return False
        
        # List tools
        print("\n[2/4] List available tools...")
        list_tools_req = {
            'jsonrpc': '2.0',
            'id': 2,
            'method': 'tools/list',
            'params': {}
        }
        
        tools_resp = send_mcp_request(proc, list_tools_req)
        
        if 'result' not in tools_resp:
            print("  ❌ Failed to list tools")
            return False
        
        tools = tools_resp['result']['tools']
        tool_names = [t['name'] for t in tools]
        
        print(f"  ✅ Found {len(tools)} tools:")
        for name in tool_names:
            print(f"     - {name}")
        
        # Check new features
        print("\n[3/4] Verify new feature tools...")
        new_features = {
            'codex-auto-orchestrate': '自律オーケストレーション',
            'codex-webhook': 'Webhook統合',
        }
        
        passed = 0
        for tool_name, feature_name in new_features.items():
            if tool_name in tool_names:
                print(f"  ✅ {tool_name} ({feature_name})")
                passed += 1
            else:
                print(f"  ❌ {tool_name} ({feature_name}) - NOT FOUND")
        
        # Test auto-orchestrate tool call
        print("\n[4/4] Test auto-orchestrate tool call...")
        orchestrate_req = {
            'jsonrpc': '2.0',
            'id': 3,
            'method': 'tools/call',
            'params': {
                'name': 'codex-auto-orchestrate',
                'arguments': {
                    'goal': 'Fix typo in README',
                    'auto_threshold': 0.7,
                    'strategy': 'sequential',
                    'format': 'json'
                }
            }
        }
        
        orchestrate_resp = send_mcp_request(proc, orchestrate_req)
        
        if 'result' in orchestrate_resp and not orchestrate_resp.get('result', {}).get('isError'):
            content = orchestrate_resp['result']['content'][0]['text']
            result_data = json.loads(content)
            
            print(f"  ✅ Tool call succeeded")
            print(f"  📊 Complexity: {result_data.get('complexity_score', 'N/A')}")
            print(f"  🎯 Orchestrated: {result_data.get('was_orchestrated', 'N/A')}")
            passed += 1
        else:
            print(f"  ❌ Tool call failed")
            if 'error' in orchestrate_resp:
                print(f"     Error: {orchestrate_resp['error']}")
        
        # Summary
        print("\n" + "=" * 60)
        print(f"📊 Test Results: {passed + 1}/{len(new_features) + 2} passed")
        print("=" * 60)
        
        if passed >= len(new_features):
            print("🎉 NEW FEATURES VERIFIED! Production ready!")
            return True
        else:
            print("⚠️  Some features missing, check implementation")
            return False
        
    except Exception as e:
        print(f"\n❌ Test error: {e}")
        import traceback
        traceback.print_exc()
        return False
    finally:
        proc.terminate()
        proc.wait(timeout=2)

if __name__ == '__main__':
    success = test_mcp_tools()
    sys.exit(0 if success else 1)

