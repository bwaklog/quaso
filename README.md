# quaso 🥐

## Checklist for raft

- [x] Leader Election
- [x] Persistence
- [x] Log Replication
- [x] Leader Commits
- [x] State delivery mechanism (partially tested)

### Low Priority

- [ ] Dynamic cluster - plan for a network registry along with discovering
nodes in a network?
- [ ] Log Compaction

--- 

## Running the example KV store 

> The project can be fully deployed on a simple docker container, you can either pull the latest container from `bwaklog/quaso:latest` on docker hub or build it yourself. There is a `justfile` and a few sample machine configurations available at `./kv/data/` to get started

- (optional) create a docker network

```bash
sudo docker network create quaso-test
```

- building and running the container

```bash
# build a quaso container with the `latest` tag
just build latest

# run a machine with hostname ABC which has a config at ./kv/data/ABC.yml
just run latest ABC

# run a clean container for starting a client
just run_empty

# both the run commands allow a configurable docker network name at the end
just run latest ABC network_name
```

- execution within a container

```bash
# start a node
quaso-server -c config.yml

# on a client container start the client by connecting to a leader,
# the address is for the KV store and not the raft node address
quaso-cli -a ABC:6060
```

![Example KV cluster](https://imgur.com/ElduCMX.jpg)
