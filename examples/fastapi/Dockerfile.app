FROM python:3.13-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y curl build-essential
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

COPY synapse-core /synapse-core
COPY synapse-py /synapse-py
WORKDIR /synapse-py
RUN pip install maturin && maturin build --release

FROM python:3.13-slim
WORKDIR /app
COPY --from=builder /synapse-py/target/wheels/*.whl .
RUN pip install *.whl && pip install fastapi uvicorn redis cachetools

COPY examples/fastapi/main.py .
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]
