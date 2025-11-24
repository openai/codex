#!/bin/sh

HERE="`echo $0 | sed -e 's|[^/]*$||'`"
OPENSSL="${HERE}../apps/openssl"

if [ -d "${HERE}../engines" -a "x$OPENSSL_ENGINES" = "x" ]; then
	OPENSSL_ENGINES="${HERE}../engines"; export OPENSSL_ENGINES
fi
if [ -d "${HERE}../providers" -a "x$OPENSSL_MODULES" = "x" ]; then
	OPENSSL_MODULES="${HERE}../providers"; export OPENSSL_MODULES
fi

if [ -x "${OPENSSL}.exe" ]; then
	# The original reason for this script existence is to work around
	# certain caveats in run-time linker behaviour. On Windows platforms
	# adjusting $PATH used to be sufficient, but with introduction of
	# SafeDllSearchMode in XP/2003 the only way to get it right in
	# *all* possible situations is to copy newly built .DLLs to apps/
	# and test/, which is now done elsewhere... The $PATH is adjusted
	# for backward compatibility (and nostagical reasons:-).
	if [ "$OSTYPE" != msdosdjgpp ]; then
		PATH="${HERE}..:$PATH"; export PATH
	fi
	exec "${OPENSSL}.exe" "$@"
elif [ -x "${OPENSSL}" -a -x "${HERE}shlib_wrap.sh" ]; then
	exec "${HERE}shlib_wrap.sh" "${OPENSSL}" "$@"
else
	exec "${OPENSSL}" "$@"	# hope for the best...
fi
