.PHONY:  format lint dev

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
	yarn run tauri dev
