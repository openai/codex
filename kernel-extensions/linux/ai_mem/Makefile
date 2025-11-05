obj-m += ai_mem.o

KDIR := /lib/modules/$(shell uname -r)/build
PWD := $(shell pwd)

all:
	$(MAKE) -C $(KDIR) M=$(PWD) modules

clean:
	$(MAKE) -C $(KDIR) M=$(PWD) clean

install:
	sudo insmod ai_mem.ko

uninstall:
	sudo rmmod ai_mem

reload: uninstall install

test:
	cat /proc/ai_memory

.PHONY: all clean install uninstall reload test

