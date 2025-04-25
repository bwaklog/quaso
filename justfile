all:
  -just -l

# clean all the .state files
clean_state:
	find . -type f -name "*.state" | xargs rm

# xxd for each .state file
inspect:
	find . -type f -name "*.state" -exec xxd {} \;

# build the quaso container
build tag:
  sudo docker build . --platform linux/arm64 -t bwaklog/quaso:latest

# run the quaso container
run tag hostname network='quaso-test':
  sudo docker run --rm -it \
    --platform linux/arm64 \
    --mount type=bind,src=./kv/data/{{hostname}}.yml,dst=/app/config.yml \
    --network {{network}} \
    --hostname {{hostname}} \
    --privileged \
    bwaklog/quaso:{{tag}}

# run a quaso container for a client (without config)
run_empty tag network='quaso-test':
  sudo docker run --rm -it \
    --platform linux/arm64 \
    --network {{network}} \
    --privileged \
    bwaklog/quaso:{{tag}}
