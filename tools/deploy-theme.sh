#!/bin/bash

set -e

TARGET_DIR="$1" ; shift
THEME_NAME="$1" ; shift
ARCHIVE="$1" ; shift

# Fetch the theme files
THEME_PATH="$TARGET_DIR/share/themes/$THEME_NAME/gtk-3.0"
rm -rf "$THEME_PATH"
mkdir -p "$THEME_PATH"

if [[ $ARCHIVE == *.tar.* ]]; then
	# Get the top-level directory
	TOP_LEVEL_DIR=$(tar tf "$ARCHIVE" | head -1 | cut -d/ -f1)
	tar -C "$THEME_PATH" --strip-components=2 -xf "$ARCHIVE" "$TOP_LEVEL_DIR/gtk-3.0"
else
	# Just copy from the source
	cp -rav "$ARCHIVE/gtk-3.0" "$(dirname "$THEME_PATH")"
fi

# Write the settings file
ETC_PATH="$TARGET_DIR/etc/gtk-3.0"
rm -rf "$ETC_PATH"
mkdir -p "$ETC_PATH"

cat >"$ETC_PATH/settings.ini" <<EOT
[Settings]
gtk-theme-name=$THEME_NAME
EOT

# Deploy the icons
ICONS_PATH="$TARGET_DIR/share/icons"
rm -rf "$ICONS_PATH"
mkdir -p "$ICONS_PATH"
for THEME_NAME in Adwaita hicolor; do
	for SOURCE_LOC in "$MINGW_PREFIX/share/icons"; do
		if [ -d "$SOURCE_LOC/$THEME_NAME" ]; then
			rsync --exclude="cursors" --exclude="512x512" --exclude="256x256" \
				--exclude="scalable*/" --exclude="apps/" \
				-av "$SOURCE_LOC/$THEME_NAME" "$ICONS_PATH"
			gtk-update-icon-cache "$ICONS_PATH/$THEME_NAME"
			break
		fi
	done
done

# Deploy the strings
LOCALE_PATH="$TARGET_DIR/share/locale"
rm -rf "$LOCALE_PATH"
rsync -avm "$MINGW_PREFIX/share/locale/fr" "$LOCALE_PATH/"
