.PHONY: build test install release lint fmt check clean doc \
        completions man docker smoke doctor setup \
        install-completions install-man

BINARY_NAME = omk
CARGO = cargo
PREFIX = $(HOME)/.local

build:
	$(CARGO) build --release

test:
	$(CARGO) test

lint:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

fmt:
	$(CARGO) fmt --check

fmt-fix:
	$(CARGO) fmt

check: fmt lint test

install:
	$(CARGO) install --path .

release:
	$(CARGO) build --release
	@echo "Binary: target/release/$(BINARY_NAME)"

clean:
	$(CARGO) clean

doc:
	$(CARGO) doc --no-deps --open

# Shell completions
completions: build
	mkdir -p completions
	./target/release/$(BINARY_NAME) completions bash > completions/$(BINARY_NAME).bash
	./target/release/$(BINARY_NAME) completions zsh > completions/_$(BINARY_NAME)
	./target/release/$(BINARY_NAME) completions fish > completions/$(BINARY_NAME).fish
	@echo "Completions written to completions/"

install-completions: completions
	install -d $(PREFIX)/share/bash-completion/completions
	install -d $(PREFIX)/share/zsh/site-functions
	install -d $(PREFIX)/share/fish/vendor_completions.d
	install completions/$(BINARY_NAME).bash $(PREFIX)/share/bash-completion/completions/$(BINARY_NAME)
	install completions/_$(BINARY_NAME) $(PREFIX)/share/zsh/site-functions/
	install completions/$(BINARY_NAME).fish $(PREFIX)/share/fish/vendor_completions.d/

# Man page
man: build
	mkdir -p man
	./target/release/$(BINARY_NAME) man > man/$(BINARY_NAME).1
	@echo "Man page written to man/$(BINARY_NAME).1"

install-man: man
	install -d $(PREFIX)/share/man/man1
	install man/$(BINARY_NAME).1 $(PREFIX)/share/man/man1/

# Docker
docker:
	docker build -t $(BINARY_NAME):latest .

# Quick smoke test
smoke: build
	./target/release/$(BINARY_NAME) --help
	./target/release/$(BINARY_NAME) doctor
	./target/release/$(BINARY_NAME) setup
	./target/release/$(BINARY_NAME) team --help
	./target/release/$(BINARY_NAME) config show

# Development helpers
doctor: build
	./target/release/$(BINARY_NAME) doctor

setup: build
	./target/release/$(BINARY_NAME) setup
