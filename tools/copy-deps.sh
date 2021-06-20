#!/bin/bash

set -e

TARGET_FILE="$1" ; shift
TARGET_DIR="$(dirname "$TARGET_FILE")/.."

THEME_FILE=/usr/share/themes/Adwaita
THEME_NAME=Adwaita

# For Windows builds, copy dependencies into target
if [[ $TARGET_FILE == *.exe ]]; then
	# Recursively find dependent DLLs
	./tools/copy-dlls.sh "$MINGW_PREFIX" "$TARGET_FILE"

	# Deploy schemas
	SCHEMAS="$TARGET_DIR/share/glib-2.0/schemas"
	mkdir -p "$SCHEMAS"
	cp -v "$MINGW_PREFIX/share/glib-2.0/schemas/gschemas.compiled" "$SCHEMAS"

	# Deploy loaders
	LIB="$TARGET_DIR/lib"
	mkdir -p "$LIB"
	cp -rv "$MINGW_PREFIX/lib/gdk-pixbuf-2.0" "$LIB"

	# Deploy gdbus
	cp -v "$MINGW_PREFIX/bin/gdbus.exe" "$TARGET_DIR/bin"
fi

# Deploy theme files
./tools/deploy-theme.sh "$TARGET_DIR" "$THEME_NAME" "$THEME_FILE"
