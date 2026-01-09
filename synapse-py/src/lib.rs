use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use synapse_core::{CacheCommand, CacheResponce};
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
            Ok::<_, PyErr>(Framed::new(stream, LengthDelimitedCodec::new()))
        })?;

        Ok(SynapseClient {
            runtime: rt,
            framed: Mutex::new(framed),
        })
    }

    fn get(&self, key: String) -> PyResult<Option<Vec<u8>>> {
        self.runtime.block_on(async {
            let mut framed = self.framed.lock().await;

            let cmd = CacheCommand::Get { key };
            let bytes = bincode::serialize(&cmd).unwrap();

            framed.send(Bytes::from(bytes)).await.unwrap();

            if let Some(Ok(packet)) = framed.next().await {
                match bincode::deserialize::<CacheResponce>(&packet).unwrap() {
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

            let cmd = CacheCommand::Set {
                key,
                value,
                ttl_secs,
            };
            let bytes = bincode::serialize(&cmd).unwrap();

            framed.send(Bytes::from(bytes)).await.unwrap();

            if let Some(Ok(_)) = framed.next().await {
                Ok(true)
            } else {
                Ok(false)
            }
        })
    }
}

#[pymodule]
fn synapse_py(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SynapseClient>()?;
    Ok(())
}
