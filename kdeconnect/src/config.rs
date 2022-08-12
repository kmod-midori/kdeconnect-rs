use std::{fs::File, io::BufReader, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct EncodedConfig {
    uuid: String,
    tls_key: String,
    tls_cert: String,
}

impl From<&Config> for EncodedConfig {
    fn from(config: &Config) -> Self {
        Self {
            uuid: config.uuid.clone(),
            tls_key: base64::encode(&config.tls_key),
            tls_cert: base64::encode(&config.tls_cert),
        }
    }
}

#[derive(Debug)]
pub struct Config {
    pub uuid: String,
    pub tls_key: Vec<u8>,
    pub tls_cert: Vec<u8>,
}

impl Config {
    /// Loads config from a file, or creates a new one if it doesn't exist.
    pub fn init_or_load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            Self::load(path)
        } else {
            let r = Self::init()?;
            r.save(path)?;
            Ok(r)
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let f = BufReader::new(File::open(path)?);
        let config: EncodedConfig = serde_json::from_reader(f)?;
        Self::try_from(config)
    }

    /// Initialize new UUID and certificates.
    pub fn init() -> Result<Self> {
        let uuid = uuid::Uuid::new_v4().to_string();

        let (tls_cert, tls_key) = crate::tls::generate_certs(&uuid)?;

        Ok(Self {
            uuid,
            tls_key,
            tls_cert,
        })
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let config = EncodedConfig::from(self);
        let f = File::create(path)?;
        serde_json::to_writer(f, &config)?;
        Ok(())
    }
}

impl TryFrom<EncodedConfig> for Config {
    type Error = anyhow::Error;

    fn try_from(encoded: EncodedConfig) -> Result<Self, Self::Error> {
        let tls_key = base64::decode(&encoded.tls_key)?;
        let tls_cert = base64::decode(&encoded.tls_cert)?;
        Ok(Self {
            uuid: encoded.uuid,
            tls_key,
            tls_cert,
        })
    }
}
