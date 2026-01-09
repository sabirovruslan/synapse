import asyncio
import synapse_py
import os
from fastapi import FastAPI

app = FastAPI()

SOCKET_PATH = "/tmp/synapse.sock"

client = None


@app.on_event("startup")
def startup_event():
    global client
    if os.path.exists(SOCKET_PATH):
        client = synapse_py.SynapseClient(SOCKET_PATH)


@app.get("/data/{key}")
async def get_data(key: str):
    if not client:
        return {"error": "Synapse not connected"}

    data = client.get(key)
    if data:
        return {"key": key, "value": data.decode(), "source": "L1/L2 Cache"}

    value = f"Data for {key}"
    await asyncio.sleep(0.300)
    client.set(key, value.encode(), 60)
    return {"key": key, "value": value, "source": "Slow DB"}
