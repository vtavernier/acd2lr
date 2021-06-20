# Fedora mingw prefix
MINGW_PREFIX:=/usr/x86_64-w64-mingw32/sys-root/mingw
export MINGW_PREFIX

WINDOWS_BUILD_ARGS:=export PKG_CONFIG_ALLOW_CROSS=1 ; \
		export PKG_CONFIG_PATH=$(MINGW_PREFIX)/lib/pkgconfig

WINDOWS_TRIPLE:=x86_64-pc-windows-gnu

BINARY_NAME:=acd2lr
PACKAGE_VERSION:=$(shell cargo metadata --format-version=1 | jq -r '.packages[0].version')
BASE_DIR:=$(shell pwd)

BUILD_DIR:=$(BASE_DIR)/build
BUILD_DIR_WINDOWS:=$(BUILD_DIR)/win64
BUILD_DIR_WINDOWS_DEBUG:=$(BUILD_DIR_WINDOWS)-dbg
BUILD_DIR_WINDOWS_RELEASE:=$(BUILD_DIR_WINDOWS)-rel

all:

$(BUILD_DIR_WINDOWS_DEBUG): $(BUILD_DIR_WINDOWS_DEBUG)/bin
$(BUILD_DIR_WINDOWS_RELEASE): $(BUILD_DIR_WINDOWS_RELEASE)/bin
$(BUILD_DIR_WINDOWS_DEBUG)/bin:
	mkdir -p $@
$(BUILD_DIR_WINDOWS_RELEASE)/bin:
	mkdir -p $@
.PHONY: $(BUILD_DIR_WINDOWS_DEBUG) $(BUILD_DIR_WINDOWS_RELEASE)

build-windows: $(BUILD_DIR_WINDOWS_DEBUG)
	eval $(WINDOWS_BUILD_ARGS) && \
		cargo build --target $(WINDOWS_TRIPLE) && \
		cp target/$(WINDOWS_TRIPLE)/debug/$(BINARY_NAME).exe $(BUILD_DIR_WINDOWS_DEBUG)/bin/ && \
		./tools/copy-deps.sh $(BUILD_DIR_WINDOWS_DEBUG)/bin/$(BINARY_NAME).exe

build-windows-release: $(BUILD_DIR_WINDOWS_RELEASE)
	eval $(WINDOWS_BUILD_ARGS) && \
		cargo build --target $(WINDOWS_TRIPLE) --release && \
		cp target/$(WINDOWS_TRIPLE)/release/$(BINARY_NAME).exe $(BUILD_DIR_WINDOWS_RELEASE)/bin/ && \
		mingw-strip $(BUILD_DIR_WINDOWS_RELEASE)/bin/$(BINARY_NAME).exe && \
		./tools/copy-deps.sh $(BUILD_DIR_WINDOWS_RELEASE)/bin/$(BINARY_NAME).exe

package-windows: installer.nsi build-windows-release
	makensis \
		-DROOT=$(BUILD_DIR_WINDOWS_RELEASE) \
		-DINSTALLSIZE=$$(du -s -b $(patsubst %,$(BUILD_DIR_WINDOWS_RELEASE)/%,bin etc lib share) \
			| awk '{sum+=$$1} END{print int(sum/1024)}') \
		-DOUTFILE=$(BUILD_DIR_WINDOWS_RELEASE)/installer.exe \
		-DVERSIONMAJOR=$$(echo $(PACKAGE_VERSION) | cut -d. -f1) \
		-DVERSIONMINOR=$$(echo $(PACKAGE_VERSION) | cut -d. -f2) \
		-DVERSIONBUILD=$$(echo $(PACKAGE_VERSION) | cut -d. -f3) \
		$<

clean:
	rm -rf $(BUILD_DIR)

realclean: clean
	cargo clean

.PHONY: all build-windows build-windows-release package-windows clean realclean
