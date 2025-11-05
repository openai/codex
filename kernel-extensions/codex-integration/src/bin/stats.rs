//! Kernel statistics display utility

use codex_ai_kernel_integration::KernelModuleStats;

fn main() {
    println!("🚀 Codex AI-Native OS Kernel Statistics\n");
    
    match KernelModuleStats::read() {
        Ok(stats) => {
            stats.print();
            
            if !stats.is_available() {
                eprintln!("\n💡 Hint: Load kernel modules with:");
                eprintln!("   sudo insmod /path/to/ai_scheduler.ko");
                eprintln!("   sudo insmod /path/to/ai_mem.ko");
                eprintln!("   sudo insmod /path/to/ai_gpu.ko");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to read kernel stats: {}", e);
            eprintln!("\n💡 Hint: Run with sudo or load kernel modules");
            std::process::exit(1);
        }
    }
}

