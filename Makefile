DESTDIR := ~/.local/bin
.PHONY: output install check

output: $(DEPS)
	cargo build --bin bkmk --release
	cargo build --bin itmn --release

install: output
	@echo Installing to $(DESTDIR)...
	cd target/release && cp bkmk itmn -t $(DESTDIR)

check:
	cargo check
