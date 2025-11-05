#
# AI Driver Makefile for Windows WDK
#

# Driver name
TARGETNAME=ai_driver

# Driver type
TARGETTYPE=DRIVER

# Entry point
DRIVERTYPE=WDM

# Source files
SOURCES=ai_driver.c \
        ioctl_handlers.c \
        gpu_integration.c \
        nvapi_bridge.c \
        dx12_compute.c

# Include paths
INCLUDES=$(DDK_INC_PATH)

# Libraries
TARGETLIBS=$(DDK_LIB_PATH)\wdf01000.lib \
          $(SDK_LIB_PATH)\ntoskrnl.lib \
          $(SDK_LIB_PATH)\hal.lib

# Compiler flags
MSC_WARNING_LEVEL=/W4 /WX

# Build for AMD64
_NT_TARGET_VERSION=$(_NT_TARGET_VERSION_WIN10)

