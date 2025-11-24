#! /bin/sh

HERE=`dirname $0`

for f in "$@"; do
    # It's worth nothing that 'openssl sha256 -r' assumes that all input
    # is binary.  This isn't quite true, and we know better, so we convert
    # the '*stdin' marker to the filename preceded by a space.  See the
    # sha1sum manual for a specification of the format.
    case "$f" in
        *.c | *.c.in | *.h | *.h.in | *.inc)
            cat "$f" \
                | $HERE/lang-compress.pl 'C' \
                | unifdef -DFIPS_MODULE=1 \
                | openssl sha256 -r \
                | sed -e "s| \\*stdin|  $f|"
            ;;
        *.pl )
            cat "$f" \
                | $HERE/lang-compress.pl 'perl' \
                | openssl sha256 -r \
                | sed -e "s| \\*stdin|  $f|"
            ;;
        *.S )
            cat "$f" \
                | $HERE/lang-compress.pl 'S' \
                | openssl sha256 -r \
                | sed -e "s| \\*stdin|  $f|"
            ;;
    esac
done
