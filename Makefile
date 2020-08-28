DESTDIR := ~/.local/bin
BUILDTYPE := release

output: bkmk itmn

bkmk:
	cargo build --bin bkmk --$(BUILDTYPE)

itmn:
	cargo build --bin itmn --$(BUILDTYPE)

install: output
	@echo Installing to $(DESTDIR)...
	cd target/release && cp bkmk itmn -t $(DESTDIR)

check:
	cargo check

.PHONY: output install check
