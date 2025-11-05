#!/usr/bin/env python3
"""
AI Kernel Monitor
Real-time monitoring of AI kernel extensions
"""

import os
import sys
import time
from datetime import datetime

try:
    from bcc import BPF
except ImportError:
    print("❌ BCC not installed. Install with: sudo apt install bpftrace bcc")
    sys.exit(1)

# eBPFプログラム
bpf_program = """
#include <linux/sched.h>

// GPU利用率マップ
BPF_HASH(gpu_util, u32, u64);

// 推論レイテンシヒストグラム
BPF_HISTOGRAM(inference_latency);

// 推論開始時刻マップ
BPF_HASH(inference_start, u64, u64);

// CUDA kernel起動トレース
int trace_cuda_launch(struct pt_regs *ctx) {
    u64 pid_tgid = bpf_get_current_pid_tgid();
    u64 ts = bpf_ktime_get_ns();
    
    inference_start.update(&pid_tgid, &ts);
    
    return 0;
}

// CUDA kernel完了トレース
int trace_cuda_complete(struct pt_regs *ctx) {
    u64 pid_tgid = bpf_get_current_pid_tgid();
    u64 *start_ts = inference_start.lookup(&pid_tgid);
    
    if (start_ts) {
        u64 delta = bpf_ktime_get_ns() - *start_ts;
        
        // msに変換してヒストグラム記録
        u64 latency_ms = delta / 1000000;
        inference_latency.increment(bpf_log2l(latency_ms));
        
        inference_start.delete(&pid_tgid);
    }
    
    return 0;
}
"""

def print_header():
    print("\n" + "="*80)
    print("🚀 Codex AI-Native OS Kernel Monitor")
    print("="*80)
    print(f"⏰ Started at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("="*80 + "\n")

def print_gpu_stats(b):
    print("📊 GPU Statistics:")
    print("-" * 60)
    
    gpu_util = b.get_table("gpu_util")
    
    if len(gpu_util) == 0:
        print("  No GPU data available")
    else:
        for k, v in gpu_util.items():
            print(f"  GPU {k.value}: Utilization {v.value}%")
    
    print()

def print_inference_latency(b):
    print("⚡ Inference Latency Distribution:")
    print("-" * 60)
    
    b["inference_latency"].print_log2_hist("Latency (ms)")
    print()

def print_memory_stats():
    print("💾 AI Memory Pool Status:")
    print("-" * 60)
    
    try:
        with open('/proc/ai_memory', 'r') as f:
            print(f.read())
    except FileNotFoundError:
        print("  AI Memory module not loaded")
    
    print()

def print_scheduler_stats():
    print("🔄 AI Scheduler Status:")
    print("-" * 60)
    
    try:
        with open('/proc/ai_scheduler', 'r') as f:
            print(f.read())
    except FileNotFoundError:
        print("  AI Scheduler module not loaded")
    
    print()

def main():
    print_header()
    
    # eBPFプログラムロード
    try:
        b = BPF(text=bpf_program)
        
        # CUDAフック（存在する場合）
        try:
            b.attach_kprobe(event="cuLaunchKernel", fn_name="trace_cuda_launch")
            b.attach_kretprobe(event="cuLaunchKernel", fn_name="trace_cuda_complete")
            print("✅ CUDA tracing enabled")
        except:
            print("⚠️  CUDA not available (no NVIDIA GPU?)")
        
    except Exception as e:
        print(f"❌ Failed to load eBPF: {e}")
        b = None
    
    print("\n📈 Monitoring (Press Ctrl+C to stop)...\n")
    
    try:
        while True:
            print(f"\n⏰ {datetime.now().strftime('%H:%M:%S')}")
            print("=" * 60)
            
            if b:
                print_gpu_stats(b)
                print_inference_latency(b)
            
            print_memory_stats()
            print_scheduler_stats()
            
            time.sleep(2)
            
    except KeyboardInterrupt:
        print("\n\n👋 Monitoring stopped")
        sys.exit(0)

if __name__ == "__main__":
    if os.geteuid() != 0:
        print("❌ This script requires root privileges")
        print("   Run with: sudo python3 ai_monitor.py")
        sys.exit(1)
    
    main()
