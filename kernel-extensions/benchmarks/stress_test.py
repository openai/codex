#!/usr/bin/env python3
"""
AI Kernel Module Stress Test
Runs continuous load for 24 hours
"""

import subprocess
import time
import sys
import os
from datetime import datetime, timedelta
from collections import defaultdict

class StressTest:
    def __init__(self, duration_hours=24):
        self.duration = duration_hours * 3600  # Convert to seconds
        self.start_time = time.time()
        self.stats = defaultdict(int)
        
    def log(self, message):
        timestamp = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
        print(f"[{timestamp}] {message}")
        
    def check_modules_loaded(self):
        """Check if AI kernel modules are loaded"""
        result = subprocess.run(
            ['lsmod'],
            capture_output=True,
            text=True
        )
        
        modules = ['ai_scheduler', 'ai_mem', 'ai_gpu']
        loaded = []
        
        for module in modules:
            if module in result.stdout:
                loaded.append(module)
        
        return loaded
    
    def run_inference_batch(self):
        """Simulate AI inference batch"""
        # This would call actual AI models
        # For now, simulate with sleep
        time.sleep(0.01)  # 10ms inference
        self.stats['inferences'] += 1
    
    def check_memory_usage(self):
        """Check /proc/ai_memory status"""
        try:
            with open('/proc/ai_memory', 'r') as f:
                content = f.read()
                # Parse allocated bytes
                for line in content.split('\n'):
                    if 'Allocated:' in line:
                        parts = line.split()
                        if len(parts) >= 2:
                            allocated = int(parts[1])
                            self.stats['peak_memory'] = max(
                                self.stats['peak_memory'],
                                allocated
                            )
        except FileNotFoundError:
            pass
    
    def check_gpu_stats(self):
        """Check /proc/ai_gpu status"""
        try:
            with open('/proc/ai_gpu', 'r') as f:
                content = f.read()
                self.stats['gpu_checks'] += 1
        except FileNotFoundError:
            pass
    
    def run(self):
        """Run stress test"""
        self.log(f"🚀 Starting {self.duration / 3600:.1f}h stress test")
        
        # Check modules
        loaded = self.check_modules_loaded()
        if not loaded:
            self.log("❌ No AI kernel modules loaded")
            self.log("   Load with: sudo insmod ai_scheduler.ko")
            return 1
        
        self.log(f"✅ Loaded modules: {', '.join(loaded)}")
        
        iteration = 0
        last_report = time.time()
        
        try:
            while (time.time() - self.start_time) < self.duration:
                iteration += 1
                
                # Run inference batch
                self.run_inference_batch()
                
                # Check memory every 100 iterations
                if iteration % 100 == 0:
                    self.check_memory_usage()
                    self.check_gpu_stats()
                
                # Report every 60 seconds
                if time.time() - last_report >= 60:
                    elapsed = time.time() - self.start_time
                    remaining = self.duration - elapsed
                    
                    self.log(f"📊 Progress: {elapsed / 3600:.2f}h / {self.duration / 3600:.1f}h")
                    self.log(f"   Inferences: {self.stats['inferences']}")
                    self.log(f"   Peak Memory: {self.stats['peak_memory'] / 1024 / 1024:.1f} MB")
                    self.log(f"   Remaining: {remaining / 3600:.2f}h")
                    
                    last_report = time.time()
                
                # Small sleep to prevent 100% CPU
                time.sleep(0.001)
                
        except KeyboardInterrupt:
            self.log("\n⚠️  Test interrupted by user")
        
        # Final report
        total_time = time.time() - self.start_time
        self.log(f"\n🎉 Stress test completed!")
        self.log(f"   Duration: {total_time / 3600:.2f} hours")
        self.log(f"   Total inferences: {self.stats['inferences']}")
        self.log(f"   Throughput: {self.stats['inferences'] / total_time:.1f} inf/s")
        self.log(f"   Peak memory: {self.stats['peak_memory'] / 1024 / 1024:.1f} MB")
        
        return 0

if __name__ == "__main__":
    if os.geteuid() != 0:
        print("❌ This script requires root privileges")
        print("   Run with: sudo python3 stress_test.py")
        sys.exit(1)
    
    # Parse duration
    duration_hours = 24
    if len(sys.argv) > 1:
        try:
            duration_hours = float(sys.argv[1])
        except ValueError:
            print(f"Invalid duration: {sys.argv[1]}")
            sys.exit(1)
    
    test = StressTest(duration_hours)
    sys.exit(test.run())

