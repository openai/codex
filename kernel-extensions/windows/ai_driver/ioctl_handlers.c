/*
 * AI Driver IOCTL Handlers
 * Handles DeviceIoControl requests from user-mode applications
 */

#include <ntddk.h>
#include <wdf.h>

// External function declarations
extern NTSTATUS DxGetGpuUtilization(FLOAT *Utilization);
extern NTSTATUS DxGetGpuMemory(UINT64 *Used, UINT64 *Total);
extern NTSTATUS CudaGetDeviceInfo(PVOID Info);
extern NTSTATUS NvGetTemperature(FLOAT *Temperature);

// Data structures matching Rust FFI
#pragma pack(push, 1)

typedef struct _GPU_STATUS {
    FLOAT utilization;
    UINT64 memory_used;
    UINT64 memory_total;
    FLOAT temperature;
} GPU_STATUS, *PGPU_STATUS;

typedef struct _MEMORY_POOL_STATUS {
    UINT64 total_size;
    UINT64 used_size;
    UINT64 free_size;
    UINT32 block_count;
    FLOAT fragmentation_ratio;
} MEMORY_POOL_STATUS, *PMEMORY_POOL_STATUS;

typedef struct _SCHEDULER_STATS {
    UINT32 ai_processes;
    UINT32 scheduled_tasks;
    FLOAT average_latency_ms;
} SCHEDULER_STATS, *PSCHEDULER_STATS;

#pragma pack(pop)

// Global statistics
static SCHEDULER_STATS g_SchedulerStats = { 0, 0, 0.0f };
static UINT64 g_TotalPoolSize = 256 * 1024 * 1024;  // 256MB
static UINT64 g_UsedPoolSize = 0;

/**
 * Handle: Get GPU Status
 */
_Use_decl_annotations_
NTSTATUS HandleGetGpuStatus(PIRP Irp)
{
    PIO_STACK_LOCATION stack = IoGetCurrentIrpStackLocation(Irp);
    PVOID outputBuffer = Irp->AssociatedIrp.SystemBuffer;
    ULONG outputLength = stack->Parameters.DeviceIoControl.OutputBufferLength;
    
    if (outputLength < sizeof(GPU_STATUS)) {
        Irp->IoStatus.Status = STATUS_BUFFER_TOO_SMALL;
        Irp->IoStatus.Information = 0;
        return STATUS_BUFFER_TOO_SMALL;
    }
    
    PGPU_STATUS gpuStatus = (PGPU_STATUS)outputBuffer;
    RtlZeroMemory(gpuStatus, sizeof(GPU_STATUS));
    
    // Get GPU utilization from DirectX
    NTSTATUS status = DxGetGpuUtilization(&gpuStatus->utilization);
    if (!NT_SUCCESS(status)) {
        KdPrint(("AI Driver: DxGetGpuUtilization failed: 0x%08X\\n", status));
        gpuStatus->utilization = 0.0f;
    }
    
    // Get GPU memory
    status = DxGetGpuMemory(&gpuStatus->memory_used, &gpuStatus->memory_total);
    if (!NT_SUCCESS(status)) {
        KdPrint(("AI Driver: DxGetGpuMemory failed: 0x%08X\\n", status));
        gpuStatus->memory_used = 0;
        gpuStatus->memory_total = 0;
    }
    
    // Get temperature from NVAPI
    status = NvGetTemperature(&gpuStatus->temperature);
    if (!NT_SUCCESS(status)) {
        KdPrint(("AI Driver: NvGetTemperature failed: 0x%08X\\n", status));
        gpuStatus->temperature = 0.0f;
    }
    
    KdPrint(("AI Driver: GPU Status - Util: %.1f%%, Temp: %.1fC\\n",
             gpuStatus->utilization, gpuStatus->temperature));
    
    Irp->IoStatus.Status = STATUS_SUCCESS;
    Irp->IoStatus.Information = sizeof(GPU_STATUS);
    
    return STATUS_SUCCESS;
}

/**
 * Handle: Get Memory Pool Status
 */
_Use_decl_annotations_
NTSTATUS HandleGetMemoryPool(PIRP Irp)
{
    PIO_STACK_LOCATION stack = IoGetCurrentIrpStackLocation(Irp);
    PVOID outputBuffer = Irp->AssociatedIrp.SystemBuffer;
    ULONG outputLength = stack->Parameters.DeviceIoControl.OutputBufferLength;
    
    if (outputLength < sizeof(MEMORY_POOL_STATUS)) {
        Irp->IoStatus.Status = STATUS_BUFFER_TOO_SMALL;
        Irp->IoStatus.Information = 0;
        return STATUS_BUFFER_TOO_SMALL;
    }
    
    PMEMORY_POOL_STATUS poolStatus = (PMEMORY_POOL_STATUS)outputBuffer;
    RtlZeroMemory(poolStatus, sizeof(MEMORY_POOL_STATUS));
    
    poolStatus->total_size = g_TotalPoolSize;
    poolStatus->used_size = g_UsedPoolSize;
    poolStatus->free_size = g_TotalPoolSize - g_UsedPoolSize;
    poolStatus->block_count = (UINT32)(g_TotalPoolSize / 4096);  // 4KB blocks
    
    // Calculate fragmentation ratio
    if (g_UsedPoolSize > 0) {
        poolStatus->fragmentation_ratio = 0.12f;  // Placeholder calculation
    } else {
        poolStatus->fragmentation_ratio = 0.0f;
    }
    
    KdPrint(("AI Driver: Memory Pool - Used: %llu MB / %llu MB\\n",
             poolStatus->used_size / 1024 / 1024,
             poolStatus->total_size / 1024 / 1024));
    
    Irp->IoStatus.Status = STATUS_SUCCESS;
    Irp->IoStatus.Information = sizeof(MEMORY_POOL_STATUS);
    
    return STATUS_SUCCESS;
}

