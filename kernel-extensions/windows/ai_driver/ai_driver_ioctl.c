/*
 * AI Driver IOCTL Dispatcher
 * Routes IOCTL requests to appropriate handlers
 */

#include <ntddk.h>
#include <wdf.h>

// IOCTL codes
#define IOCTL_AI_GET_STATS          0x222004
#define IOCTL_AI_SET_GPU_UTIL       0x222008
#define IOCTL_AI_BOOST_PRIORITY     0x22200C
#define IOCTL_AI_GET_GPU_STATUS     0x222010
#define IOCTL_AI_GET_MEMORY_POOL    0x222014
#define IOCTL_AI_GET_SCHEDULER_STATS 0x222018
#define IOCTL_AI_ALLOC_PINNED       0x22201C
#define IOCTL_AI_FREE_PINNED        0x222020

// Forward declarations
extern NTSTATUS HandleGetGpuStatus(PIRP Irp);
extern NTSTATUS HandleGetMemoryPool(PIRP Irp);
extern NTSTATUS HandleGetSchedulerStats(PIRP Irp);
extern NTSTATUS HandleAllocPinned(PIRP Irp);
extern NTSTATUS HandleFreePinned(PIRP Irp);

/**
 * IOCTL Device Control Handler
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
    WDFDEVICE device = WdfIoQueueGetDevice(Queue);
    
    UNREFERENCED_PARAMETER(device);
    UNREFERENCED_PARAMETER(OutputBufferLength);
    UNREFERENCED_PARAMETER(InputBufferLength);
    
    KdPrint(("AI Driver: IOCTL request - Code: 0x%08X\n", IoControlCode));
    
    // Get IRP
    WDF_REQUEST_PARAMETERS params;
    WDF_REQUEST_PARAMETERS_INIT(&params);
    WdfRequestGetParameters(Request, &params);
    
    PIRP irp = WdfRequestWdmGetIrp(Request);
    
    // Route to appropriate handler
    switch (IoControlCode) {
        case IOCTL_AI_GET_GPU_STATUS:
            status = HandleGetGpuStatus(irp);
            information = irp->IoStatus.Information;
            break;
            
        case IOCTL_AI_GET_MEMORY_POOL:
            status = HandleGetMemoryPool(irp);
            information = irp->IoStatus.Information;
            break;
            
        case IOCTL_AI_GET_SCHEDULER_STATS:
            status = HandleGetSchedulerStats(irp);
            information = irp->IoStatus.Information;
            break;
            
        case IOCTL_AI_ALLOC_PINNED:
            status = HandleAllocPinned(irp);
            information = irp->IoStatus.Information;
            break;
            
        case IOCTL_AI_FREE_PINNED:
            status = HandleFreePinned(irp);
            information = irp->IoStatus.Information;
            break;
            
        case IOCTL_AI_GET_STATS:
            // Legacy stats handler
            status = STATUS_NOT_IMPLEMENTED;
            break;
            
        case IOCTL_AI_SET_GPU_UTIL:
            // Legacy GPU util setter
            status = STATUS_NOT_IMPLEMENTED;
            break;
            
        case IOCTL_AI_BOOST_PRIORITY:
            // Legacy priority boost
            status = STATUS_NOT_IMPLEMENTED;
            break;
            
        default:
            KdPrint(("AI Driver: Unknown IOCTL code: 0x%08X\n", IoControlCode));
            status = STATUS_INVALID_DEVICE_REQUEST;
            break;
    }
    
    // Complete request
    WdfRequestCompleteWithInformation(Request, status, information);
}

