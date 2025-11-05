# AI Scheduler Kernel Module Makefile

obj-m += ai_scheduler.o

KDIR := /lib/modules/$(shell uname -r)/build
PWD := $(shell pwd)

all:
	$(MAKE) -C $(KDIR) M=$(PWD) modules

clean:
	$(MAKE) -C $(KDIR) M=$(PWD) clean

install:
	sudo insmod ai_scheduler.ko

uninstall:
	sudo rmmod ai_scheduler

reload: uninstall install

test:
	sudo dmesg | tail -20

.PHONY: all clean install uninstall reload test

