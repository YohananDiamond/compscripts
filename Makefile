DESTDIR := ~/.local/bin
.PHONY: output install run

output: $(DEPS)
	cargo build --bin bkmk --release

install: output
	@echo Installing to $(DESTDIR)...
	cp target/release/bkmk -t $(DESTDIR)

run:
	@echo 'The "run" action is disabled here, since multiple binaries are being made.'
	@echo 'I might find another solution later.'
