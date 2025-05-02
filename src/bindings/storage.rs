//! Node.js bindings for Soradyne storage
//!
//! This module provides bindings for the Soradyne storage module
//! to be used from Node.js via TypeScript.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use uuid::Uuid;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

use crate::storage::{
    StorageProvider, StorageConfig, LocalStorage,
    DissolutionManager, DissolutionMetadata,
    CrystallizationManager, CrystallizationMetadata
};

/// JavaScript representation of a storage configuration
#[napi(object)]
pub struct JsStorageConfig {
    pub base_dir: String,
    pub shard_count: u32,
    pub threshold: u32,
    pub encrypt: bool,
}

/// JavaScript representation of dissolution metadata
#[napi(object)]
pub struct JsDissolutionMetadata {
    pub id: String,
    pub shard_count: u32,
    pub threshold: u32,
    pub original_size: u32,
    pub encrypted: bool,
    pub shard_ids: Vec<String>,
}

/// JavaScript representation of crystallization metadata
#[napi(object)]
pub struct JsCrystallizationMetadata {
    pub id: String,
    pub dissolution_id: String,
    pub path: String,
    pub size: u32,
    pub encrypted: bool,
}

/// Wrapper for the local storage provider
#[napi]
pub struct JsLocalStorage {
    inner: Arc<Mutex<Option<LocalStorage>>>,
    runtime: Arc<Runtime>,
}

#[napi]
impl JsLocalStorage {
    /// Create a new local storage provider
    #[napi(constructor)]
    pub fn new(config: JsStorageConfig) -> Self {
        // Create a tokio runtime for async operations
        let runtime = Arc::new(Runtime::new().unwrap());
        
        let inner = Arc::new(Mutex::new(None));
        let inner_clone = inner.clone();
        let runtime_clone = runtime.clone();
        
        // Initialize the storage provider in the background
        std::thread::spawn(move || {
            let storage_config = StorageConfig {
                base_dir: PathBuf::from(config.base_dir),
                shard_count: config.shard_count as usize,
                threshold: config.threshold as usize,
                encrypt: config.encrypt,
            };
            
            let storage = runtime_clone.block_on(async {
                LocalStorage::new(storage_config).await.unwrap()
            });
            
            let mut guard = inner_clone.lock().unwrap();
            *guard = Some(storage);
        });
        
        Self {
            inner,
            runtime,
        }
    }
    
    /// Store data and return a unique identifier
    #[napi]
    pub fn store(&self, data: Buffer) -> Promise<String> {
        let inner = self.inner.clone();
        let runtime = self.runtime.clone();
        
        Promise::new(move |resolve, reject| {
            let inner_guard = inner.lock().unwrap();
            if let Some(ref storage) = *inner_guard {
                let storage_clone = storage.clone();
                let data_vec = data.to_vec();
                
                runtime.spawn(async move {
                    match storage_clone.store(data_vec).await {
                        Ok(id) => resolve(id.to_string()),
                        Err(e) => reject(Error::new(Status::GenericFailure, e.to_string())),
                    }
                });
            } else {
                reject(Error::new(Status::GenericFailure, "Storage not initialized"));
            }
        })
    }
    
    /// Retrieve data by its identifier
    #[napi]
    pub fn retrieve(&self, id: String) -> Promise<Buffer> {
        let inner = self.inner.clone();
        let runtime = self.runtime.clone();
        
        Promise::new(move |resolve, reject| {
            let id_uuid = match Uuid::parse_str(&id) {
                Ok(uuid) => uuid,
                Err(e) => {
                    reject(Error::new(Status::GenericFailure, e.to_string()));
                    return;
                }
            };
            
            let inner_guard = inner.lock().unwrap();
            if let Some(ref storage) = *inner_guard {
                let storage_clone = storage.clone();
                
                runtime.spawn(async move {
                    match storage_clone.retrieve(id_uuid).await {
                        Ok(data) => resolve(Buffer::from(data)),
                        Err(e) => reject(Error::new(Status::GenericFailure, e.to_string())),
                    }
                });
            } else {
                reject(Error::new(Status::GenericFailure, "Storage not initialized"));
            }
        })
    }
    
    /// Check if data exists
    #[napi]
    pub fn exists(&self, id: String) -> Promise<bool> {
        let inner = self.inner.clone();
        let runtime = self.runtime.clone();
        
        Promise::new(move |resolve, reject| {
            let id_uuid = match Uuid::parse_str(&id) {
                Ok(uuid) => uuid,
                Err(e) => {
                    reject(Error::new(Status::GenericFailure, e.to_string()));
                    return;
                }
            };
            
            let inner_guard = inner.lock().unwrap();
            if let Some(ref storage) = *inner_guard {
                let storage_clone = storage.clone();
                
                runtime.spawn(async move {
                    match storage_clone.exists(id_uuid).await {
                        Ok(exists) => resolve(exists),
                        Err(e) => reject(Error::new(Status::GenericFailure, e.to_string())),
                    }
                });
            } else {
                reject(Error::new(Status::GenericFailure, "Storage not initialized"));
            }
        })
    }
    
