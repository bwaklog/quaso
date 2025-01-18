all:
	cargo c

clean_state:
	find . -type f -name "*.state" | xargs rm

inspect:
	find . -type f -name "*.state" -exec xxd {} \;

run:
	./target/release/kv -c ./kv/sample.yml

release:
	cargo b -p kv --release
	cargo b -p kv --bin client --release
	cp ./target/release/kv quaso_server
	cp ./target/release/client quaso_client

clean:
	rm quaso_server
	rm quaso_client

# now ofcourse wasm wont work using threads :P
wasmtime_run:
	wasmtime --dir=. target/wasm32-wasip1/release/kv.wasm -c kv/sample.yml

release_wasm:
	cargo build --target wasm32-wasip1 --release -p kv
