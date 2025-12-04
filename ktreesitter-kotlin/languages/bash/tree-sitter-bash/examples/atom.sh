#!/bin/bash

if [ "$(uname)" == 'Darwin' ]; then
  OS='Mac'
elif [ "$(expr substr $(uname -s) 1 5)" == 'Linux' ]; then
  OS='Linux'
else
  echo "Your platform ($(uname -a)) is not supported."
  exit 1
fi

if [ "$(basename $0)" == 'atom-beta' ]; then
  BETA_VERSION=true
else
  BETA_VERSION=
fi

export ATOM_DISABLE_SHELLING_OUT_FOR_ENVIRONMENT=true

while getopts ":wtfvh-:" opt; do
  case "$opt" in
    -)
      case "${OPTARG}" in
        wait)
          WAIT=1
          ;;
        help|version)
          REDIRECT_STDERR=1
          EXPECT_OUTPUT=1
          ;;
        foreground|benchmark|benchmark-test|test)
          EXPECT_OUTPUT=1
          ;;
      esac
      ;;
    w)
      WAIT=1
      ;;
    h|v)
      REDIRECT_STDERR=1
      EXPECT_OUTPUT=1
      ;;
    f|t)
      EXPECT_OUTPUT=1
      ;;
  esac
done

if [ $REDIRECT_STDERR ]; then
  exec 2> /dev/null
fi

if [ $EXPECT_OUTPUT ]; then
  export ELECTRON_ENABLE_LOGGING=1
fi

if [ $OS == 'Mac' ]; then
  if [ -L "$0" ]; then
    SCRIPT="$(readlink "$0")"
  else
    SCRIPT="$0"
  fi
  ATOM_APP="$(dirname "$(dirname "$(dirname "$(dirname "$SCRIPT")")")")"
  if [ "$ATOM_APP" == . ]; then
    unset ATOM_APP
  else
    ATOM_PATH="$(dirname "$ATOM_APP")"
    ATOM_APP_NAME="$(basename "$ATOM_APP")"
  fi

  if [ -n "$BETA_VERSION" ]; then
    ATOM_EXECUTABLE_NAME="Atom Beta"
  else
    ATOM_EXECUTABLE_NAME="Atom"
  fi

  if [ -z "${ATOM_PATH}" ]; then
    # If ATOM_PATH isn't set, check /Applications and then ~/Applications for Atom.app
    if [ -x "/Applications/$ATOM_APP_NAME" ]; then
      ATOM_PATH="/Applications"
    elif [ -x "$HOME/Applications/$ATOM_APP_NAME" ]; then
      ATOM_PATH="$HOME/Applications"
    else
      # We haven't found an Atom.app, use spotlight to search for Atom
      ATOM_PATH="$(mdfind "kMDItemCFBundleIdentifier == 'com.github.atom'" | grep -v ShipIt | head -1 | xargs -0 dirname)"

      # Exit if Atom can't be found
      if [ ! -x "$ATOM_PATH/$ATOM_APP_NAME" ]; then
        echo "Cannot locate ${ATOM_APP_NAME}, it is usually located in /Applications. Set the ATOM_PATH environment variable to the directory containing ${ATOM_APP_NAME}."
        exit 1
      fi
    fi
  fi

  if [ $EXPECT_OUTPUT ]; then
    "$ATOM_PATH/$ATOM_APP_NAME/Contents/MacOS/$ATOM_EXECUTABLE_NAME" --executed-from="$(pwd)" --pid=$$ "$@"
    exit $?
  else
    open -a "$ATOM_PATH/$ATOM_APP_NAME" -n --args --executed-from="$(pwd)" --pid=$$ --path-environment="$PATH" "$@"
  fi
elif [ $OS == 'Linux' ]; then
  SCRIPT=$(readlink -f "$0")
  USR_DIRECTORY=$(readlink -f $(dirname $SCRIPT)/..)

  if [ -n "$BETA_VERSION" ]; then
    ATOM_PATH="$USR_DIRECTORY/share/atom-beta/atom"
  else
    ATOM_PATH="$USR_DIRECTORY/share/atom/atom"
  fi

  ATOM_HOME="${ATOM_HOME:-$HOME/.atom}"
  mkdir -p "$ATOM_HOME"

  : ${TMPDIR:=/tmp}

  [ -x "$ATOM_PATH" ] || ATOM_PATH="$TMPDIR/atom-build/Atom/atom"

  if [ $EXPECT_OUTPUT ]; then
    "$ATOM_PATH" --executed-from="$(pwd)" --pid=$$ "$@"
    exit $?
  else
    (
    nohup "$ATOM_PATH" --executed-from="$(pwd)" --pid=$$ "$@" > "$ATOM_HOME/nohup.out" 2>&1
    if [ $? -ne 0 ]; then
      cat "$ATOM_HOME/nohup.out"
      exit $?
    fi
    ) &
  fi
fi

# Exits this process when Atom is used as $EDITOR
on_die() {
  exit 0
}
trap 'on_die' SIGQUIT SIGTERM

# If the wait flag is set, don't exit this process until Atom tells it to.
if [ $WAIT ]; then
  while true; do
    sleep 1
  done
fi
