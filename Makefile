.PHONY: build run check test install uninstall

BINARY := shairport-dashboard
ASSET_DIR := /usr/local/share/$(BINARY)
ASSETS := src/index.html src/index.mjs src/index.css

build:
	cargo build --release

run:
	cargo run --release

check:
	cargo check

test:
	cargo test

install: build
	sudo install -m 755 target/release/$(BINARY) /usr/local/bin/$(BINARY)
	sudo install -d $(ASSET_DIR)
	sudo install -m 644 $(ASSETS) $(ASSET_DIR)/
	sudo install -m 644 $(BINARY).service /etc/systemd/system/$(BINARY).service
	sudo systemctl daemon-reload
	sudo systemctl enable $(BINARY).service
	sudo systemctl restart $(BINARY).service

uninstall:
	sudo systemctl stop $(BINARY).service || true
	sudo systemctl disable $(BINARY).service || true
	sudo rm -f /usr/local/bin/$(BINARY)
	sudo rm -rf $(ASSET_DIR)
	sudo rm -f /etc/systemd/system/$(BINARY).service
	sudo systemctl daemon-reload

reinstall: uninstall install