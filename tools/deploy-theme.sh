#!/bin/bash

set -e

TARGET_DIR="$1" ; shift
THEME_NAME="$1" ; shift
ARCHIVE="$1" ; shift

# Fetch the theme files
THEME_PATH="$TARGET_DIR/share/themes/$THEME_NAME/gtk-3.0"
mkdir -p "$THEME_PATH"

if [[ $ARCHIVE == *.tar.* ]]; then
	# Get the top-level directory
	TOP_LEVEL_DIR=$(tar tf "$ARCHIVE" | head -1 | cut -d/ -f1)
	tar -C "$THEME_PATH" --strip-components=2 -xf "$ARCHIVE" "$TOP_LEVEL_DIR/gtk-3.0"
else
	if [ -d "$ARCHIVE/gtk-3.0" ]; then
		# Just copy from the source
		cp -rav "$ARCHIVE/gtk-3.0" "$(dirname "$THEME_PATH")"
	else
		# Make this optional, since Adwaita is built-in
		echo "$ARCHIVE not found" >&2
	fi
fi

# Write the settings file
ETC_PATH="$TARGET_DIR/etc/gtk-3.0"
mkdir -p "$ETC_PATH"

cat >"$ETC_PATH/settings.ini" <<EOT
[Settings]
gtk-theme-name=$THEME_NAME
EOT

# Deploy the icons
ICONS_PATH="$TARGET_DIR/share/icons"
mkdir -p "$ICONS_PATH"
for THEME_NAME in Adwaita hicolor; do
	for SOURCE_LOC in "$MINGW_PREFIX/share/icons"; do
		if [ -d "$SOURCE_LOC/$THEME_NAME" ]; then
			tar --exclude="cursors" --exclude="512x512" --exclude="256x256" \
				--exclude="scalable" --exclude="apps" --exclude="scalable-up-to-32" \
				-C "$SOURCE_LOC" -c "$THEME_NAME" | tar -C "$ICONS_PATH" -xv
			gtk-update-icon-cache "$ICONS_PATH/$THEME_NAME"
			break
		fi
	done
done

# Deploy the strings
LOCALE_PATH="$TARGET_DIR/share/locale"
for LNG in fr; do
	mkdir -p "$LOCALE_PATH/$LNG"
	cp -rv "$MINGW_PREFIX/share/locale/$LNG" "$LOCALE_PATH"
done
