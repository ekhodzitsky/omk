.PHONY: build test install release lint fmt check clean doc

BINARY_NAME = omk
CARGO = cargo

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

# Quick smoke test
smoke: build
	./target/release/$(BINARY_NAME) --help
	./target/release/$(BINARY_NAME) setup
	./target/release/$(BINARY_NAME) team --help
