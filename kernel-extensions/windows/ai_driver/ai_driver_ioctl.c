/*
 * AI Driver IOCTL Dispatcher
 * Routes IOCTL requests to appropriate handlers
 * 
 * Version: 0.3.0 - Best Practices Edition
 * FIXED: Proper error handling, consistent IOCTL codes
 */

#include <ntddk.h>
#include <wdf.h>
#include <ntstrsafe.h>

// IOCTL codes (using CTL_CODE macro for proper definition)
#define IOCTL_AI_GET_STATS          CTL_CODE(FILE_DEVICE_UNKNOWN, 0x800, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define IOCTL_AI_SET_GPU_UTIL       CTL_CODE(FILE_DEVICE_UNKNOWN, 0x801, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define IOCTL_AI_BOOST_PRIORITY     CTL_CODE(FILE_DEVICE_UNKNOWN, 0x802, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define IOCTL_AI_GET_GPU_STATUS     CTL_CODE(FILE_DEVICE_UNKNOWN, 0x803, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define IOCTL_AI_GET_MEMORY_POOL    CTL_CODE(FILE_DEVICE_UNKNOWN, 0x804, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define IOCTL_AI_GET_SCHEDULER_STATS CTL_CODE(FILE_DEVICE_UNKNOWN, 0x805, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define IOCTL_AI_ALLOC_PINNED       CTL_CODE(FILE_DEVICE_UNKNOWN, 0x806, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define IOCTL_AI_FREE_PINNED        CTL_CODE(FILE_DEVICE_UNKNOWN, 0x807, METHOD_BUFFERED, FILE_ANY_ACCESS)

// Forward declarations with CORRECT signatures
extern NTSTATUS HandleGetGpuStatus(PIRP Irp);
extern NTSTATUS HandleGetMemoryPool(PIRP Irp);
extern NTSTATUS HandleGetSchedulerStats(PIRP Irp);
extern NTSTATUS HandleAllocPinned(PIRP Irp);
extern NTSTATUS HandleFreePinned(PIRP Irp);

/**
 * IOCTL Device Control Handler
 * FIXED: Better error handling, logging, parameter validation
 */
_Use_decl_annotations_
VOID AiDriverEvtIoDeviceControl(
    WDFQUEUE Queue,
    WDFREQUEST Request,
    size_t OutputBufferLength,
    size_t InputBufferLength,
    ULONG IoControlCode
)
{
    NTSTATUS status = STATUS_INVALID_DEVICE_REQUEST;
    ULONG_PTR information = 0;
    WDFDEVICE device;
    PIRP irp;
    
    UNREFERENCED_PARAMETER(OutputBufferLength);
    UNREFERENCED_PARAMETER(InputBufferLength);
    
    device = WdfIoQueueGetDevice(Queue);
    if (!device) {
        KdPrint(("AI Driver: Invalid device in IOCTL handler\n"));
        WdfRequestComplete(Request, STATUS_INVALID_DEVICE_REQUEST);
        return;
    }
    
    irp = WdfRequestWdmGetIrp(Request);
    if (!irp) {
        KdPrint(("AI Driver: Failed to get IRP from request\n"));
        WdfRequestComplete(Request, STATUS_INVALID_PARAMETER);
        return;
    }
    
    KdPrint(("AI Driver: IOCTL request - Code: 0x%08X\n", IoControlCode));
    
    // Route to appropriate handler
    switch (IoControlCode) {
        case IOCTL_AI_GET_GPU_STATUS:
            KdPrint(("AI Driver: IOCTL_AI_GET_GPU_STATUS\n"));
            status = HandleGetGpuStatus(irp);
            information = irp->IoStatus.Information;
            break;
            
        case IOCTL_AI_GET_MEMORY_POOL:
            KdPrint(("AI Driver: IOCTL_AI_GET_MEMORY_POOL\n"));
            status = HandleGetMemoryPool(irp);
            information = irp->IoStatus.Information;
            break;
            
        case IOCTL_AI_GET_SCHEDULER_STATS:
            KdPrint(("AI Driver: IOCTL_AI_GET_SCHEDULER_STATS\n"));
            status = HandleGetSchedulerStats(irp);
            information = irp->IoStatus.Information;
            break;
            
        case IOCTL_AI_ALLOC_PINNED:
            KdPrint(("AI Driver: IOCTL_AI_ALLOC_PINNED\n"));
            status = HandleAllocPinned(irp);
            information = irp->IoStatus.Information;
            break;
            
        case IOCTL_AI_FREE_PINNED:
            KdPrint(("AI Driver: IOCTL_AI_FREE_PINNED\n"));
            status = HandleFreePinned(irp);
            information = irp->IoStatus.Information;
            break;
            
        case IOCTL_AI_GET_STATS:
            // Legacy stats handler (deprecated)
            KdPrint(("AI Driver: IOCTL_AI_GET_STATS (deprecated, use GET_GPU_STATUS)\n"));
            status = STATUS_NOT_IMPLEMENTED;
            information = 0;
            break;
            
        case IOCTL_AI_SET_GPU_UTIL:
            // Legacy GPU util setter (deprecated)
            KdPrint(("AI Driver: IOCTL_AI_SET_GPU_UTIL (deprecated)\n"));
            status = STATUS_NOT_IMPLEMENTED;
            information = 0;
            break;
            
        case IOCTL_AI_BOOST_PRIORITY:
            // Legacy priority boost (deprecated)
            KdPrint(("AI Driver: IOCTL_AI_BOOST_PRIORITY (deprecated)\n"));
            status = STATUS_NOT_IMPLEMENTED;
            information = 0;
            break;
            
        default:
            KdPrint(("AI Driver: Unknown IOCTL code: 0x%08X\n", IoControlCode));
            status = STATUS_INVALID_DEVICE_REQUEST;
            information = 0;
            break;
    }
    
    // Log result
    if (NT_SUCCESS(status)) {
        KdPrint(("AI Driver: IOCTL completed successfully (0x%08X), bytes: %llu\n", 
                 IoControlCode, (ULONG64)information));
    } else {
        KdPrint(("AI Driver: IOCTL failed (0x%08X) with status: 0x%08X\n",
                 IoControlCode, status));
    }
    
    // Complete request
    WdfRequestCompleteWithInformation(Request, status, information);
}
