/*
 * AI Filter Driver for Windows
 * WDM/KMDF Filter Driver
 * 
 * Features:
 * - GPU-aware thread scheduling
 * - AI task detection
 * - Non-paged memory pool
 * - DirectX/CUDA integration
 */

#include <ntddk.h>
#include <wdf.h>

#define AI_DRIVER_TAG 'iAcD'  // 'DcAi' reversed
#define AI_MEMORY_POOL_SIZE (256 * 1024 * 1024)  // 256MB

/* Driver globals */
typedef struct _AI_DRIVER_GLOBALS {
    WDFDRIVER Driver;
    PVOID MemoryPool;
    SIZE_T PoolSize;
    KSPIN_LOCK PoolLock;
    LONG AiTaskCount;
    LONG GpuUtilization;
} AI_DRIVER_GLOBALS, *PAI_DRIVER_GLOBALS;

static AI_DRIVER_GLOBALS g_Globals = { 0 };

/* Forward declarations */
DRIVER_INITIALIZE DriverEntry;
EVT_WDF_DRIVER_DEVICE_ADD AiDriverDeviceAdd;
EVT_WDF_OBJECT_CONTEXT_CLEANUP AiDriverCleanup;
EVT_WDF_IO_QUEUE_IO_DEVICE_CONTROL AiDriverEvtIoDeviceControl;

/* External function declarations from other modules */
extern VOID AiDriverEvtIoDeviceControl(WDFQUEUE Queue, WDFREQUEST Request, 
    size_t OutputBufferLength, size_t InputBufferLength, ULONG IoControlCode);
extern VOID InitializePinnedMemory(VOID);
extern VOID CleanupPinnedMemory(VOID);
extern NTSTATUS InitializeNvapi(VOID);
extern VOID CleanupNvapi(VOID);
extern NTSTATUS InitializeDx12(VOID);
extern VOID CleanupDx12(VOID);

/*
 * Check if process is AI-related
 */
_Use_decl_annotations_
BOOLEAN IsAiProcess(PEPROCESS Process)
{
    PUNICODE_STRING processName;
    
    if (!Process) {
        return FALSE;
    }
    
    processName = (PUNICODE_STRING)PsGetProcessImageFileName(Process);
    if (!processName) {
        return FALSE;
    }
    
    /* Check for AI-related process names */
    if (wcsstr(processName->Buffer, L"python") ||
        wcsstr(processName->Buffer, L"codex") ||
        wcsstr(processName->Buffer, L"ai") ||
        wcsstr(processName->Buffer, L"ml")) {
        return TRUE;
    }
    
    return FALSE;
}

/*
 * Boost thread priority for AI tasks
 */
_Use_decl_annotations_
NTSTATUS BoostAiThreadPriority(PETHREAD Thread)
{
    KPRIORITY newPriority = HIGH_PRIORITY;
    
    if (!Thread) {
        return STATUS_INVALID_PARAMETER;
    }
    
    /* Set high priority for AI inference threads */
    KeSetBasePriorityThread(Thread, newPriority);
    
    KdPrint(("AI Driver: Boosted thread priority to %d\n", newPriority));
    
    return STATUS_SUCCESS;
}

/*
 * Allocate non-paged memory for AI workloads
 */
_Use_decl_annotations_
PVOID AiAllocateNonPagedMemory(SIZE_T Size)
{
    PVOID buffer;
    
    if (Size == 0 || Size > AI_MEMORY_POOL_SIZE) {
        KdPrint(("AI Driver: Invalid allocation size: %zu\n", Size));
        return NULL;
    }
    
    buffer = ExAllocatePoolWithTag(
        NonPagedPool,
        Size,
        AI_DRIVER_TAG
    );
    
    if (buffer) {
        KdPrint(("AI Driver: Allocated %zu bytes of non-paged memory\n", Size));
    } else {
        KdPrint(("AI Driver: Failed to allocate memory\n"));
    }
    
    return buffer;
}

/*
 * Free non-paged memory
 */
_Use_decl_annotations_
VOID AiFreeNonPagedMemory(PVOID Buffer)
{
    if (Buffer) {
        ExFreePoolWithTag(Buffer, AI_DRIVER_TAG);
        KdPrint(("AI Driver: Freed memory\n"));
    }
}

