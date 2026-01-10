import os
import time
from fastapi import Response

import redis.asyncio as redis
import synapse_py
from fastapi import FastAPI

app = FastAPI()

SOCKET_PATH = "/tmp/synapse.sock"
REDIS_URL = os.getenv("REDIS_URL", "redis://redis:6379")
BIG_JSON_PATH = os.getenv(
    "BIG_JSON_PATH",
    os.path.join(os.path.dirname(__file__), "big_payload.json"),
)

client = None
redis_client = None


def load_big_json() -> str:
    if not os.path.exists(BIG_JSON_PATH):
        raise Exception("Data not found")

    with open(BIG_JSON_PATH, "r", encoding="utf-8") as f:
        return f.read()


@app.on_event("startup")
def startup_event():
    global client
    global redis_client
    if os.path.exists(SOCKET_PATH):
        client = synapse_py.SynapseClient(SOCKET_PATH)
    redis_client = redis.from_url(REDIS_URL, decode_responses=True)


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
