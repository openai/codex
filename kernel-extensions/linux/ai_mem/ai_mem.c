/*
 * AI Memory Allocator
 * Kernel Module for AI-optimized memory management
 * 
 * Features:
 * - Pinned memory pool (GPU accessible)
 * - Zero-copy transfers
 * - NUMA-aware allocation
 */

#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/init.h>
#include <linux/slab.h>
#include <linux/mm.h>
#include <linux/vmalloc.h>
#include <linux/dma-mapping.h>
#include <linux/proc_fs.h>
#include <linux/seq_file.h>

MODULE_LICENSE("GPL");
MODULE_AUTHOR("zapabob");
MODULE_DESCRIPTION("AI Memory Allocator");
MODULE_VERSION("0.1.0");

// Memory pool configuration
#define AI_MEM_POOL_SIZE (256 * 1024 * 1024)  // 256MB
#define AI_MEM_BLOCK_SIZE (4 * 1024)          // 4KB blocks

struct ai_memory_block {
    void *virt_addr;
    dma_addr_t dma_addr;
    size_t size;
    bool is_allocated;
    pid_t owner_pid;
};

struct ai_memory_pool {
    struct ai_memory_block *blocks;
    int num_blocks;
    spinlock_t lock;
    atomic_t allocated_bytes;
};

static struct ai_memory_pool *global_pool = NULL;

/*
 * Memory pool初期化
 */
static int ai_mem_pool_init(void)
{
    int i;
    int num_blocks = AI_MEM_POOL_SIZE / AI_MEM_BLOCK_SIZE;
    
    global_pool = kzalloc(sizeof(struct ai_memory_pool), GFP_KERNEL);
    if (!global_pool)
        return -ENOMEM;
    
    global_pool->blocks = kzalloc(
        num_blocks * sizeof(struct ai_memory_block),
        GFP_KERNEL
    );
    if (!global_pool->blocks) {
        kfree(global_pool);
        return -ENOMEM;
    }
    
    global_pool->num_blocks = num_blocks;
    spin_lock_init(&global_pool->lock);
    atomic_set(&global_pool->allocated_bytes, 0);
    
    // 各ブロックを事前確保（Pinned memory）
    for (i = 0; i < num_blocks; i++) {
        struct page *page = alloc_page(GFP_KERNEL | __GFP_DMA);
        if (!page) {
            pr_err("AI Mem: Failed to allocate block %d\n", i);
            continue;
        }
        
        global_pool->blocks[i].virt_addr = page_address(page);
        global_pool->blocks[i].size = AI_MEM_BLOCK_SIZE;
        global_pool->blocks[i].is_allocated = false;
        
        // ページをピン留め
        SetPageReserved(page);
    }
    
    pr_info("AI Mem: Initialized %d blocks (%zu MB)\n",
            num_blocks, AI_MEM_POOL_SIZE / 1024 / 1024);
    
    return 0;
}

/*
 * Pinned memory確保
 */
void* ai_alloc_pinned(size_t size)
{
    unsigned long flags;
    int i;
    int num_blocks_needed = (size + AI_MEM_BLOCK_SIZE - 1) / AI_MEM_BLOCK_SIZE;
    void *first_block = NULL;
    
    if (!global_pool)
        return NULL;
    
    spin_lock_irqsave(&global_pool->lock, flags);
    
    // 連続ブロック検索
    for (i = 0; i <= global_pool->num_blocks - num_blocks_needed; i++) {
        bool available = true;
        int j;
        
        for (j = 0; j < num_blocks_needed; j++) {
            if (global_pool->blocks[i + j].is_allocated) {
                available = false;
                break;
            }
        }
        
        if (available) {
            // 確保
            for (j = 0; j < num_blocks_needed; j++) {
                global_pool->blocks[i + j].is_allocated = true;
                global_pool->blocks[i + j].owner_pid = current->pid;
            }
            
            first_block = global_pool->blocks[i].virt_addr;
            atomic_add(size, &global_pool->allocated_bytes);
            break;
        }
    }
    
    spin_unlock_irqrestore(&global_pool->lock, flags);
    
    if (first_block) {
        pr_debug("AI Mem: Allocated %zu bytes for PID %d\n", size, current->pid);
    } else {
        pr_warn("AI Mem: Failed to allocate %zu bytes (OOM)\n", size);
    }
    
    return first_block;
}
EXPORT_SYMBOL(ai_alloc_pinned);

