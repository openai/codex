/*
 * AI Driver IOCTL Handlers
 * Handles DeviceIoControl requests from user-mode applications
 * 
 * Version: 0.3.0 - Best Practices Edition
 * FIXED: Proper buffer validation, memory tracking, error handling
 */

#include <ntddk.h>
#include <wdf.h>
#include <ntstrsafe.h>

#define AI_DRIVER_TAG 'iAcD'

// External function declarations with CORRECT signatures
extern NTSTATUS GetGpuStatus(PVOID OutputBuffer, SIZE_T OutputBufferLength);
extern NTSTATUS GetMemoryPoolStatus(PVOID OutputBuffer, SIZE_T OutputBufferLength);
extern NTSTATUS GetSchedulerStats(PVOID OutputBuffer, SIZE_T OutputBufferLength);
extern NTSTATUS AllocatePinnedMemory(ULONG64 Size, PULONG64 Address);
extern NTSTATUS FreePinnedMemory(ULONG64 Address);

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

/**
 * Handle: Get GPU Status
 * FIXED: Proper buffer validation
 */
_Use_decl_annotations_
NTSTATUS HandleGetGpuStatus(PIRP Irp)
{
    PIO_STACK_LOCATION stack;
    PVOID outputBuffer;
    ULONG outputLength;
    NTSTATUS status;
    
    if (!Irp) {
        return STATUS_INVALID_PARAMETER;
    }
    
    stack = IoGetCurrentIrpStackLocation(Irp);
    outputBuffer = Irp->AssociatedIrp.SystemBuffer;
    outputLength = stack->Parameters.DeviceIoControl.OutputBufferLength;
    
    /* Validate buffer size */
    if (outputLength < sizeof(GPU_STATUS)) {
        Irp->IoStatus.Status = STATUS_BUFFER_TOO_SMALL;
        Irp->IoStatus.Information = 0;
        return STATUS_BUFFER_TOO_SMALL;
    }
    
    if (!outputBuffer) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    /* Call GPU integration function */
    status = GetGpuStatus(outputBuffer, outputLength);
    
    if (NT_SUCCESS(status)) {
        Irp->IoStatus.Status = STATUS_SUCCESS;
        Irp->IoStatus.Information = sizeof(GPU_STATUS);
    } else {
        KdPrint(("AI Driver: GetGpuStatus failed: 0x%08X\n", status));
        Irp->IoStatus.Status = status;
        Irp->IoStatus.Information = 0;
    }
    
    return status;
}

/**
 * Handle: Get Memory Pool Status
 * FIXED: Proper buffer validation
 */
_Use_decl_annotations_
NTSTATUS HandleGetMemoryPool(PIRP Irp)
{
    PIO_STACK_LOCATION stack;
    PVOID outputBuffer;
    ULONG outputLength;
    NTSTATUS status;
    
    if (!Irp) {
        return STATUS_INVALID_PARAMETER;
    }
    
    stack = IoGetCurrentIrpStackLocation(Irp);
    outputBuffer = Irp->AssociatedIrp.SystemBuffer;
    outputLength = stack->Parameters.DeviceIoControl.OutputBufferLength;
    
    /* Validate buffer size */
    if (outputLength < sizeof(MEMORY_POOL_STATUS)) {
        Irp->IoStatus.Status = STATUS_BUFFER_TOO_SMALL;
        Irp->IoStatus.Information = 0;
        return STATUS_BUFFER_TOO_SMALL;
    }
    
    if (!outputBuffer) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    /* Call memory pool function */
    status = GetMemoryPoolStatus(outputBuffer, outputLength);
    
    if (NT_SUCCESS(status)) {
        Irp->IoStatus.Status = STATUS_SUCCESS;
        Irp->IoStatus.Information = sizeof(MEMORY_POOL_STATUS);
    } else {
        KdPrint(("AI Driver: GetMemoryPoolStatus failed: 0x%08X\n", status));
        Irp->IoStatus.Status = status;
        Irp->IoStatus.Information = 0;
    }
    
    return status;
}

/**
 * Handle: Get Scheduler Statistics
 * FIXED: Proper buffer validation
 */
