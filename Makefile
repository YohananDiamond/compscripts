DESTDIR := ~/.local/bin
RELEASE := true
BINARIES := bkmk itmn

output:
	if [ $(RELEASE) = true ]; then cargo build --release; else cargo build; fi

install: output
	@echo Installing to $(DESTDIR)...
	cd target/release && cp $(BINARIES) -t $(DESTDIR)
	cp tools/compscripts-defaultedit -t $(DESTDIR)

check:
	cargo check

test:
	cargo test

.PHONY: output install check
