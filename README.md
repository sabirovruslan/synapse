# Synapse

Experimental local cache over UDS with Redis synchronization to reduce cache misses and improve performance. The goal is a fast in-process/localhost L1 with a lightweight binary protocol, plus optional Redis pub/sub to keep multiple local caches warm.

## What is here
- `synapse-core/`: core cache types and protocol constants.
- `synapse-server/`: UDS server with optional Redis sync (gRPC stub exists but is not implemented yet).
- `synapse-py/`: Python client that talks to the UDS server.
- `synapse-embedded-py/`: Python embedded cache (sync).
- `synapse-embedded-async-py/`: Python embedded cache (async).
- `examples/fastapi/`: FastAPI demo comparing Synapse, Redis, and in-process LRU.

## How it works (high level)
- Clients send `GET`/`SET` over a Unix Domain Socket using a length-delimited binary frame.
- The server holds an in-memory `moka` cache (L1).
- On `SET`, the server writes to Redis (if configured) and publishes a cache-update message.
- Other Synapse servers subscribe to updates and fetch the value from Redis to warm their local L1.

## Run the server
```bash
cargo run -p synapse-server
```

By default it listens on `/tmp/synapse.sock`.

## Environment variables
- `SYNAPSE_SOCKET_PATH`: UDS path (default: `/tmp/synapse.sock`).
- `SYNAPSE_REDIS_URL`: Redis connection URL; if unset, Redis sync is disabled.
- `SYNAPSE_REDIS_PREFIX`: Key prefix (default: `synapse:cache:`).
- `SYNAPSE_REDIS_CHANNEL`: Pub/sub channel (default: `synapse:cache_updates`).

## Python clients
UDS client (talks to the server):
```python
import synapse_py

client = synapse_py.SynapseClient("/tmp/synapse.sock")
client.set("alpha", b"hello", None)
print(client.get("alpha"))
```

Embedded cache (no server required):
```python
import synapse_embedded_py

cache = synapse_embedded_py.SynapseEmbedded(10_000)
cache.set("alpha", b"hello", None)
print(cache.get("alpha"))
```

Async embedded cache:
```python
import synapse_embedded_async_py

cache = synapse_embedded_async_py.SynapseEmbedded(10_000)
val = await cache.get("alpha")
```

## FastAPI example
Docker compose spins up two Synapse servers, Redis, and a demo app:
```bash
cd examples/fastapi
docker compose up --build
```

Then hit:
- `http://localhost:8000/synapse/<key>`
- `http://localhost:8000/synapse2/<key>`
- `http://localhost:8000/redis/<key>`
- `http://localhost:8000/memory/<key>`
- `http://localhost:8000/synapse-embedded/<key>`
- `http://localhost:8000/synapse-embedded-async/<key>`

## Development
```bash
cargo build
cargo test
cargo fmt
cargo clippy --all-targets --all-features
```