    /// Delete data by its identifier
    #[napi]
    pub fn delete(&self, id: String) -> Promise<bool> {
        let inner = self.inner.clone();
        let runtime = self.runtime.clone();
        
        Promise::new(move |resolve, reject| {
            let id_uuid = match Uuid::parse_str(&id) {
                Ok(uuid) => uuid,
                Err(e) => {
                    reject(Error::new(Status::GenericFailure, e.to_string()));
                    return;
                }
            };
            
            let inner_guard = inner.lock().unwrap();
            if let Some(ref storage) = *inner_guard {
                let storage_clone = storage.clone();
                
                runtime.spawn(async move {
                    match storage_clone.delete(id_uuid).await {
                        Ok(_) => resolve(true),
                        Err(e) => reject(Error::new(Status::GenericFailure, e.to_string())),
                    }
                });
            } else {
                reject(Error::new(Status::GenericFailure, "Storage not initialized"));
            }
        })
    }
    
    /// List all stored data identifiers
    #[napi]
    pub fn list(&self) -> Promise<Vec<String>> {
        let inner = self.inner.clone();
        let runtime = self.runtime.clone();
        
        Promise::new(move |resolve, reject| {
            let inner_guard = inner.lock().unwrap();
            if let Some(ref storage) = *inner_guard {
                let storage_clone = storage.clone();
                
                runtime.spawn(async move {
                    match storage_clone.list().await {
                        Ok(ids) => {
                            let string_ids = ids.iter().map(|id| id.to_string()).collect();
                            resolve(string_ids)
                        },
                        Err(e) => reject(Error::new(Status::GenericFailure, e.to_string())),
                    }
                });
            } else {
                reject(Error::new(Status::GenericFailure, "Storage not initialized"));
            }
        })
    }
}

/// Wrapper for the dissolution manager
#[napi]
pub struct JsDissolutionManager {
    inner: Arc<Mutex<DissolutionManager>>,
    runtime: Arc<Runtime>,
}

#[napi]
impl JsDissolutionManager {
    /// Create a new dissolution manager
    #[napi(constructor)]
    pub fn new(config: JsStorageConfig, storage: JsLocalStorage) -> Self {
        // Convert the JavaScript storage config to a Rust storage config
        let storage_config = StorageConfig {
            base_dir: PathBuf::from(config.base_dir),
            shard_count: config.shard_count as usize,
            threshold: config.threshold as usize,
            encrypt: config.encrypt,
        };
        
        // Create a storage provider from the local storage
        let storage_providers = vec![
            Box::new(storage.inner.lock().unwrap().clone().unwrap()) as Box<dyn StorageProvider>
        ];
        
        // Create the dissolution manager
        let dissolution_manager = DissolutionManager::new(storage_config, storage_providers);
        
        Self {
            inner: Arc::new(Mutex::new(dissolution_manager)),
            runtime: storage.runtime.clone(),
        }
    }
    
    /// Dissolve data across multiple storage providers
    #[napi]
    pub fn dissolve(&self, data: Buffer) -> Promise<JsDissolutionMetadata> {
        let inner = self.inner.clone();
        let runtime = self.runtime.clone();
        let data_vec = data.to_vec();
        
        Promise::new(move |resolve, reject| {
            let inner_guard = inner.lock().unwrap();
            
            runtime.spawn(async move {
                match inner_guard.dissolve(&data_vec).await {
                    Ok(metadata) => {
                        let js_metadata = JsDissolutionMetadata {
                            id: metadata.id.to_string(),
                            shard_count: metadata.shard_count as u32,
                            threshold: metadata.threshold as u32,
                            original_size: metadata.original_size as u32,
                            encrypted: metadata.encrypted,
                            shard_ids: metadata.shard_ids.iter().map(|id| id.to_string()).collect(),
                        };
                        resolve(js_metadata)
                    },
                    Err(e) => reject(Error::new(Status::GenericFailure, e.to_string())),
                }
            });
        })
    }
    
    /// Reconstruct data from shards
    #[napi]
    pub fn crystallize(&self, metadata: JsDissolutionMetadata) -> Promise<Buffer> {
        let inner = self.inner.clone();
        let runtime = self.runtime.clone();
        
        Promise::new(move |resolve, reject| {
            // Convert the JavaScript dissolution metadata to a Rust dissolution metadata
            let dissolution_metadata = match to_dissolution_metadata(metadata) {
                Ok(metadata) => metadata,
                Err(e) => {
                    reject(e);
                    return;
                }
            };
            
            let inner_guard = inner.lock().unwrap();
            
            runtime.spawn(async move {
                match inner_guard.crystallize(&dissolution_metadata).await {
                    Ok(data) => resolve(Buffer::from(data)),
                    Err(e) => reject(Error::new(Status::GenericFailure, e.to_string())),
                }
            });
        })
    }
}

