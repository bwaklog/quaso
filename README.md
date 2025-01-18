# quaso

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

Example with a KV

1. Create `YAML` configuration files for each of your nodes, a sample one can be found at `./kv/sample.yml`
2. Build the KV Store 

```sh
make release
```

3. Run the server

```sh 
./quaso_server -c ./kv/sample.yml
```

4. Connect to a host with the client


```sh
./quaso_client -a grogu:6060
```

![Example KV cluster](https://imgur.com/ElduCMX.jpg)