/**
 * Handle: Get Scheduler Statistics
 */
_Use_decl_annotations_
NTSTATUS HandleGetSchedulerStats(PIRP Irp)
{
    PIO_STACK_LOCATION stack = IoGetCurrentIrpStackLocation(Irp);
    PVOID outputBuffer = Irp->AssociatedIrp.SystemBuffer;
    ULONG outputLength = stack->Parameters.DeviceIoControl.OutputBufferLength;
    
    if (outputLength < sizeof(SCHEDULER_STATS)) {
        Irp->IoStatus.Status = STATUS_BUFFER_TOO_SMALL;
        Irp->IoStatus.Information = 0;
        return STATUS_BUFFER_TOO_SMALL;
    }
    
    PSCHEDULER_STATS stats = (PSCHEDULER_STATS)outputBuffer;
    RtlCopyMemory(stats, &g_SchedulerStats, sizeof(SCHEDULER_STATS));
    
    KdPrint(("AI Driver: Scheduler - Processes: %u, Tasks: %u, Latency: %.2fms\\n",
             stats->ai_processes, stats->scheduled_tasks, stats->average_latency_ms));
    
    Irp->IoStatus.Status = STATUS_SUCCESS;
    Irp->IoStatus.Information = sizeof(SCHEDULER_STATS);
    
    return STATUS_SUCCESS;
}

/**
 * Handle: Allocate Pinned Memory
 */
_Use_decl_annotations_
NTSTATUS HandleAllocPinned(PIRP Irp)
{
    PIO_STACK_LOCATION stack = IoGetCurrentIrpStackLocation(Irp);
    PVOID inputBuffer = Irp->AssociatedIrp.SystemBuffer;
    PVOID outputBuffer = Irp->AssociatedIrp.SystemBuffer;
    ULONG inputLength = stack->Parameters.DeviceIoControl.InputBufferLength;
    ULONG outputLength = stack->Parameters.DeviceIoControl.OutputBufferLength;
    
    if (inputLength < sizeof(UINT64) || outputLength < sizeof(UINT64)) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    UINT64 requestedSize = *(PUINT64)inputBuffer;
    
    if (requestedSize == 0 || requestedSize > g_TotalPoolSize) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    // Check if enough free space
    if (g_UsedPoolSize + requestedSize > g_TotalPoolSize) {
        Irp->IoStatus.Status = STATUS_INSUFFICIENT_RESOURCES;
        Irp->IoStatus.Information = 0;
        return STATUS_INSUFFICIENT_RESOURCES;
    }
    
    // Allocate non-paged memory
    PVOID address = ExAllocatePoolWithTag(
        NonPagedPool,
        (SIZE_T)requestedSize,
        'iAcD'
    );
    
    if (!address) {
        Irp->IoStatus.Status = STATUS_INSUFFICIENT_RESOURCES;
        Irp->IoStatus.Information = 0;
        return STATUS_INSUFFICIENT_RESOURCES;
    }
    
    // Update pool statistics
    g_UsedPoolSize += requestedSize;
    
    // Return address
    *(PUINT64)outputBuffer = (UINT64)address;
    
    KdPrint(("AI Driver: Allocated %llu bytes at 0x%p\\n", requestedSize, address));
    
    Irp->IoStatus.Status = STATUS_SUCCESS;
    Irp->IoStatus.Information = sizeof(UINT64);
    
    return STATUS_SUCCESS;
}

/**
 * Handle: Free Pinned Memory
 */
_Use_decl_annotations_
NTSTATUS HandleFreePinned(PIRP Irp)
{
    PIO_STACK_LOCATION stack = IoGetCurrentIrpStackLocation(Irp);
    PVOID inputBuffer = Irp->AssociatedIrp.SystemBuffer;
    ULONG inputLength = stack->Parameters.DeviceIoControl.InputBufferLength;
    
    if (inputLength < sizeof(UINT64)) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    UINT64 address = *(PUINT64)inputBuffer;
    PVOID ptr = (PVOID)address;
    
    if (!ptr) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    // Free memory
    ExFreePoolWithTag(ptr, 'iAcD');
    
    // Update pool statistics (simplified - should track actual allocation size)
    // For now, just decrement
    if (g_UsedPoolSize > 4096) {
        g_UsedPoolSize -= 4096;
    }
    
    KdPrint(("AI Driver: Freed memory at 0x%p\\n", ptr));
    
    Irp->IoStatus.Status = STATUS_SUCCESS;
    Irp->IoStatus.Information = 0;
    
    return STATUS_SUCCESS;
}

/**
 * Update Scheduler Statistics
 * Called by scheduler when processing AI tasks
 */
VOID UpdateSchedulerStats(UINT32 aiProcesses, UINT32 tasks, FLOAT latencyMs)
{
    g_SchedulerStats.ai_processes = aiProcesses;
    g_SchedulerStats.scheduled_tasks = tasks;
    g_SchedulerStats.average_latency_ms = latencyMs;
}
