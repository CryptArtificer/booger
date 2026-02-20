.PHONY: build install release check clippy test clean index status

build:
	cargo build

release:
	cargo build --release

install:
	cargo install --path .

check:
	cargo check
	cargo clippy

test:
	cargo test

clean:
	cargo clean
	rm -rf .booger

index:
	booger index .

status:
	booger status

fmt:
	cargo fmt

fmt-check:
	cargo fmt -- --check

all: check test install
