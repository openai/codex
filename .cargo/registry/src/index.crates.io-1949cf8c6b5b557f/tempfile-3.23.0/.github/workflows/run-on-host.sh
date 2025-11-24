#!/system/bin/sh

CMD="$1"
shift

cd /data/local/tmp || exit 1
exec "/data/host$CMD" "$@"
