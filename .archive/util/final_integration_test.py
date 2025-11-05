"""
Codex Deep Research 最終統合テスト
DuckDuckGo検索、URLデコード、バージョン整合性をすべて確認
"""
import subprocess
import json
import os
from tqdm import tqdm
import time

def run_command(cmd):
    """コマンドを実行して結果を返す"""
    print(f"\n[*] Running: {cmd}")
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, encoding='utf-8', errors='ignore')
    return result

def test_version():
    """バージョン確認"""
    print("\n" + "="*60)
    print("Test 1: Version Check")
    print("="*60)
    
    # VERSIONファイル
    with open('VERSION', 'r') as f:
        version_file = f.read().strip()
    print(f"[OK] VERSION file: {version_file}")
    
    # package.json
    with open('codex-cli/package.json', 'r') as f:
        pkg = json.load(f)
        pkg_version = pkg['version']
    print(f"[OK] package.json: {pkg_version}")
    
    # Codexバイナリバージョン
    result = run_command('codex --version')
    print(f"[OK] Codex CLI: {result.stdout.strip()}")
    
    # 整合性チェック
    if version_file == pkg_version:
        print("\n✅ [SUCCESS] Versions are consistent!")
        return True
    else:
        print(f"\n❌ [FAILED] Version mismatch: {version_file} != {pkg_version}")
        return False

def test_deep_research():
    """Deep Research機能テスト"""
    print("\n" + "="*60)
    print("Test 2: Deep Research with DuckDuckGo")
    print("="*60)
    
    queries = [
        "Rust async programming",
        "Python web framework",
        "JavaScript tutorial",
    ]
    
    results = []
    for query in tqdm(queries, desc="Testing queries"):
        cmd = f'.\\codex-cli\\vendor\\x86_64-pc-windows-msvc\\codex\\codex.exe research "{query}" --depth 1 --breadth 2'
        result = run_command(cmd)
        
        success = 'Sources found:' in result.stdout or result.stderr
        results.append({
            'query': query,
            'success': success,
            'has_urls': 'https://' in result.stdout or 'https://' in result.stderr
        })
        
        print(f"\n  Query: {query}")
        print(f"  Success: {'✅' if success else '❌'}")
        print(f"  Has URLs: {'✅' if results[-1]['has_urls'] else '❌'}")
        
        time.sleep(1)
    
    all_success = all(r['success'] for r in results)
    all_have_urls = all(r['has_urls'] for r in results)
    
    if all_success and all_have_urls:
        print("\n✅ [SUCCESS] All deep research tests passed!")
        return True
    else:
        print("\n❌ [FAILED] Some tests failed")
        return False

def test_url_decoder():
    """URLデコーダーテスト"""
    print("\n" + "="*60)
    print("Test 3: URL Decoder")
    print("="*60)
    
    result = run_command('cd codex-rs ; cargo test -p codex-deep-research url_decoder --lib')
    
    if 'test result: ok' in result.stdout:
        print("✅ [SUCCESS] URL decoder tests passed!")
        return True
    else:
        print("❌ [FAILED] URL decoder tests failed")
        print(result.stdout)
        return False

def test_command_availability():
    """コマンド利用可能性テスト"""
    print("\n" + "="*60)
    print("Test 4: Command Availability")
    print("="*60)
    
    # codexコマンドが利用可能か
    result = run_command('codex --help')
    if result.returncode == 0:
        print("✅ [OK] codex command available")
    else:
        print("❌ [FAILED] codex command not found")
        return False
    
    # researchサブコマンドが利用可能か
    if 'research' in result.stdout.lower():
        print("✅ [OK] research subcommand available")
        return True
    else:
        print("❌ [FAILED] research subcommand not found")
        return False

def generate_summary(results):
    """サマリー生成"""
    print("\n" + "="*60)
    print("FINAL SUMMARY")
    print("="*60)
    
    total = len(results)
    passed = sum(results.values())
    
    print(f"\nTotal Tests: {total}")
    print(f"Passed: {passed}")
    print(f"Failed: {total - passed}")
    print(f"Success Rate: {passed/total*100:.1f}%")
    
    print("\n" + "-"*60)
    for test_name, success in results.items():
        status = "✅ PASS" if success else "❌ FAIL"
        print(f"{status}: {test_name}")
    print("-"*60)
    
    if all(results.values()):
        print("\n" + "🎊"*20)
        print("✅ ALL TESTS PASSED!")
        print("🎊 Production Ready!")
        print("🎊"*20)
    else:
        print("\n❌ Some tests failed. Please review.")

def main():
    print("="*60)
    print("Codex Deep Research - Final Integration Test")
    print("="*60)
    print("Testing DuckDuckGo integration, URL decoding,")
    print("and version consistency")
    print("="*60)
    
    results = {}
    
    # Test 1: Version
    results['Version Consistency'] = test_version()
    time.sleep(1)
    
    # Test 2: Deep Research
    results['Deep Research Functionality'] = test_deep_research()
    time.sleep(1)
    
    # Test 3: URL Decoder
    results['URL Decoder'] = test_url_decoder()
    time.sleep(1)
    
    # Test 4: Command Availability
    results['Command Availability'] = test_command_availability()
    
    # Generate Summary
    generate_summary(results)
    
    # Save results
    with open('_docs/final_integration_test_results.json', 'w') as f:
        json.dump(results, f, indent=2)
    print("\n[OK] Results saved to: _docs/final_integration_test_results.json")

if __name__ == "__main__":
    main()

