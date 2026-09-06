.PHONY: check ensure-deny run

CARGO_DENY_VERSION := 0.20.2

# Recipes remain sequential even when invoked with make -j.
check:
	cargo fmt --check
	cargo clippy --all-targets --locked -- -D warnings
	LIVE=0 cargo test --locked
	$(MAKE) ensure-deny
	cargo deny check
	cargo build --locked --release

ensure-deny:
	@if [ "$$(cargo deny --version 2>/dev/null)" != "cargo-deny $(CARGO_DENY_VERSION)" ]; then \
		cargo install cargo-deny --version '=$(CARGO_DENY_VERSION)' --locked; \
	fi

run:
	@cargo run --quiet --locked
