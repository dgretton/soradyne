//! Node.js bindings for Soradyne Self-Data Objects
//!
//! This module provides bindings for the Soradyne SDO module
//! to be used from Node.js via TypeScript.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use uuid::Uuid;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

use crate::sdo::SelfDataObject;
use crate::sdo::types::heartrate::{HeartRateSDO, HeartRateData};
use crate::sdo::types::chat::{ChatSDO, ChatConversation, ChatMessage};

/// JavaScript representation of an SDO type
#[napi]
pub enum JsSdoType {
    RealTime = 0,
    EventualConsistent = 1,
    Hybrid = 2,
}

/// JavaScript representation of an SDO access level
#[napi]
pub enum JsSdoAccess {
    Read = 0,
    Write = 1,
    Admin = 2,
}

/// JavaScript representation of SDO metadata
#[napi(object)]
pub struct JsSdoMetadata {
    pub id: String,
    pub name: String,
    pub sdo_type: JsSdoType,
    pub created_at: String,
    pub modified_at: String,
    pub owner_id: String,
    pub access: Vec<JsSdoAccess>,
}

/// Wrapper for the heart rate SDO
#[napi]
pub struct JsHeartRateSDO {
    inner: Arc<Mutex<HeartRateSDO>>,
    runtime: Arc<Runtime>,
}

#[napi]
impl JsHeartRateSDO {
    /// Create a new heart rate SDO
    #[napi(constructor)]
    pub fn new(name: String, owner_id: String) -> Result<Self> {
        // Create a tokio runtime for async operations
        let runtime = Arc::new(
            Runtime::new().map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
        );
        
        // Parse the owner ID
        let owner_uuid = Uuid::parse_str(&owner_id)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        // Create the heart rate SDO
        let sdo = HeartRateSDO::new(&name, owner_uuid);
        
        Ok(Self {
            inner: Arc::new(Mutex::new(sdo)),
            runtime,
        })
    }
    
    /// Get the metadata for this SDO
    #[napi]
    pub fn get_metadata(&self) -> Result<JsSdoMetadata> {
        let guard = self.inner.lock()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
            
        let metadata = guard.metadata();
        
        Ok(JsSdoMetadata {
            id: metadata.id.to_string(),
            name: metadata.name.clone(),
            sdo_type: match metadata.sdo_type {
                SDOType::RealTime => JsSdoType::RealTime,
                SDOType::EventualConsistent => JsSdoType::EventualConsistent,
                SDOType::Hybrid => JsSdoType::Hybrid,
            },
            created_at: metadata.created_at.to_rfc3339(),
            modified_at: metadata.modified_at.to_rfc3339(),
            owner_id: metadata.owner_id.to_string(),
            access: metadata.access.iter()
                .map(|(_, &access)| match access {
                    SDOAccess::Read => JsSdoAccess::Read,
                    SDOAccess::Write => JsSdoAccess::Write,
                    SDOAccess::Admin => JsSdoAccess::Admin,
                })
                .collect(),
        })
    }
    
    /// Get the current heart rate
    #[napi]
    pub fn get_heart_rate(&self) -> Result<Object> {
        let guard = self.inner.lock()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
            
        let heart_rate = guard.get_heart_rate()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        let env = napi::Env::get().unwrap();
        let mut obj = env.create_object()?;
        
        obj.set("bpm", heart_rate.bpm)?;
        if let Some(hrv) = heart_rate.hrv {
            obj.set("hrv", hrv)?;
        }
        obj.set("timestamp", heart_rate.timestamp.to_rfc3339())?;
        
        Ok(obj)
    }
    
    /// Update the heart rate
    #[napi]
    pub fn update_heart_rate(&self, identity_id: String, bpm: f64, hrv: Option<f64>) -> Result<()> {
        let identity_uuid = Uuid::parse_str(&identity_id)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        let guard = self.inner.lock()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
            
        guard.update_heart_rate(identity_uuid, bpm as f32, hrv.map(|v| v as f32))
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        Ok(())
    }
    
    /// Subscribe to heart rate updates
    #[napi]
    pub fn subscribe(&self, callback: JsFunction) -> Result<String> {
        let guard = self.inner.lock()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
            
        let tsfn: ThreadsafeFunction<(), ErrorStrategy::Fatal> = 
            callback.create_threadsafe_function(0, |_ctx| Ok(vec![]))?;
        
        let subscription_id = self.runtime.block_on(async {
            guard.subscribe(Box::new(move || {
                let tsfn = tsfn.clone();
                tsfn.call((), ThreadsafeFunctionCallMode::Blocking);
            }))
        }).map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        Ok(subscription_id.to_string())
    }
    
    /// Unsubscribe from heart rate updates
    #[napi]
    pub fn unsubscribe(&self, subscription_id: String) -> Result<()> {
        let subscription_uuid = Uuid::parse_str(&subscription_id)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        let guard = self.inner.lock()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
            
        self.runtime.block_on(async {
            guard.unsubscribe(subscription_uuid)
        }).map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        Ok(())
    }
}

