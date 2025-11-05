obj-m += ai_gpu.o

KDIR := /lib/modules/$(shell uname -r)/build
PWD := $(shell pwd)

# CUDA include path
CUDA_PATH ?= /usr/local/cuda
EXTRA_CFLAGS += -I$(CUDA_PATH)/include

all:
	$(MAKE) -C $(KDIR) M=$(PWD) modules

clean:
	$(MAKE) -C $(KDIR) M=$(PWD) clean

install:
	sudo insmod ai_gpu.ko

uninstall:
	sudo rmmod ai_gpu

reload: uninstall install

test:
	cat /proc/ai_gpu

.PHONY: all clean install uninstall reload test

