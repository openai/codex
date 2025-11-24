#!/bin/sh -eu

for f in "$@"
do
	echo >&2 "Checking $f"
	wayland-scanner -s client-header "$f" /dev/null
	wayland-scanner -s server-header "$f" /dev/null
	wayland-scanner -s public-code "$f" /dev/null
	wayland-scanner -s private-code "$f" /dev/null
done
