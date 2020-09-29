DESTDIR := ~/.local/bin
BUILD_TYPE := release

output:
	cargo build --bin bkmk --$(BUILD_TYPE)
	cargo build --bin itmn --$(BUILD_TYPE)

install: output
	@echo Installing to $(DESTDIR)...
	cd target/release && cp bkmk itmn -t $(DESTDIR)

check:
	cargo check --bin bkmk
	cargo check --bin itmn

.PHONY: output install check
