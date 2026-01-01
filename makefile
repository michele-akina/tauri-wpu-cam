.PHONY: format lint dev bench

lint:
	cd src-tauri && \
	cargo clippy -- -D warnings && \
	cargo fmt -- --check && \
	cargo machete

format:
	cd src-tauri && \
	cargo fmt && \
	cargo fix

bench:
	cd src-tauri && \
	cargo bench

dev:
	RUST_LOG=info yarn run tauri dev
