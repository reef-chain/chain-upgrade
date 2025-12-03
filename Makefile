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
	@echo "\n[1/7] Building release binary..."
	@make build
	@# Clean up previous chain data
	@echo "\n[2/7] Cleaning up previous chain data..."
	@rm -rf /tmp/validator1 /tmp/validator2 /tmp/validator3 /tmp/bootnode /tmp/validator1.txt /tmp/validator2.txt /tmp/validator3.txt /tmp/v1_seed.txt /tmp/v2_seed.txt /tmp/v3_seed.txt /tmp/v1_addr.txt /tmp/v2_addr.txt /tmp/v3_addr.txt /tmp/bootnode_peer_id.txt /tmp/bootnode_node_key.txt /tmp/v1_node_key.txt /tmp/v2_node_key.txt /tmp/v3_node_key.txt /tmp/local-chain-spec.json /tmp/local-chain-spec-updated.json /tmp/local-chain-spec-raw.json
	@# Generate new accounts and update chain spec
	@echo "\n[3/7] Generating new validator accounts..."
	@./target/release/reef-node key generate --scheme Sr25519 --output-type json > /tmp/validator1.txt
	@./target/release/reef-node key generate --scheme Sr25519 --output-type json > /tmp/validator2.txt
	@./target/release/reef-node key generate --scheme Sr25519 --output-type json > /tmp/validator3.txt
	@# Generate random node keys
	@echo "\n[4/7] Generating random node keys..."
	@./target/release/reef-node key generate-node-key --chain local > /tmp/bootnode_node_key.txt
	@./target/release/reef-node key generate-node-key --chain local > /tmp/v1_node_key.txt
	@./target/release/reef-node key generate-node-key --chain local > /tmp/v2_node_key.txt
	@./target/release/reef-node key generate-node-key --chain local > /tmp/v3_node_key.txt
	@echo "\n[5/7] Generating and updating chain spec..."
	@./target/release/reef-node build-spec --chain testnet-new --disable-default-bootnode > /tmp/local-chain-spec.json
	@# Extract keys and update chain spec using a shell script
	@bash -c ' \
		set -e; \
		V1_SEED=$$(grep -o "\"secretSeed\": \"[^\"]*\"" /tmp/validator1.txt | cut -d"\"" -f4); \
		V1_ADDR=$$(grep -o "\"ss58Address\": \"[^\"]*\"" /tmp/validator1.txt | cut -d"\"" -f4); \
		V2_SEED=$$(grep -o "\"secretSeed\": \"[^\"]*\"" /tmp/validator2.txt | cut -d"\"" -f4); \
		V2_ADDR=$$(grep -o "\"ss58Address\": \"[^\"]*\"" /tmp/validator2.txt | cut -d"\"" -f4); \
		V3_SEED=$$(grep -o "\"secretSeed\": \"[^\"]*\"" /tmp/validator3.txt | cut -d"\"" -f4); \
		V3_ADDR=$$(grep -o "\"ss58Address\": \"[^\"]*\"" /tmp/validator3.txt | cut -d"\"" -f4); \
		echo "Validator 1: $$V1_ADDR"; \
		echo "Validator 2: $$V2_ADDR"; \
		echo "Validator 3: $$V3_ADDR"; \
		echo "Deriving session keys..."; \
		V1_BABE=$$(./target/release/reef-node key inspect --scheme Sr25519 "$$V1_SEED//babe" --output-type json 2>/dev/null | grep -o "\"ss58Address\": \"[^\"]*\"" | cut -d"\"" -f4); \
		V1_GRAN=$$(./target/release/reef-node key inspect --scheme Ed25519 "$$V1_SEED//grandpa" --output-type json 2>/dev/null | grep -o "\"ss58Address\": \"[^\"]*\"" | cut -d"\"" -f4); \
		V1_IMON=$$(./target/release/reef-node key inspect --scheme Sr25519 "$$V1_SEED//im_online" --output-type json 2>/dev/null | grep -o "\"ss58Address\": \"[^\"]*\"" | cut -d"\"" -f4); \
		V1_AUDI=$$(./target/release/reef-node key inspect --scheme Sr25519 "$$V1_SEED//authority_discovery" --output-type json 2>/dev/null | grep -o "\"ss58Address\": \"[^\"]*\"" | cut -d"\"" -f4); \
		V2_BABE=$$(./target/release/reef-node key inspect --scheme Sr25519 "$$V2_SEED//babe" --output-type json 2>/dev/null | grep -o "\"ss58Address\": \"[^\"]*\"" | cut -d"\"" -f4); \
		V2_GRAN=$$(./target/release/reef-node key inspect --scheme Ed25519 "$$V2_SEED//grandpa" --output-type json 2>/dev/null | grep -o "\"ss58Address\": \"[^\"]*\"" | cut -d"\"" -f4); \
		V2_IMON=$$(./target/release/reef-node key inspect --scheme Sr25519 "$$V2_SEED//im_online" --output-type json 2>/dev/null | grep -o "\"ss58Address\": \"[^\"]*\"" | cut -d"\"" -f4); \
		V2_AUDI=$$(./target/release/reef-node key inspect --scheme Sr25519 "$$V2_SEED//authority_discovery" --output-type json 2>/dev/null | grep -o "\"ss58Address\": \"[^\"]*\"" | cut -d"\"" -f4); \
		V3_BABE=$$(./target/release/reef-node key inspect --scheme Sr25519 "$$V3_SEED//babe" --output-type json 2>/dev/null | grep -o "\"ss58Address\": \"[^\"]*\"" | cut -d"\"" -f4); \
		V3_GRAN=$$(./target/release/reef-node key inspect --scheme Ed25519 "$$V3_SEED//grandpa" --output-type json 2>/dev/null | grep -o "\"ss58Address\": \"[^\"]*\"" | cut -d"\"" -f4); \
		V3_IMON=$$(./target/release/reef-node key inspect --scheme Sr25519 "$$V3_SEED//im_online" --output-type json 2>/dev/null | grep -o "\"ss58Address\": \"[^\"]*\"" | cut -d"\"" -f4); \
		V3_AUDI=$$(./target/release/reef-node key inspect --scheme Sr25519 "$$V3_SEED//authority_discovery" --output-type json 2>/dev/null | grep -o "\"ss58Address\": \"[^\"]*\"" | cut -d"\"" -f4); \
		echo "V1 BABE: $$V1_BABE, GRAN: $$V1_GRAN, IMON: $$V1_IMON, AUDI: $$V1_AUDI"; \
		echo "V2 BABE: $$V2_BABE, GRAN: $$V2_GRAN, IMON: $$V2_IMON, AUDI: $$V2_AUDI"; \
		echo "V3 BABE: $$V3_BABE, GRAN: $$V3_GRAN, IMON: $$V3_IMON, AUDI: $$V3_AUDI"; \
		echo "$$V1_SEED" > /tmp/v1_seed.txt; \
		echo "$$V2_SEED" > /tmp/v2_seed.txt; \
		echo "$$V3_SEED" > /tmp/v3_seed.txt; \
		echo "$$V1_ADDR" > /tmp/v1_addr.txt; \
		echo "$$V2_ADDR" > /tmp/v2_addr.txt; \
		echo "$$V3_ADDR" > /tmp/v3_addr.txt; \
		echo "Updating chain spec..."; \
		jq ".genesis.runtimeGenesis.patch.balances.balances += [[\"$$V1_ADDR\", 100000000000000000000000000], [\"$$V2_ADDR\", 100000000000000000000000000], [\"$$V3_ADDR\", 100000000000000000000000000]]" /tmp/local-chain-spec.json | \
		jq ".genesis.runtimeGenesis.patch.session.keys = [[\"$$V1_ADDR\", \"$$V1_ADDR\", {\"authority_discovery\": \"$$V1_AUDI\", \"babe\": \"$$V1_BABE\", \"grandpa\": \"$$V1_GRAN\", \"im_online\": \"$$V1_IMON\"}], [\"$$V2_ADDR\", \"$$V2_ADDR\", {\"authority_discovery\": \"$$V2_AUDI\", \"babe\": \"$$V2_BABE\", \"grandpa\": \"$$V2_GRAN\", \"im_online\": \"$$V2_IMON\"}], [\"$$V3_ADDR\", \"$$V3_ADDR\", {\"authority_discovery\": \"$$V3_AUDI\", \"babe\": \"$$V3_BABE\", \"grandpa\": \"$$V3_GRAN\", \"im_online\": \"$$V3_IMON\"}]]" | \
		jq ".genesis.runtimeGenesis.patch.staking.invulnerables = [\"$$V1_ADDR\", \"$$V2_ADDR\", \"$$V3_ADDR\"]" | \
		jq ".genesis.runtimeGenesis.patch.staking.stakers = [[\"$$V1_ADDR\", \"$$V1_ADDR\", 1000000000000000000000000, \"Validator\"], [\"$$V2_ADDR\", \"$$V2_ADDR\", 1000000000000000000000000, \"Validator\"], [\"$$V3_ADDR\", \"$$V3_ADDR\", 1000000000000000000000000, \"Validator\"]]" \
		> /tmp/local-chain-spec-updated.json \
	'
	@./target/release/reef-node build-spec --chain /tmp/local-chain-spec-updated.json --disable-default-bootnode --raw > /tmp/local-chain-spec-raw.json
	@# Insert keys for Validator 1
	@echo "\n[6/9] Inserting keys for Validator 1..."
	@bash -c ' \
		V1_SEED=$$(cat /tmp/v1_seed.txt); \
		./target/release/reef-node key insert --base-path /tmp/validator1 \
			--chain=/tmp/local-chain-spec-raw.json \
			--scheme Sr25519 \
			--suri "$$V1_SEED//babe" \
			--key-type babe; \
		./target/release/reef-node key insert --base-path /tmp/validator1 \
			--chain=/tmp/local-chain-spec-raw.json \
			--scheme Ed25519 \
			--suri "$$V1_SEED//grandpa" \
			--key-type gran; \
		./target/release/reef-node key insert --base-path /tmp/validator1 \
			--chain=/tmp/local-chain-spec-raw.json \
			--scheme Sr25519 \
			--suri "$$V1_SEED//im_online" \
			--key-type imon; \
		./target/release/reef-node key insert --base-path /tmp/validator1 \
			--chain=/tmp/local-chain-spec-raw.json \
			--scheme Sr25519 \
			--suri "$$V1_SEED//authority_discovery" \
			--key-type audi \
	'
	@# Insert keys for Validator 2
	@echo "\n[7/9] Inserting keys for Validator 2..."
	@bash -c ' \
		V2_SEED=$$(cat /tmp/v2_seed.txt); \
		./target/release/reef-node key insert --base-path /tmp/validator2 \
			--chain=/tmp/local-chain-spec-raw.json \
			--scheme Sr25519 \
			--suri "$$V2_SEED//babe" \
			--key-type babe; \
		./target/release/reef-node key insert --base-path /tmp/validator2 \
			--chain=/tmp/local-chain-spec-raw.json \
			--scheme Ed25519 \
			--suri "$$V2_SEED//grandpa" \
			--key-type gran; \
		./target/release/reef-node key insert --base-path /tmp/validator2 \
			--chain=/tmp/local-chain-spec-raw.json \
			--scheme Sr25519 \
			--suri "$$V2_SEED//im_online" \
			--key-type imon; \
		./target/release/reef-node key insert --base-path /tmp/validator2 \
			--chain=/tmp/local-chain-spec-raw.json \
			--scheme Sr25519 \
			--suri "$$V2_SEED//authority_discovery" \
			--key-type audi \
	'
	@# Insert keys for Validator 3
	@echo "\n[8/9] Inserting keys for Validator 3..."
	@bash -c ' \
		V3_SEED=$$(cat /tmp/v3_seed.txt); \
		./target/release/reef-node key insert --base-path /tmp/validator3 \
			--chain=/tmp/local-chain-spec-raw.json \
			--scheme Sr25519 \
			--suri "$$V3_SEED//babe" \
			--key-type babe; \
		./target/release/reef-node key insert --base-path /tmp/validator3 \
			--chain=/tmp/local-chain-spec-raw.json \
			--scheme Ed25519 \
			--suri "$$V3_SEED//grandpa" \
			--key-type gran; \
		./target/release/reef-node key insert --base-path /tmp/validator3 \
			--chain=/tmp/local-chain-spec-raw.json \
			--scheme Sr25519 \
			--suri "$$V3_SEED//im_online" \
			--key-type imon; \
		./target/release/reef-node key insert --base-path /tmp/validator3 \
			--chain=/tmp/local-chain-spec-raw.json \
			--scheme Sr25519 \
			--suri "$$V3_SEED//authority_discovery" \
			--key-type audi \
	'
	@# Start the validator nodes
	@echo "\n[9/9] Starting bootnode and validator nodes..."
	@bash -c ' \
		V1_ADDR=$$(cat /tmp/v1_addr.txt); \
		V2_ADDR=$$(cat /tmp/v2_addr.txt); \
		V3_ADDR=$$(cat /tmp/v3_addr.txt); \
		BOOTNODE_NODE_KEY=$$(cat /tmp/bootnode_node_key.txt); \
		BOOTNODE_PEER_ID=$$(./target/release/reef-node key inspect-node-key --file /tmp/bootnode_node_key.txt 2>/dev/null | tail -n1); \
		echo "$$BOOTNODE_PEER_ID" > /tmp/bootnode_peer_id.txt; \
		echo ""; \
		echo "Bootnode will run on:"; \
		echo "  - P2P port: 30335"; \
		echo "  - Peer ID: $$BOOTNODE_PEER_ID"; \
		echo ""; \
		echo "Validator 1 ($$V1_ADDR) will run on:"; \
		echo "  - P2P port: 30333"; \
		echo "  - RPC port: 9944"; \
		echo "  - WebSocket: ws://127.0.0.1:9944"; \
		echo ""; \
		echo "Validator 2 ($$V2_ADDR) will run on:"; \
		echo "  - P2P port: 30334"; \
		echo "  - RPC port: 9945"; \
		echo "  - WebSocket: ws://127.0.0.1:9945"; \
		echo ""; \
		echo "Validator 3 ($$V3_ADDR) will run on:"; \
		echo "  - P2P port: 30336"; \
		echo "  - RPC port: 9946"; \
		echo "  - WebSocket: ws://127.0.0.1:9946" \
	'
	@echo "\n=========================================="
	@echo "Starting nodes in separate terminals..."
	@echo "Press Ctrl+C in each terminal to stop"
	@echo "=========================================="
	@# Start Bootnode in a new terminal
	@osascript -e 'tell app "Terminal" to do script "cd $(PWD) && ./target/release/reef-node \
		--base-path /tmp/bootnode \
		--chain /tmp/local-chain-spec-raw.json \
		--port 30335 \
		--node-key-file /tmp/bootnode_node_key.txt \
		--name Bootnode"' &
	@sleep 2
	@# Start Validator 1 in a new terminal
	@bash -c 'BOOTNODE_PEER_ID=$$(cat /tmp/bootnode_peer_id.txt); \
	osascript -e "tell app \"Terminal\" to do script \"cd $(PWD) && ./target/release/reef-node \
		--base-path /tmp/validator1 \
		--chain /tmp/local-chain-spec-raw.json \
		--port 30333 \
		--rpc-port 9944 \
		--node-key-file /tmp/v1_node_key.txt \
		--bootnodes /ip4/127.0.0.1/tcp/30335/p2p/$$BOOTNODE_PEER_ID \
		--validator \
		--rpc-cors all \
		--rpc-methods Unsafe \
		--rpc-external \
		--name Validator1Node\""' &
	@sleep 2
	@# Start Validator 2 in a new terminal
	@bash -c 'BOOTNODE_PEER_ID=$$(cat /tmp/bootnode_peer_id.txt); \
	osascript -e "tell app \"Terminal\" to do script \"cd $(PWD) && ./target/release/reef-node \
		--base-path /tmp/validator2 \
		--chain /tmp/local-chain-spec-raw.json \
		--port 30334 \
		--rpc-port 9945 \
		--node-key-file /tmp/v2_node_key.txt \
		--bootnodes /ip4/127.0.0.1/tcp/30335/p2p/$$BOOTNODE_PEER_ID \
		--validator \
		--rpc-cors all \
		--rpc-methods Unsafe \
		--rpc-external \
		--name Validator2Node\""' &
	@sleep 2
	@# Start Validator 3 in a new terminal
	@bash -c 'BOOTNODE_PEER_ID=$$(cat /tmp/bootnode_peer_id.txt); \
	osascript -e "tell app \"Terminal\" to do script \"cd $(PWD) && ./target/release/reef-node \
		--base-path /tmp/validator3 \
		--chain /tmp/local-chain-spec-raw.json \
		--port 30336 \
		--rpc-port 9946 \
		--node-key-file /tmp/v3_node_key.txt \
		--bootnodes /ip4/127.0.0.1/tcp/30335/p2p/$$BOOTNODE_PEER_ID \
		--validator \
		--rpc-cors all \
		--rpc-methods Unsafe \
		--rpc-external \
		--name Validator3Node\""' &
	@sleep 1
	@bash -c ' \
		V1_ADDR=$$(cat /tmp/v1_addr.txt); \
		V2_ADDR=$$(cat /tmp/v2_addr.txt); \
		V3_ADDR=$$(cat /tmp/v3_addr.txt); \
		BOOTNODE_PEER_ID=$$(cat /tmp/bootnode_peer_id.txt); \
		echo ""; \
		echo "✅ Local validator network started!"; \
		echo "Bootnode ($$BOOTNODE_PEER_ID): P2P 127.0.0.1:30335"; \
		echo "Validator 1 ($$V1_ADDR): ws://127.0.0.1:9944"; \
		echo "Validator 2 ($$V2_ADDR): ws://127.0.0.1:9945"; \
		echo "Validator 3 ($$V3_ADDR): ws://127.0.0.1:9946" \
	'
