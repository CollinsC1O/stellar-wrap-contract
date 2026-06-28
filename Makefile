build:
	soroban contract build

test:
	cargo test

fmt:
	cargo fmt

clean:
	cargo clean

docker-build:
	docker build -t stellar-wrap-contract .

docker-build-verify:
	docker build -t stellar-wrap-contract-verify .
	docker run --rm stellar-wrap-contract-verify sha256sum /contract/target/wasm32-unknown-unknown/release/stellar_wrap_contract.wasm