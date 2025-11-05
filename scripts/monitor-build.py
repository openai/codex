#!/usr/bin/env python3
"""
Cargo Build Progress Monitor with tqdm-style visualization
カーゴビルド進捗モニター（tqdm風視覚化）
"""

import subprocess
import re
import time
from datetime import datetime, timedelta

def monitor_cargo_build():
    """Monitor cargo build output and display progress"""
    
    print("🚀 Codex 1.0.0 Release Build Monitor")
    print("=" * 60)
    
    start_time = datetime.now()
    total_crates = 0
    compiled_crates = 0
    current_crate = ""
    
    # Pattern: "Compiling crate-name v1.0.0 (...)"
    compile_pattern = re.compile(r'^\s+Compiling\s+(.+?)\s+v[\d.]+')
    finished_pattern = re.compile(r'^\s+Finished')
    
    try:
        # Start cargo build process
        process = subprocess.Popen(
            ['cargo', 'build', '--release', '-p', 'codex-cli'],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
            cwd=r'C:\Users\downl\Desktop\codex\codex-rs'
        )
        
        for line in iter(process.stdout.readline, ''):
            if not line:
                break
                
            line = line.strip()
            
            # Compiling phase
            match = compile_pattern.match(line)
            if match:
                current_crate = match.group(1)
                compiled_crates += 1
                
                # Progress bar
                elapsed = datetime.now() - start_time
                elapsed_str = str(elapsed).split('.')[0]
                
                # Simple progress indicator
                bar_width = 30
                filled = min(bar_width, compiled_crates % bar_width)
                bar = '█' * filled + '░' * (bar_width - filled)
                
                print(f"\r[{bar}] {compiled_crates:3d} crates | {elapsed_str} | {current_crate[:40]}", end='', flush=True)
            
            # Finished phase
            elif finished_pattern.match(line):
                total_time = datetime.now() - start_time
                print(f"\n\n✅ Build Completed!")
                print(f"📊 Statistics:")
                print(f"  - Total crates: {compiled_crates}")
                print(f"  - Build time: {total_time}")
                print(f"  - Average: {total_time.total_seconds() / max(compiled_crates, 1):.2f}s per crate")
                print(f"  - Binary: codex-rs/target/release/codex.exe")
                break
        
        process.wait()
        return process.returncode == 0
        
    except KeyboardInterrupt:
        print("\n\n⚠️  Build interrupted by user")
        process.terminate()
        return False
    except Exception as e:
        print(f"\n\n❌ Error: {e}")
        return False

if __name__ == "__main__":
    success = monitor_cargo_build()
    exit(0 if success else 1)

