.PHONY: configure-rust-old
configure-rust-old:
	rustup toolchain install nightly-2023-01-01
	rustup target add wasm32-unknown-unknown --toolchain nightly-2023-01-01
	rustup component add clippy

PHONY: configure-rust
configure-rust:
	rustup toolchain install nightly
	rustup target add wasm32-unknown-unknown --toolchain nightly
	rustup component add clippy

.PHONY: init
init:
	make configure-rust
# 	git submodule update --init --recursive

.PHONY: release
release:
	make configure-rust
	rm -rf target/
	cargo build --manifest-path node/Cargo.toml --features with-ethereum-compatibility --release

.PHONY: build
build:
	cargo build --manifest-path node/Cargo.toml --release
# 	cargo build --manifest-path node/Cargo.toml --features runtime-benchmarks,with-ethereum-compatibility --release

.PHONY: wasm
wasm:
	cargo build -p reef-runtime --features with-ethereum-compatibility --release

.PHONY: genesis
genesis:
	make release
	./target/release/reef-node build-spec --chain testnet-new > assets/chain_spec_testnet.json
	./target/release/reef-node build-spec --chain mainnet-new > assets/chain_spec_mainnet.json
	./target/release/reef-node build-spec --chain testnet-new --raw > assets/chain_spec_testnet_raw.json
	./target/release/reef-node build-spec --chain mainnet-new --raw > assets/chain_spec_mainnet_raw.json

.PHONY: check
check:
	SKIP_WASM_BUILD=1 cargo check

.PHONY: clippy
clippy:
	SKIP_WASM_BUILD=1 cargo clippy -- -D warnings -A clippy::from-over-into -A clippy::unnecessary-cast -A clippy::identity-op -A clippy::upper-case-acronyms

.PHONY: watch
watch:
	SKIP_WASM_BUILD=1 cargo watch -c -x build

.PHONY: test
test:
	SKIP_WASM_BUILD=1 cargo test --features with-ethereum-compatibility --all
	SKIP_WASM_BUILD=1 cargo test --all


.PHONY: test-print
test-print:
	SKIP_WASM_BUILD=1 cargo test --features with-ethereum-compatibility --all -- --nocapture
	SKIP_WASM_BUILD=1 cargo test --all -- --nocapture

.PHONY: debug
debug:
	cargo build && RUST_LOG=debug RUST_BACKTRACE=1 rust-gdb --args target/debug/reef-node --dev --tmp -lruntime=debug

.PHONY: run
run:
	RUST_BACKTRACE=1 cargo run --manifest-path node/Cargo.toml --features with-ethereum-compatibility  -- --dev --tmp

.PHONY: log
log:
	RUST_BACKTRACE=1 RUST_LOG=debug cargo run --manifest-path node/Cargo.toml --features with-ethereum-compatibility  -- --dev --tmp

.PHONY: noeth
noeth:
	RUST_BACKTRACE=1 cargo run -- --dev --tmp

.PHONY: bench
bench:
	SKIP_WASM_BUILD=1 cargo test --manifest-path node/Cargo.toml --features runtime-benchmarks,with-ethereum-compatibility benchmarking

.PHONY: doc
doc:
	SKIP_WASM_BUILD=1 cargo doc --open

.PHONY: cargo-update
cargo-update:
	cargo update
	cargo update --manifest-path node/Cargo.toml
	make test

.PHONY: fork
fork:
	npm i --prefix fork fork
ifeq (,$(wildcard fork/data))
	mkdir fork/data
endif
	cp target/release/reef-node fork/data/binary
	cp target/release/wbuild/reef-runtime/reef_runtime.compact.wasm fork/data/runtime.wasm
	cp assets/types.json fork/data/schema.json
	cp assets/chain_spec_$(chain)_raw.json fork/data/genesis.json
	cd fork && npm start && cd ..

