use pyo3::{exceptions::PyRuntimeError, prelude::*};
use synapse_core::{CacheResponce, L1Cache};
use tokio::runtime::{Builder, Runtime};

#[pyclass]
struct SynapseEmbedded {
    cache: L1Cache,
    runtime: Runtime,
}

#[pymethods]
impl SynapseEmbedded {
    #[new]
    fn new(max_capacity: u64) -> PyResult<Self> {
        let runtime = Builder::new_current_thread()
            .enable_time()
            .build()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        let cache = L1Cache::new(max_capacity);

        Ok(Self { cache, runtime })
    }

    fn get(&self, key: String) -> PyResult<Option<Vec<u8>>> {
        self.runtime.block_on(async {
            match self.cache.get(&key).await {
                CacheResponce::Hit(value) => Ok(Some(value)),
                CacheResponce::Miss => Ok(None),
                CacheResponce::Error(err) => Err(PyRuntimeError::new_err(err.to_string())),
                CacheResponce::Ok => Ok(None),
            }
        })
    }

    fn set(&self, key: String, value: Vec<u8>, ttl_secs: Option<u64>) -> PyResult<bool> {
        self.runtime.block_on(async {
            self.cache.set(key, value, ttl_secs).await;
            Ok(true)
        })
    }
}

#[pymodule]
fn synapse_embedded_py(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SynapseEmbedded>()?;
    Ok(())
}
