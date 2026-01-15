import os
import time
from fastapi import Response

from cachetools import LRUCache
import redis.asyncio as redis
import synapse_embedded_py
import synapse_embedded_async_py
import synapse_py
from fastapi import FastAPI

app = FastAPI()

SOCKET_PATH = "/tmp/synapse.sock"
SOCKET_PATH_2 = "/tmp/synapse2.sock"
REDIS_URL = os.getenv("REDIS_URL", "redis://redis:6379")
BIG_JSON_PATH = os.getenv(
    "BIG_JSON_PATH",
    os.path.join(os.path.dirname(__file__), "big_payload.json"),
)

client = None
client2 = None
redis_client = None
memory_cache = None
embedded_client = None
embedded_async_client = None


def load_big_json() -> str:
    if not os.path.exists(BIG_JSON_PATH):
        raise Exception("Data not found")

    with open(BIG_JSON_PATH, "r", encoding="utf-8") as f:
        return f.read()


@app.on_event("startup")
def startup_event():
    global client, client2, memory_cache, embedded_client
    global redis_client
    if os.path.exists(SOCKET_PATH):
        client = synapse_py.SynapseClient(SOCKET_PATH)
    if os.path.exists(SOCKET_PATH_2):
        client2 = synapse_py.SynapseClient(SOCKET_PATH_2)
    redis_client = redis.from_url(REDIS_URL, decode_responses=True)
    memory_cache = LRUCache(maxsize=10_000)
    embedded_client = synapse_embedded_py.SynapseEmbedded(10_000)
    embedded_async_client = synapse_embedded_async_py.SynapseEmbedded(10_000)


@app.on_event("shutdown")
async def shutdown_event():
    if redis_client:
        await redis_client.close()


@app.get("/synapse/{key}")
async def get_data(key: str):
    if not client:
        return {"error": "Synapse not connected"}

    start = time.perf_counter()
    data = client.get(key)
    if data:
        duration_ms = (time.perf_counter() - start) * 1000
        print(f"[timing] synapse get hit {duration_ms:.2f}ms key={key}")
        return Response(content=data, media_type="application/json")

    load_start = time.perf_counter()
    value = load_big_json()
    load_ms = (time.perf_counter() - load_start) * 1000
    client.set(key, value.encode(), None)
    duration_ms = (time.perf_counter() - start) * 1000
    print(
        f"[timing] synapse get miss {duration_ms:.2f}ms (load {load_ms:.2f}ms) key={key}"
    )
    return {"key": key, "source": "Set synapse"}


@app.get("/synapse2/{key}")
async def get_data_from_synapse2(key: str):
    if not client2:
        return {"error": "Synapse2 not connected"}

    start = time.perf_counter()
    data = client2.get(key)
    if data:
        duration_ms = (time.perf_counter() - start) * 1000
        print(f"[timing] synapse2 get hit {duration_ms:.2f}ms key={key}")
        return Response(content=data, media_type="application/json")

    load_start = time.perf_counter()
    value = load_big_json()
    load_ms = (time.perf_counter() - load_start) * 1000
    client2.set(key, value.encode(), None)
    duration_ms = (time.perf_counter() - start) * 1000
    print(
        f"[timing] synapse2 get miss {duration_ms:.2f}ms (load {load_ms:.2f}ms) key={key}"
    )
    return {"key": key, "source": "Set synapse2"}


@app.get("/redis/{key}")
async def get_redis_data(key: str):
    if not redis_client:
        return {"error": "Redis not connected"}

    start = time.perf_counter()
    data = await redis_client.get(key)
    if data:
        duration_ms = (time.perf_counter() - start) * 1000
        print(f"[timing] redis get hit {duration_ms:.2f}ms key={key}")
        return Response(content=data, media_type="application/json")

    load_start = time.perf_counter()
    value = load_big_json()
    load_ms = (time.perf_counter() - load_start) * 1000
    await redis_client.set(key, value)
    duration_ms = (time.perf_counter() - start) * 1000
    print(
        f"[timing] redis get miss {duration_ms:.2f}ms (load {load_ms:.2f}ms) key={key}"
    )

    return {"key": key, "source": "Set Redis"}


@app.get("/memory/{key}")
async def get_memory_data(key: str):
    if memory_cache is None:
        return {"error": "Memory cache not initialized"}

    start = time.perf_counter()
    data = memory_cache.get(key)
    if data:
        duration_ms = (time.perf_counter() - start) * 1000
        print(f"[timing] memory get hit {duration_ms:.2f}ms key={key}")
        return Response(content=data, media_type="application/json")

    load_start = time.perf_counter()
    value = load_big_json()
    load_ms = (time.perf_counter() - load_start) * 1000
    memory_cache[key] = value.encode()
    duration_ms = (time.perf_counter() - start) * 1000
    print(
        f"[timing] memory get miss {duration_ms:.2f}ms (load {load_ms:.2f}ms) key={key}"
    )
    return {"key": key, "source": "Set Memory"}


@app.get("/synapse-embedded/{key}")
async def get_data_from_synapse_embedded_py(key: str):
    if embedded_client is None:
        return {"error": "Synapse embedded cache not initialized"}

    start = time.perf_counter()
    data = embedded_client.get(key)
    if data:
        duration_ms = (time.perf_counter() - start) * 1000
        print(f"[timing] embedded get hit {duration_ms:.2f}ms key={key}")
        return Response(content=data, media_type="application/json")

    load_start = time.perf_counter()
    value = load_big_json()
    load_ms = (time.perf_counter() - load_start) * 1000
    embedded_client.set(key, value.encode(), None)
    duration_ms = (time.perf_counter() - start) * 1000
    print(
        f"[timing] embedded get miss {duration_ms:.2f}ms (load {load_ms:.2f}ms) key={key}"
    )
    return {"key": key, "source": "Set embedded"}


@app.get("/synapse-embedded-async/{key}")
async def get_data_from_synapse_embedded_py(key: str):
    if embedded_client is None:
        return {"error": "Synapse embedded async cache not initialized"}

    start = time.perf_counter()
    data = await embedded_async_client.get(key)
    if data:
        duration_ms = (time.perf_counter() - start) * 1000
        print(f"[timing] embedded async get hit {duration_ms:.2f}ms key={key}")
        return Response(content=data, media_type="application/json")

    load_start = time.perf_counter()
    value = load_big_json()
    load_ms = (time.perf_counter() - load_start) * 1000
    await embedded_async_client.set(key, value.encode(), None)
    duration_ms = (time.perf_counter() - start) * 1000
    print(
        f"[timing] embedded async get miss {duration_ms:.2f}ms (load {load_ms:.2f}ms) key={key}"
    )
    return {"key": key, "source": "Set embedded async"}


@app.get("/.well-known/appspecific/com.chrome.devtools.json")
async def chrome_devtools_probe():
    return Response(status_code=204)
