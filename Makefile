.PHONY: build test install release lint fmt cargo-check check clean doc \
        completions man docker smoke doctor setup repo-map wire-smoke \
        install-completions install-man bench profile

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

cargo-check:
	$(CARGO) check --all-targets

check: fmt cargo-check lint test

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

repo-map:
	./scripts/repo-map.sh

wire-smoke:
	./scripts/kimi-wire-smoke.sh

# Benchmarks
bench:
	$(CARGO) bench

# Profiling (requires cargo-flamegraph: cargo install cargo-flamegraph)
profile:
	@which cargo-flamegraph > /dev/null 2>&1 || { echo "cargo-flamegraph not installed. Run: cargo install cargo-flamegraph"; exit 1; }
	$(CARGO) flamegraph --bin $(BINARY_NAME) -- team run 1:coder "benchmark test"
