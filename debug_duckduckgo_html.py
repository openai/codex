"""
DuckDuckGo HTML構造デバッグスクリプト
実際のHTMLを取得して構造を分析
"""
import requests
from bs4 import BeautifulSoup
import re
from urllib.parse import quote
from tqdm import tqdm
import time

def fetch_duckduckgo_html(query):
    """DuckDuckGoからHTMLを取得"""
    url = f"https://html.duckduckgo.com/html/?q={quote(query)}"
    print(f"\n[*] Fetching URL: {url}")
    
    headers = {
        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
    }
    
    response = requests.get(url, headers=headers, timeout=30)
    print(f"[OK] Status Code: {response.status_code}")
    print(f"[OK] Content Length: {len(response.text)} bytes")
    
    return response.text

def analyze_html_structure(html):
    """HTML構造を分析"""
    print("\n" + "="*60)
    print("HTML Structure Analysis")
    print("="*60)
    
    soup = BeautifulSoup(html, 'html.parser')
    
    # 1. result__aクラスを持つaタグを検索（Rustで使用中のパターン）
    print("\n[1] Searching for 'result__a' class links:")
    result_links = soup.find_all('a', class_='result__a')
    print(f"   Found: {len(result_links)} links")
    
    for i, link in enumerate(result_links[:5]):
        print(f"\n   Result #{i+1}:")
        print(f"      Title: {link.get_text(strip=True)}")
        print(f"      URL: {link.get('href', 'N/A')}")
        print(f"      Classes: {link.get('class', [])}")
    
    # 2. 他の可能性のあるクラス名を検索
    print("\n[2] Searching for other result patterns:")
    
    patterns = [
        ('result', 'class_'),
        ('search-result', 'class_'),
        ('web-result', 'class_'),
        ('result-link', 'class_'),
    ]
    
    for pattern, attr in patterns:
        found = soup.find_all(attrs={attr: re.compile(pattern, re.I)})
        if found:
            print(f"   Found {len(found)} elements matching '{pattern}'")
            if found:
                print(f"      Example: {found[0].name} with class={found[0].get('class', [])}")
    
    # 3. すべての<a>タグを確認（トップ10）
    print("\n[3] All <a> tags (first 10):")
    all_links = soup.find_all('a', href=True)
    print(f"   Total <a> tags: {len(all_links)}")
    
    for i, link in enumerate(all_links[:10]):
        href = link.get('href', '')
        if href and not href.startswith('#') and not href.startswith('javascript:'):
            print(f"   {i+1}. href={href[:80]}... class={link.get('class', [])}")
    
    # 4. 正規表現パターンテスト（Rustコードで使用中）
    print("\n[4] Testing Rust regex pattern:")
    rust_pattern = r'<a[^>]*class="result__a"[^>]*href="([^"]*)"[^>]*>([^<]*)</a>'
    matches = re.findall(rust_pattern, html)
    print(f"   Regex matches: {len(matches)}")
    
    for i, (url, title) in enumerate(matches[:5]):
        print(f"\n   Match #{i+1}:")
        print(f"      Title: {title}")
        print(f"      URL: {url[:100]}...")
    
    return soup

def save_html_sample(html, filename="_debug_duckduckgo_sample.html"):
    """HTMLをファイルに保存"""
    with open(filename, 'w', encoding='utf-8') as f:
        f.write(html)
    print(f"\n[OK] HTML saved to: {filename}")

def test_multiple_queries():
    """複数のクエリでテスト"""
    queries = [
        "Rust async programming",
        "Python web framework",
        "JavaScript tutorial",
    ]
    
    print("\n" + "="*60)
    print("Testing Multiple Queries")
    print("="*60)
    
    for query in tqdm(queries, desc="Testing queries"):
        print(f"\n\n{'='*60}")
        print(f"Query: {query}")
        print('='*60)
        
        html = fetch_duckduckgo_html(query)
        analyze_html_structure(html)
        
        # 最初のクエリだけHTMLを保存
        if query == queries[0]:
            save_html_sample(html)
        
        time.sleep(2)  # レート制限回避

def main():
    print("="*60)
    print("DuckDuckGo HTML Parse Debug Tool")
    print("="*60)
    
    # シンプルなクエリでテスト
    query = "Rust async"
    print(f"\n[*] Testing query: '{query}'")
    
    html = fetch_duckduckgo_html(query)
    soup = analyze_html_structure(html)
    save_html_sample(html)
    
    # 複数クエリテスト
    print("\n\n[*] Running multiple query test...")
    choice = input("\nTest multiple queries? (y/n): ").lower()
    if choice == 'y':
        test_multiple_queries()
    
    print("\n" + "="*60)
    print("[SUCCESS] Debug completed!")
    print("="*60)
    print("\n[*] Next steps:")
    print("  1. Check _debug_duckduckgo_sample.html")
    print("  2. Update Rust regex pattern if needed")
    print("  3. Rebuild and test")

if __name__ == "__main__":
    main()

