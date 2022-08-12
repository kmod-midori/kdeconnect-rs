use std::{fmt::Debug, path::PathBuf, sync::Arc};

use anyhow::Result;
use lru_cache::LruCache;
use tokio::sync::Mutex;

type Cache = LruCache<String, Arc<Vec<u8>>>;

lazy_static::lazy_static! {
    pub static ref PAYLOAD_CACHE: PayloadCache = {
        PayloadCache::new().expect("Failed to initialize payload cache")
    };
}

pub struct PayloadCache {
    cache: Mutex<Cache>,
    cache_path: PathBuf,
}

impl PayloadCache {
    pub fn new() -> Result<Self> {
        let cache_path = std::env::temp_dir().join("kdeconnect-rs");
        if !cache_path.exists() {
            std::fs::create_dir_all(&cache_path)?;
        }
        Ok(Self {
            cache: Mutex::new(LruCache::new(10)),
            cache_path,
        })
    }

    async fn get_internal(&self, cache: &mut Cache, name: &str) -> Result<Option<Arc<Vec<u8>>>> {
        if let Some(cached) = cache.get_mut(name) {
            return Ok(Some(Arc::clone(cached)));
        };

        let path = self.cache_path.join(name);
        match tokio::fs::read(&path).await {
            Ok(data) => {
                let a = Arc::new(data);
                cache.insert(name.to_string(), a.clone());
                Ok(Some(a))
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => Ok(None),
                _ => Err(e.into()),
            },
        }
    }

    pub async fn get(&self, name: &str) -> Result<Option<Arc<Vec<u8>>>> {
        let mut cache = self.cache.lock().await;
        self.get_internal(&mut cache, name).await
    }

    pub async fn get_path(&self, name: &str) -> Result<Option<PathBuf>> {
        let path = self.cache_path.join(name);

        match tokio::fs::metadata(&path).await {
            Ok(_) => Ok(Some(path)),
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => Ok(None),
                _ => Err(e.into()),
            },
        }
    }

    pub async fn put(&self, name: &str, data: Vec<u8>) -> Result<()> {
        let mut cache = self.cache.lock().await;

        if self.get_internal(&mut cache, name).await?.is_some() {
            return Ok(());
        }

        let data = Arc::new(data);
        cache.insert(name.to_string(), data.clone());

        let path = self.cache_path.join(name);
        tokio::fs::write(&path, data.as_slice()).await?;

        Ok(())
    }
}

impl Debug for PayloadCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlbumArtCache")
            .field("cache_path", &self.cache_path)
            .finish()
    }
}
