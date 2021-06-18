# Fedora mingw prefix
MINGW_PREFIX=/usr/x86_64-w64-mingw32/sys-root/mingw
export MINGW_PREFIX

WINDOWS_BUILD_ARGS=export PKG_CONFIG_ALLOW_CROSS=1 ; \
		export PKG_CONFIG_PATH=$(MINGW_PREFIX)/lib/pkgconfig

WINDOWS_TRIPLE=x86_64-pc-windows-gnu

BINARY_NAME=acd2lr
BASE_DIR=$(shell pwd)

all:

build-windows:
	eval $(WINDOWS_BUILD_ARGS) && \
		cargo build --target $(WINDOWS_TRIPLE) && \
		./tools/copy-deps.sh target/$(WINDOWS_TRIPLE)/debug/$(BINARY_NAME).exe

build-windows-release:
	eval $(WINDOWS_BUILD_ARGS) && \
		cargo build --target $(WINDOWS_TRIPLE) --release && \
		mingw-strip target/$(WINDOWS_TRIPLE)/release/$(BINARY_NAME).exe && \
		./tools/copy-deps.sh target/$(WINDOWS_TRIPLE)/release/$(BINARY_NAME).exe

package-windows: installer.exe

installer.exe: installer.nsi
	makensis -DINSTALLSIZE=$$(du -s -b $(patsubst %,target/$(WINDOWS_TRIPLE)/release/%,bin etc lib share) | awk '{sum+=$$1} END{print int(sum/1024)}') $^

clean:
	cargo clean

.PHONY: all build-windows build-windows-release package-windows installer.exe
