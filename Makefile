all:
	cargo c

clean_state:
	rm ./kv/tmp/*.state

inspect_state:
	xxd ./kv/tmp/raft_a.state
	xxd ./kv/tmp/raft_b.state
	xxd ./kv/tmp/raft_c.state

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
