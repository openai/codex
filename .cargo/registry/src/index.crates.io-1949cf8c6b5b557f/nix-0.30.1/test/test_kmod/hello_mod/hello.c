/*
 *  SPDX-License-Identifier: GPL-2.0+ or MIT
 */
#include <linux/module.h>
#include <linux/kernel.h>

static int number= 1;
static char *who = "World";

module_param(number, int, S_IRUSR | S_IWUSR | S_IRGRP | S_IROTH);
MODULE_PARM_DESC(myint, "Just some number");
module_param(who, charp, 0000);
MODULE_PARM_DESC(who, "Whot to greet");

int init_module(void)
{
	printk(KERN_INFO "Hello %s (%d)!\n", who, number);
	return 0;
}

void cleanup_module(void)
{
	printk(KERN_INFO "Goodbye %s (%d)!\n", who, number);
}

MODULE_LICENSE("Dual MIT/GPL");
