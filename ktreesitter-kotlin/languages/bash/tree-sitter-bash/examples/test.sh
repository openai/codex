#!/usr/bin/env bash

set -e

function usage {
  cat <<-EOF
USAGE

  $0  [-dgGhv] [-f focus-string] [-s seed]

OPTIONS

  -h  print this message

  -b  run make under scan-build static analyzer

  -d  run tests in a debugger (either lldb or gdb)

  -g  run tests with valgrind's memcheck tool

  -G  run tests with valgrind's memcheck tool, including a full leak check

  -v  run tests with verbose output

  -f  run only tests whose description contain the given string

  -s  set the seed used to control random behavior

  -z  pipe tests' stderr to \`dot(1)\` to render an SVG log


EOF
}

profile=
leak_check=no
mode=normal
verbose=
args=()
target=tests
export BUILDTYPE=Test
cmd="out/${BUILDTYPE}/${target}"
run_scan_build=

if [ "$(uname -s)" == "Darwin" ]; then
  export LINK="clang++ -fsanitize=address"
fi

while getopts "bdf:s:gGhpvS" option; do
  case ${option} in
    h)
      usage
      exit
      ;;
    d)
      mode=debug
      ;;
    g)
      mode=valgrind
      ;;
    G)
      mode=valgrind
      leak_check=full
      ;;
    p)
      profile=true
      ;;
    f)
      args+=("--only=${OPTARG}")
      ;;
    v)
      verbose=true
      ;;
    s)
      export TREE_SITTER_SEED=${OPTARG}
      ;;
    S)
      mode=SVG
      ;;
    b)
      run_scan_build=true
      ;;
  esac
done

if [[ -n $verbose ]]; then
  args+=("--reporter=spec")
else
  args+=("--reporter=singleline")
fi

if [[ -n "$run_scan_build" ]]; then
  . script/util/scan-build.sh
  scan_build make -j2 $target
else
  make -j2 $target
fi
args=${args:-""}

if [[ -n $profile ]]; then
  export CPUPROFILE=/tmp/${target}-$(date '+%s').prof
fi

case ${mode} in
  valgrind)
    valgrind                                     \
      --suppressions=./script/util/valgrind.supp \
      --dsymutil=yes                             \
      --leak-check=${leak_check}                 \
      $cmd "${args[@]}" 2>&1 |                   \
      grep --color -E '\w+_tests?.cc:\d+|$'
    ;;

  debug)
    if which -s lldb; then
      lldb $cmd -- "${args[@]}"
    elif which -s gdb; then
      gdb $cmd -- "${args[@]}"
    else
      echo "No debugger found"
      exit 1
    fi
    ;;

  SVG)
    echo "<!DOCTYPE html><style>svg { width: 100%; margin-bottom: 20px; }</style>" > index.html
    $cmd "${args[@]}" 2> >(grep -v 'Assertion failed' | dot -Tsvg >> index.html)
    echo "Wrote index.html"
    ;;

  normal)
    time $cmd "${args[@]}"
    ;;
esac

if [[ -n $profile ]]; then
  pprof $cmd $CPUPROFILE
fi
