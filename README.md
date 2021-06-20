# [acd2lr](https://github.com/vtavernier/acd2lr)

## Building

### Linux (Fedora 34)

	sudo dnf install -y gtk3-devel
	cargo run

### Windows (cross-compile from Fedora 34)

	sudo dnf install -y mingw64-gcc mingw64-pango mingw64-poppler mingw64-gtk3 mingw64-winpthreads-static \
		mingw64-hicolor-icon-theme mingw64-adwaita-icon-theme mingw32-nsis jq
	rustup target add x86_64-pc-windows-gnu
	make build-windows

## Author

Vincent Tavernier <vince.tavernier@gmail.com>

## License

[MIT](LICENSE)