_Use_decl_annotations_
NTSTATUS HandleGetSchedulerStats(PIRP Irp)
{
    PIO_STACK_LOCATION stack;
    PVOID outputBuffer;
    ULONG outputLength;
    NTSTATUS status;
    
    if (!Irp) {
        return STATUS_INVALID_PARAMETER;
    }
    
    stack = IoGetCurrentIrpStackLocation(Irp);
    outputBuffer = Irp->AssociatedIrp.SystemBuffer;
    outputLength = stack->Parameters.DeviceIoControl.OutputBufferLength;
    
    /* Validate buffer size */
    if (outputLength < sizeof(SCHEDULER_STATS)) {
        Irp->IoStatus.Status = STATUS_BUFFER_TOO_SMALL;
        Irp->IoStatus.Information = 0;
        return STATUS_BUFFER_TOO_SMALL;
    }
    
    if (!outputBuffer) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    /* Call scheduler stats function */
    status = GetSchedulerStats(outputBuffer, outputLength);
    
    if (NT_SUCCESS(status)) {
        Irp->IoStatus.Status = STATUS_SUCCESS;
        Irp->IoStatus.Information = sizeof(SCHEDULER_STATS);
    } else {
        KdPrint(("AI Driver: GetSchedulerStats failed: 0x%08X\n", status));
        Irp->IoStatus.Status = status;
        Irp->IoStatus.Information = 0;
    }
    
    return status;
}

/**
 * Handle: Allocate Pinned Memory
 * FIXED: Proper input/output validation
 */
_Use_decl_annotations_
NTSTATUS HandleAllocPinned(PIRP Irp)
{
    PIO_STACK_LOCATION stack;
    PVOID inputBuffer;
    PVOID outputBuffer;
    ULONG inputLength;
    ULONG outputLength;
    UINT64 requestedSize;
    UINT64 allocatedAddress;
    NTSTATUS status;
    
    if (!Irp) {
        return STATUS_INVALID_PARAMETER;
    }
    
    stack = IoGetCurrentIrpStackLocation(Irp);
    inputBuffer = Irp->AssociatedIrp.SystemBuffer;
    outputBuffer = Irp->AssociatedIrp.SystemBuffer;  // Same buffer for METHOD_BUFFERED
    inputLength = stack->Parameters.DeviceIoControl.InputBufferLength;
    outputLength = stack->Parameters.DeviceIoControl.OutputBufferLength;
    
    /* Validate buffer sizes */
    if (inputLength < sizeof(UINT64) || outputLength < sizeof(UINT64)) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    if (!inputBuffer || !outputBuffer) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    /* Get requested size */
    requestedSize = *(PUINT64)inputBuffer;
    
    /* Validate size */
    if (requestedSize == 0) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    /* Allocate pinned memory */
    status = AllocatePinnedMemory(requestedSize, &allocatedAddress);
    
    if (NT_SUCCESS(status)) {
        /* Return address */
        *(PUINT64)outputBuffer = allocatedAddress;
        
        Irp->IoStatus.Status = STATUS_SUCCESS;
        Irp->IoStatus.Information = sizeof(UINT64);
        
        KdPrint(("AI Driver: Allocated %llu bytes at 0x%llX\n", 
                 requestedSize, allocatedAddress));
    } else {
        Irp->IoStatus.Status = status;
        Irp->IoStatus.Information = 0;
        
        KdPrint(("AI Driver: Failed to allocate %llu bytes: 0x%08X\n",
                 requestedSize, status));
    }
    
    return status;
}

/**
 * Handle: Free Pinned Memory
 * FIXED: Proper validation
 */
_Use_decl_annotations_
NTSTATUS HandleFreePinned(PIRP Irp)
{
    PIO_STACK_LOCATION stack;
    PVOID inputBuffer;
    ULONG inputLength;
    UINT64 address;
    NTSTATUS status;
    
    if (!Irp) {
        return STATUS_INVALID_PARAMETER;
    }
    
    stack = IoGetCurrentIrpStackLocation(Irp);
    inputBuffer = Irp->AssociatedIrp.SystemBuffer;
    inputLength = stack->Parameters.DeviceIoControl.InputBufferLength;
    
    /* Validate buffer size */
    if (inputLength < sizeof(UINT64)) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    if (!inputBuffer) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    /* Get address to free */
    address = *(PUINT64)inputBuffer;
    
    if (address == 0) {
        Irp->IoStatus.Status = STATUS_INVALID_PARAMETER;
        Irp->IoStatus.Information = 0;
        return STATUS_INVALID_PARAMETER;
    }
    
    /* Free pinned memory */
    status = FreePinnedMemory(address);
    
    if (NT_SUCCESS(status)) {
        Irp->IoStatus.Status = STATUS_SUCCESS;
        Irp->IoStatus.Information = 0;
        
        KdPrint(("AI Driver: Freed memory at 0x%llX\n", address));
    } else {
        Irp->IoStatus.Status = status;
        Irp->IoStatus.Information = 0;
        
        KdPrint(("AI Driver: Failed to free memory at 0x%llX: 0x%08X\n",
                 address, status));
    }
    
    return status;
}