/// Wrapper for the chat SDO
#[napi]
pub struct JsChatSDO {
    inner: Arc<Mutex<ChatSDO>>,
    runtime: Arc<Runtime>,
}

#[napi]
impl JsChatSDO {
    /// Create a new chat SDO
    #[napi(constructor)]
    pub fn new(name: String, owner_id: String) -> Result<Self> {
        // Create a tokio runtime for async operations
        let runtime = Arc::new(
            Runtime::new().map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
        );
        
        // Parse the owner ID
        let owner_uuid = Uuid::parse_str(&owner_id)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        // Create the chat SDO
        let sdo = ChatSDO::new(&name, owner_uuid);
        
        Ok(Self {
            inner: Arc::new(Mutex::new(sdo)),
            runtime,
        })
    }
    
    /// Get the metadata for this SDO
    #[napi]
    pub fn get_metadata(&self) -> Result<JsSdoMetadata> {
        let guard = self.inner.lock()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
            
        let metadata = guard.metadata();
        
        Ok(JsSdoMetadata {
            id: metadata.id.to_string(),
            name: metadata.name.clone(),
            sdo_type: match metadata.sdo_type {
                SDOType::RealTime => JsSdoType::RealTime,
                SDOType::EventualConsistent => JsSdoType::EventualConsistent,
                SDOType::Hybrid => JsSdoType::Hybrid,
            },
            created_at: metadata.created_at.to_rfc3339(),
            modified_at: metadata.modified_at.to_rfc3339(),
            owner_id: metadata.owner_id.to_string(),
            access: metadata.access.iter()
                .map(|(_, &access)| match access {
                    SDOAccess::Read => JsSdoAccess::Read,
                    SDOAccess::Write => JsSdoAccess::Write,
                    SDOAccess::Admin => JsSdoAccess::Admin,
                })
                .collect(),
        })
    }
    
    /// Get the conversation
    #[napi]
    pub fn get_conversation(&self) -> Result<Object> {
        let guard = self.inner.lock()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
            
        let conversation = guard.get_conversation()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        let env = napi::Env::get().unwrap();
        let mut obj = env.create_object()?;
        
        // Convert messages to JavaScript array
        let messages = env.create_array(conversation.messages.len() as u32)?;
        for (i, message) in conversation.messages.iter().enumerate() {
            let mut msg_obj = env.create_object()?;
            msg_obj.set("id", message.id.to_string())?;
            msg_obj.set("sender_id", message.sender_id.to_string())?;
            msg_obj.set("content", message.content.clone())?;
            msg_obj.set("timestamp", message.timestamp.to_rfc3339())?;
            msg_obj.set("edited", message.edited)?;
            msg_obj.set("deleted", message.deleted)?;
            
            // Convert reactions to JavaScript array
            let reactions = env.create_array(message.reactions.len() as u32)?;
            for (j, reaction) in message.reactions.iter().enumerate() {
                let mut reaction_obj = env.create_object()?;
                reaction_obj.set("identity_id", reaction.identity_id.to_string())?;
                reaction_obj.set("reaction", reaction.reaction.clone())?;
                reactions.set(j as u32, reaction_obj)?;
            }
            msg_obj.set("reactions", reactions)?;
            
            messages.set(i as u32, msg_obj)?;
        }
        obj.set("messages", messages)?;
        
        // Convert participants to JavaScript array
        let participants = env.create_array(conversation.participants.len() as u32)?;
        for (i, &participant_id) in conversation.participants.iter().enumerate() {
            participants.set(i as u32, participant_id.to_string())?;
        }
        obj.set("participants", participants)?;
        
        obj.set("is_group", conversation.is_group)?;
        if let Some(ref name) = conversation.name {
            obj.set("name", name.clone())?;
        }
        
        Ok(obj)
    }
    
    /// Add a message to the conversation
    #[napi]
    pub fn add_message(&self, identity_id: String, content: String) -> Result<String> {
        let identity_uuid = Uuid::parse_str(&identity_id)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        let mut guard = self.inner.lock()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
            
        let message_id = self.runtime.block_on(async {
            guard.add_message(identity_uuid, content)
        }).map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        Ok(message_id.to_string())
    }
    
    /// Subscribe to conversation updates
    #[napi]
    pub fn subscribe(&self, callback: JsFunction) -> Result<String> {
        let guard = self.inner.lock()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
            
        let tsfn: ThreadsafeFunction<(), ErrorStrategy::Fatal> = 
            callback.create_threadsafe_function(0, |_ctx| Ok(vec![]))?;
        
        let subscription_id = self.runtime.block_on(async {
            guard.subscribe(Box::new(move || {
                let tsfn = tsfn.clone();
                tsfn.call((), ThreadsafeFunctionCallMode::Blocking);
            }))
        }).map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        Ok(subscription_id.to_string())
    }
    
    /// Unsubscribe from conversation updates
    #[napi]
    pub fn unsubscribe(&self, subscription_id: String) -> Result<()> {
        let subscription_uuid = Uuid::parse_str(&subscription_id)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        let guard = self.inner.lock()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
            
        self.runtime.block_on(async {
            guard.unsubscribe(subscription_uuid)
        }).map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        
        Ok(())
    }
}
