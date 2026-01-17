use std::{
    error::Error,
    io::{self, ErrorKind},
};

use futures::{SinkExt, StreamExt};
use synapse_core::{CacheResponce, MAX_FRAME_LENGTH, decode_response, encode_get, encode_set};
use tokio::{net::UnixStream, sync::Mutex};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub struct SynapseClient {
    framed: Mutex<Framed<UnixStream, LengthDelimitedCodec>>,
}

impl SynapseClient {
    pub async fn new(socket_path: String) -> Result<Self, Box<dyn Error>> {
        let stream = UnixStream::connect(&socket_path).await?;
        let framed = Framed::new(
            stream,
            LengthDelimitedCodec::builder()
                .max_frame_length(MAX_FRAME_LENGTH)
                .new_codec(),
        );

        Ok(Self {
            framed: Mutex::new(framed),
        })
    }

    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
        let mut framed = self.framed.lock().await;
        let bytes = encode_get(key);

        framed.send(bytes).await?;

        match framed.next().await {
            Some(Ok(packet)) => {
                let response = decode_response(&packet)
                    .map_err(|err| io::Error::new(ErrorKind::InvalidData, err))?;
                match response {
                    CacheResponce::Hit(value) => Ok(Some(value)),
                    CacheResponce::Miss => Ok(None),
                    CacheResponce::Error(err) => Err(io::Error::new(ErrorKind::Other, err).into()),
                    _ => Err(io::Error::new(ErrorKind::Unsupported, "unexpected response").into()),
                }
            }
            Some(Err(err)) => Err(err.into()),
            None => Err(io::Error::new(ErrorKind::UnexpectedEof, "connection closed").into()),
        }
    }

    pub async fn set(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl_secs: Option<u64>,
    ) -> Result<bool, Box<dyn Error>> {
        let mut framed = self.framed.lock().await;
        let bytes = encode_set(key, &value, ttl_secs);

        framed.send(bytes).await?;
        match framed.next().await {
            Some(Ok(_)) => Ok(true),
            Some(Err(e)) => Err(io::Error::new(ErrorKind::UnexpectedEof, e.to_string()).into()),
            None => Ok(false),
        }
    }
}