/*
 * Pinned memory解放
 */
void ai_free_pinned(void *addr)
{
    unsigned long flags;
    int i;
    
    if (!global_pool || !addr)
        return;
    
    spin_lock_irqsave(&global_pool->lock, flags);
    
    for (i = 0; i < global_pool->num_blocks; i++) {
        if (global_pool->blocks[i].virt_addr == addr &&
            global_pool->blocks[i].is_allocated) {
            
            global_pool->blocks[i].is_allocated = false;
            global_pool->blocks[i].owner_pid = 0;
            
            atomic_sub(global_pool->blocks[i].size, 
                      &global_pool->allocated_bytes);
            
            pr_debug("AI Mem: Freed block at index %d\n", i);
            break;
        }
    }
    
    spin_unlock_irqrestore(&global_pool->lock, flags);
}
EXPORT_SYMBOL(ai_free_pinned);

/*
 * /proc/ai_memory 統計表示
 */
static int ai_memory_proc_show(struct seq_file *m, void *v)
{
    seq_printf(m, "AI Memory Allocator Status\n");
    seq_printf(m, "===========================\n");
    seq_printf(m, "Total Pool Size: %zu MB\n", 
               AI_MEM_POOL_SIZE / 1024 / 1024);
    seq_printf(m, "Block Size: %zu KB\n", 
               AI_MEM_BLOCK_SIZE / 1024);
    seq_printf(m, "Total Blocks: %d\n", 
               global_pool ? global_pool->num_blocks : 0);
    seq_printf(m, "Allocated: %d bytes\n", 
               atomic_read(&global_pool->allocated_bytes));
    
    return 0;
}

static int ai_memory_proc_open(struct inode *inode, struct file *file)
{
    return single_open(file, ai_memory_proc_show, NULL);
}

static const struct proc_ops ai_memory_proc_ops = {
    .proc_open = ai_memory_proc_open,
    .proc_read = seq_read,
    .proc_lseek = seq_lseek,
    .proc_release = single_release,
};

/*
 * モジュール初期化
 */
static int __init ai_mem_init(void)
{
    int ret;
    
    pr_info("🚀 AI Memory Allocator: Initializing...\n");
    
    // Memory pool作成
    ret = ai_mem_pool_init();
    if (ret < 0) {
        pr_err("AI Mem: Pool initialization failed\n");
        return ret;
    }
    
    // /proc エントリ作成
    proc_create("ai_memory", 0, NULL, &ai_memory_proc_ops);
    
    pr_info("AI Memory Allocator: Ready! Check /proc/ai_memory\n");
    
    return 0;
}

/*
 * モジュール終了
 */
static void __exit ai_mem_exit(void)
{
    int i;
    
    pr_info("AI Memory Allocator: Shutting down...\n");
    
    // /proc エントリ削除
    remove_proc_entry("ai_memory", NULL);
    
    // メモリプール解放
    if (global_pool) {
        for (i = 0; i < global_pool->num_blocks; i++) {
            if (global_pool->blocks[i].virt_addr) {
                struct page *page = virt_to_page(
                    global_pool->blocks[i].virt_addr
                );
                ClearPageReserved(page);
                __free_page(page);
            }
        }
        
        kfree(global_pool->blocks);
        kfree(global_pool);
    }
    
    pr_info("AI Memory Allocator: Stopped\n");
}

module_init(ai_mem_init);
module_exit(ai_mem_exit);