.PHONY: run-local
run-local:
	@echo "=========================================="
	@echo "Setting up local validator network..."
	@echo "=========================================="
	@# Build the node binary
	@echo "\n[1/6] Building release binary..."
	@make build
	@# Clean up previous chain data
	@echo "\n[2/6] Cleaning up previous chain data..."
	@rm -rf /tmp/alice /tmp/bob
	@# Generate chain spec
	@echo "\n[3/6] Generating local chain spec..."
	@./target/release/reef-node build-spec --chain local --disable-default-bootnode > /tmp/local-chain-spec.json
	@./target/release/reef-node build-spec --chain /tmp/local-chain-spec.json --disable-default-bootnode --raw > /tmp/local-chain-spec-raw.json
	@# Insert keys for Alice
	@echo "\n[4/6] Inserting keys for Alice..."
	@./target/release/reef-node key insert --base-path /tmp/alice \
		--chain=/tmp/local-chain-spec-raw.json \
		--scheme Sr25519 \
		--suri //Alice \
		--key-type babe
	@./target/release/reef-node key insert --base-path /tmp/alice \
		--chain=/tmp/local-chain-spec-raw.json \
		--scheme Ed25519 \
		--suri //Alice \
		--key-type gran
	@./target/release/reef-node key insert --base-path /tmp/alice \
		--chain=/tmp/local-chain-spec-raw.json \
		--scheme Sr25519 \
		--suri //Alice \
		--key-type imon
	@./target/release/reef-node key insert --base-path /tmp/alice \
		--chain=/tmp/local-chain-spec-raw.json \
		--scheme Sr25519 \
		--suri //Alice \
		--key-type audi
	@# Insert keys for Bob
	@echo "\n[5/6] Inserting keys for Bob..."
	@./target/release/reef-node key insert --base-path /tmp/bob \
		--chain=/tmp/local-chain-spec-raw.json \
		--scheme Sr25519 \
		--suri //Bob \
		--key-type babe
	@./target/release/reef-node key insert --base-path /tmp/bob \
		--chain=/tmp/local-chain-spec-raw.json \
		--scheme Ed25519 \
		--suri //Bob \
		--key-type gran
	@./target/release/reef-node key insert --base-path /tmp/bob \
		--chain=/tmp/local-chain-spec-raw.json \
		--scheme Sr25519 \
		--suri //Bob \
		--key-type imon
	@./target/release/reef-node key insert --base-path /tmp/bob \
		--chain=/tmp/local-chain-spec-raw.json \
		--scheme Sr25519 \
		--suri //Bob \
		--key-type audi
	@# Start the validator nodes
	@echo "\n[6/6] Starting validator nodes..."
	@echo "\nAlice node will run on:"
	@echo "  - P2P port: 30333"
	@echo "  - RPC port: 9944"
	@echo "  - WebSocket: ws://127.0.0.1:9944"
	@echo "\nBob node will run on:"
	@echo "  - P2P port: 30334"
	@echo "  - RPC port: 9945"
	@echo "  - WebSocket: ws://127.0.0.1:9945"
	@echo "\n=========================================="
	@echo "Starting nodes in separate terminals..."
	@echo "Press Ctrl+C in each terminal to stop"
	@echo "=========================================="
	@# Start Alice in a new terminal
	@osascript -e 'tell app "Terminal" to do script "cd $(PWD) && ./target/release/reef-node \
		--base-path /tmp/alice \
		--chain /tmp/local-chain-spec-raw.json \
		--alice \
		--port 30333 \
		--rpc-port 9944 \
		--node-key 0000000000000000000000000000000000000000000000000000000000000001 \
		--validator \
		--rpc-cors all \
		--rpc-methods Unsafe \
		--rpc-external \
		--name AliceNode"' &
	@sleep 2
	@# Start Bob in a new terminal
	@osascript -e 'tell app "Terminal" to do script "cd $(PWD) && ./target/release/reef-node \
		--base-path /tmp/bob \
		--chain /tmp/local-chain-spec-raw.json \
		--bob \
		--port 30334 \
		--rpc-port 9945 \
		--node-key 0000000000000000000000000000000000000000000000000000000000000002 \
		--bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp \
		--validator \
		--rpc-cors all \
		--rpc-methods Unsafe \
		--rpc-external \
		--name BobNode"' &
	@sleep 1
	@echo "\n✅ Local validator network started!"
	@echo "Connect to Alice: ws://127.0.0.1:9944"
	@echo "Connect to Bob: ws://127.0.0.1:9945"