/*
 * Device add callback
 */
_Use_decl_annotations_
NTSTATUS AiDriverDeviceAdd(
    WDFDRIVER Driver,
    PWDFDEVICE_INIT DeviceInit
)
{
    NTSTATUS status;
    WDFDEVICE device;
    WDF_OBJECT_ATTRIBUTES attributes;
    WDF_IO_QUEUE_CONFIG queueConfig;
    WDFQUEUE queue;
    
    UNREFERENCED_PARAMETER(Driver);
    
    WDF_OBJECT_ATTRIBUTES_INIT(&attributes);
    attributes.EvtCleanupCallback = AiDriverCleanup;
    
    status = WdfDeviceCreate(&DeviceInit, &attributes, &device);
    if (!NT_SUCCESS(status)) {
        KdPrint(("AI Driver: WdfDeviceCreate failed: 0x%08X\n", status));
        return status;
    }
    
    /* Create I/O queue for IOCTL requests */
    WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE(&queueConfig, WdfIoQueueDispatchSequential);
    queueConfig.EvtIoDeviceControl = AiDriverEvtIoDeviceControl;
    
    status = WdfIoQueueCreate(device, &queueConfig, WDF_NO_OBJECT_ATTRIBUTES, &queue);
    if (!NT_SUCCESS(status)) {
        KdPrint(("AI Driver: WdfIoQueueCreate failed: 0x%08X\n", status));
        return status;
    }
    
    KdPrint(("AI Driver: Device and queue added successfully\n"));
    
    return STATUS_SUCCESS;
}

/*
 * Cleanup callback
 */
_Use_decl_annotations_
VOID AiDriverCleanup(WDFOBJECT Object)
{
    UNREFERENCED_PARAMETER(Object);
    
    /* Cleanup all subsystems */
    CleanupPinnedMemory();
    CleanupNvapi();
    CleanupDx12();
    
    /* Free memory pool */
    if (g_Globals.MemoryPool) {
        AiFreeNonPagedMemory(g_Globals.MemoryPool);
        g_Globals.MemoryPool = NULL;
    }
    
    KdPrint(("AI Driver: Cleanup completed\n"));
}

/*
 * Driver Entry Point
 */
_Use_decl_annotations_
NTSTATUS DriverEntry(
    PDRIVER_OBJECT DriverObject,
    PUNICODE_STRING RegistryPath
)
{
    NTSTATUS status;
    WDF_DRIVER_CONFIG config;
    
    KdPrint(("AI Driver: Initializing...\n"));
    
    /* Initialize globals */
    RtlZeroMemory(&g_Globals, sizeof(AI_DRIVER_GLOBALS));
    KeInitializeSpinLock(&g_Globals.PoolLock);
    g_Globals.AiTaskCount = 0;
    g_Globals.GpuUtilization = 0;
    
    /* Initialize subsystems */
    InitializePinnedMemory();
    InitializeNvapi();
    InitializeDx12();
    
    /* Allocate memory pool */
    g_Globals.MemoryPool = AiAllocateNonPagedMemory(AI_MEMORY_POOL_SIZE);
    if (!g_Globals.MemoryPool) {
        KdPrint(("AI Driver: Failed to allocate memory pool\n"));
        /* Continue without pool - not fatal */
    } else {
        g_Globals.PoolSize = AI_MEMORY_POOL_SIZE;
        KdPrint(("AI Driver: Memory pool allocated: %zu MB\n",
                 AI_MEMORY_POOL_SIZE / 1024 / 1024));
    }
    
    /* Initialize WDF */
    WDF_DRIVER_CONFIG_INIT(&config, AiDriverDeviceAdd);
    
    status = WdfDriverCreate(
        DriverObject,
        RegistryPath,
        WDF_NO_OBJECT_ATTRIBUTES,
        &config,
        &g_Globals.Driver
    );
    
    if (!NT_SUCCESS(status)) {
        KdPrint(("AI Driver: WdfDriverCreate failed: 0x%08X\n", status));
        if (g_Globals.MemoryPool) {
            AiFreeNonPagedMemory(g_Globals.MemoryPool);
        }
        return status;
    }
    
    KdPrint(("AI Driver: Initialized successfully\n"));
    
    return STATUS_SUCCESS;
}