/// Wrapper for the crystallization manager
#[napi]
pub struct JsCrystallizationManager {
    inner: Arc<Mutex<CrystallizationManager>>,
    runtime: Arc<Runtime>,
}

#[napi]
impl JsCrystallizationManager {
    /// Create a new crystallization manager
    #[napi(constructor)]
    pub fn new(config: JsStorageConfig, dissolution_manager: JsDissolutionManager) -> Self {
        // Convert the JavaScript storage config to a Rust storage config
        let storage_config = StorageConfig {
            base_dir: PathBuf::from(config.base_dir),
            shard_count: config.shard_count as usize,
            threshold: config.threshold as usize,
            encrypt: config.encrypt,
        };
        
        // Create the crystallization manager
        let crystallization_manager = CrystallizationManager::new(
            storage_config,
            dissolution_manager.inner.lock().unwrap().clone(),
        );
        
        Self {
            inner: Arc::new(Mutex::new(crystallization_manager)),
            runtime: dissolution_manager.runtime.clone(),
        }
    }
    
    /// Crystallize data from a dissolution
    #[napi]
    pub fn crystallize(&self, metadata: JsDissolutionMetadata) -> Promise<JsCrystallizationMetadata> {
        let inner = self.inner.clone();
        let runtime = self.runtime.clone();
        
        Promise::new(move |resolve, reject| {
            // Convert the JavaScript dissolution metadata to a Rust dissolution metadata
            let dissolution_metadata = match to_dissolution_metadata(metadata) {
                Ok(metadata) => metadata,
                Err(e) => {
                    reject(e);
                    return;
                }
            };
            
            let inner_guard = inner.lock().unwrap();
            
            runtime.spawn(async move {
                match inner_guard.crystallize(&dissolution_metadata).await {
                    Ok(metadata) => {
                        let js_metadata = JsCrystallizationMetadata {
                            id: metadata.id.to_string(),
                            dissolution_id: metadata.dissolution.id.to_string(),
                            path: metadata.path.to_string_lossy().to_string(),
                            size: metadata.size as u32,
                            encrypted: metadata.encrypted,
                        };
                        resolve(js_metadata)
                    },
                    Err(e) => reject(Error::new(Status::GenericFailure, e.to_string())),
                }
            });
        })
    }
    
    /// Retrieve crystallized data
    #[napi]
    pub fn retrieve(&self, metadata: JsCrystallizationMetadata) -> Promise<Buffer> {
        let inner = self.inner.clone();
        let runtime = self.runtime.clone();
        
        Promise::new(move |resolve, reject| {
            // Convert the JavaScript crystallization metadata to a Rust crystallization metadata
            let crystallization_metadata = match to_crystallization_metadata(metadata) {
                Ok(metadata) => metadata,
                Err(e) => {
                    reject(e);
                    return;
                }
            };
            
            let inner_guard = inner.lock().unwrap();
            
            runtime.spawn(async move {
                match inner_guard.retrieve(&crystallization_metadata).await {
                    Ok(data) => resolve(Buffer::from(data)),
                    Err(e) => reject(Error::new(Status::GenericFailure, e.to_string())),
                }
            });
        })
    }
}

/// Convert a JavaScript dissolution metadata to a Rust dissolution metadata
fn to_dissolution_metadata(js_metadata: JsDissolutionMetadata) -> Result<DissolutionMetadata> {
    let id = Uuid::parse_str(&js_metadata.id)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
    
    let mut shard_ids = Vec::new();
    for shard_id in js_metadata.shard_ids {
        let uuid = Uuid::parse_str(&shard_id)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        shard_ids.push(uuid);
    }
    
    Ok(DissolutionMetadata {
        id,
        shard_count: js_metadata.shard_count as usize,
        threshold: js_metadata.threshold as usize,
        original_size: js_metadata.original_size as usize,
        encrypted: js_metadata.encrypted,
        shard_ids,
    })
}

/// Convert a JavaScript crystallization metadata to a Rust crystallization metadata
fn to_crystallization_metadata(js_metadata: JsCrystallizationMetadata) -> Result<CrystallizationMetadata> {
    let id = Uuid::parse_str(&js_metadata.id)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
    
    let dissolution_id = Uuid::parse_str(&js_metadata.dissolution_id)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
    
    // Create a dummy dissolution metadata
    let dissolution = DissolutionMetadata {
        id: dissolution_id,
        shard_count: 0,
        threshold: 0,
        original_size: 0,
        encrypted: js_metadata.encrypted,
        shard_ids: Vec::new(),
    };
    
    Ok(CrystallizationMetadata {
        id,
        dissolution,
        path: PathBuf::from(js_metadata.path),
        size: js_metadata.size as usize,
        encrypted: js_metadata.encrypted,
    })
}
