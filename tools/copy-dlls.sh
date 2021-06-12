#!/bin/bash

set -eo pipefail

SYSROOT="$1" ; shift
ENTRY="$1"

TARGET_DIR="$(dirname "$ENTRY")"

get_dlls () {
	mingw-objdump -x "$1" | awk -F ': ' '/DLL Name/ && !/ADVAPI32|KERNEL32|WS2_32|GDI32|DNSAPI|SHELL32|COMCTL32|USERENV|MSIMG32|USER32|IPHLPAPI|WINSPOOL|SHLWAPI|IMM32|SETUPAPI|WINMM|msvcrt|ole32|dwmapi|comdlg32/ {print $2}'
}

for DLL_NAME in $(get_dlls "$ENTRY"); do
	echo "Looking for $DLL_NAME for $ENTRY in $SYSROOT" >&2
	FULL_PATH="$(find "$SYSROOT/bin" -name "$DLL_NAME" | head -1)"
	if [ -f "$FULL_PATH" ]; then
		TARGET_PATH="$TARGET_DIR/$DLL_NAME"
		if ! [ -f "$TARGET_PATH" ]; then
			# Copy the input DLL
			cp -t "$TARGET_DIR" "$FULL_PATH"
			# Recurse with the copied DLL
			if ! "$0" "$SYSROOT" "$TARGET_PATH"; then
				# Error occurred, remove DLL so it can be processed next time
				rm "$TARGET_PATH"
			fi
		fi
	else
		echo "$DLL_NAME not found" >&2
		exit 1
	fi
done
