.PHONY: build run install uninstall

BINARY := shairport-dashboard

build:
	cargo build --release

run:
	cargo run --release

install: build
	install -m 755 target/release/$(BINARY) /usr/local/bin/$(BINARY)
	install -m 644 $(BINARY).service /etc/systemd/system/$(BINARY).service
	systemctl daemon-reload
	systemctl enable $(BINARY).service
	systemctl restart $(BINARY).service

uninstall:
	systemctl stop $(BINARY).service || true
	systemctl disable $(BINARY).service || true
	rm -f /usr/local/bin/$(BINARY)
	rm -f /etc/systemd/system/$(BINARY).service
	systemctl daemon-reload