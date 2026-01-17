use futures::{SinkExt, StreamExt};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use synapse_core::{CacheResponce, MAX_FRAME_LENGTH, decode_response, encode_get, encode_set};
use tokio::{
    net::UnixStream,
    runtime::{Builder, Runtime},
    sync::Mutex,
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

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
