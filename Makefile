DESTDIR := ~/.local/bin
.PHONY: output install run

output: $(DEPS)
	cargo build --bin bkmk --release
	cargo build --bin tkmn --release

install: output
	@echo Installing to $(DESTDIR)...
	cd target/release && cp bkmk tkmn -t $(DESTDIR)

run:
	@echo 'The "run" action is disabled here, since multiple binaries are being made.'
	@echo 'I might find another solution later.'
