use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::{SinkExt, StreamExt};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use synapse_core::{
    CacheResponce, MAX_FRAME_LENGTH, OP_GET, OP_SET, RES_ERR, RES_HIT, RES_MISS, RES_OK,
};
use tokio::{
    net::UnixStream,
    runtime::{Builder, Runtime},
    sync::Mutex,
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

fn encode_get(key: &str) -> Bytes {
    let mut out = BytesMut::new();
    out.put_u8(OP_GET);
    out.put_u32_le(key.len() as u32);
    out.extend_from_slice(key.as_bytes());
    out.freeze()
}

fn encode_set(key: &str, value: &[u8], ttl_secs: Option<u64>) -> Bytes {
    let mut out = BytesMut::new();
    out.put_u8(OP_SET);
    out.put_u32_le(key.len() as u32);
    out.put_u32_le(value.len() as u32);
    out.put_u64_le(ttl_secs.unwrap_or(0));
    out.extend_from_slice(key.as_bytes());
    out.extend_from_slice(value);
    out.freeze()
}

fn decode_response(mut buf: &[u8]) -> Result<CacheResponce, String> {
    let response = buf.get_u8();
    match response {
        RES_OK => Ok(CacheResponce::Ok),
        RES_MISS => Ok(CacheResponce::Miss),
        RES_HIT => {
            let len = buf.get_u32_le() as usize;
            let value = buf.copy_to_bytes(len).to_vec();
            Ok(CacheResponce::Hit(value))
        }
        RES_ERR => {
            let len = buf.get_u32_le() as usize;
            let msg =
                String::from_utf8(buf.copy_to_bytes(len).to_vec()).map_err(|_| "Bad err utf-8")?;
            Ok(CacheResponce::Error(msg))
        }
        _ => Err("Unknown result".into()),
    }
}

#[pyclass]
struct SynapseClient {
    runtime: Runtime,
    framed: Mutex<Framed<UnixStream, LengthDelimitedCodec>>,
}

#[pymethods]
impl SynapseClient {
    #[new]
    fn new(socket_path: String) -> PyResult<Self> {
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        let framed = rt.block_on(async {
            let stream = UnixStream::connect(&socket_path)
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            Ok::<_, PyErr>(Framed::new(
                stream,
                LengthDelimitedCodec::builder()
                    .max_frame_length(MAX_FRAME_LENGTH)
                    .new_codec(),
            ))
        })?;

        Ok(SynapseClient {
            runtime: rt,
            framed: Mutex::new(framed),
        })
    }

    fn get(&self, key: String) -> PyResult<Option<Vec<u8>>> {
        self.runtime.block_on(async {
            let mut framed = self.framed.lock().await;

            let bytes = encode_get(key.as_str());

            framed
                .send(bytes)
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

            if let Some(Ok(packet)) = framed.next().await {
                match decode_response(&packet)
                    .map_err(|e| PyRuntimeError::new_err(e.to_string()))?
                {
                    CacheResponce::Hit(val) => Ok(Some(val)),
                    CacheResponce::Miss => Ok(None),
                    CacheResponce::Error(e) => Err(PyRuntimeError::new_err(e)),
                    _ => Err(PyRuntimeError::new_err("Unexpected response")),
                }
            } else {
                Err(PyRuntimeError::new_err("Connection closed"))
            }
        })
    }

    fn set(&self, key: String, value: Vec<u8>, ttl_secs: Option<u64>) -> PyResult<bool> {
        self.runtime.block_on(async {
            let mut framed = self.framed.lock().await;

            let bytes = encode_set(key.as_str(), &value, ttl_secs);

            framed
                .send(bytes)
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

            match framed.next().await {
                Some(Ok(_)) => Ok(true),
                Some(Err(e)) => Err(PyRuntimeError::new_err(e.to_string())),
                None => Ok(false),
            }
        })
    }
}

#[pymodule]
fn synapse_py(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SynapseClient>()?;
    Ok(())
}
