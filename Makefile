DESTDIR := ~/.local/bin
.PHONY: output install check

output: $(DEPS)
	cargo build --bin bkmk --release
	cargo build --bin tkmn --release

install: output
	@echo Installing to $(DESTDIR)...
	cd target/release && cp bkmk tkmn -t $(DESTDIR)

check:
	cargo check
