use pyo3::{exceptions::PyRuntimeError, prelude::*};
use pyo3_async_runtimes::tokio::future_into_py;
use std::{sync::Arc};
use synapse_core::{CacheResponce, L1Cache};

#[pyclass]
struct SynapseEmbedded {
    cache: Arc<L1Cache>,
}

#[pymethods]
impl SynapseEmbedded {
    #[new]
    fn new(max_capacity: u64) -> PyResult<Self> {
        Ok(Self {
            cache: Arc::new(L1Cache::new(max_capacity)),
        })
    }

    fn get<'py>(&self, py: Python<'py>, key: String) -> PyResult<Bound<'py, PyAny>> {
        let cache = self.cache.clone();
        future_into_py(py, async move {
            match cache.get(&key).await {
                CacheResponce::Hit(value) => Ok(Some(value)),
                CacheResponce::Miss => Ok(None),
                CacheResponce::Error(err) => Err(PyRuntimeError::new_err(err.to_string())),
                CacheResponce::Ok => Ok(None),
            }
        })
    }

    fn set<'py>(&self, py: Python<'py>, key: String, value: Vec<u8>, ttl_secs: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let cache = self.cache.clone();
        future_into_py(py, async move {
            cache.set(key, value, ttl_secs).await;
            Ok(true)
        })
    }
}

#[pymodule]
fn synapse_embedded_py(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SynapseEmbedded>()?;
    Ok(())
}
