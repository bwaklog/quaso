all:
	cargo c

release:
	cargo build --release -p kv

run:
	./target/release/kv -c ./kv/sample.yml

# now ofcourse wasm wont work using threads :P
wasmtime_run:
	wasmtime --dir=. target/wasm32-wasip1/release/kv.wasm -c kv/sample.yml

release_wasm:
	cargo build --target wasm32-wasip1 --release -p kv
