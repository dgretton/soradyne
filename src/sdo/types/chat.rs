//! Chat Self-Data Object for Soradyne
//!
//! This module implements an eventually consistent SDO for chat conversations.

use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::collections::HashSet;

use crate::sdo::{EventualSDO, SDOType};
use crate::sdo::base::SDOError;

/// A single message in a chat conversation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Unique identifier for this message
    pub id: Uuid,
    
    /// The identity that sent this message
    pub sender_id: Uuid,
    
    /// The content of the message
    pub content: String,
    
    /// When this message was sent
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Whether this message has been edited
    pub edited: bool,
    
    /// Whether this message has been deleted
    pub deleted: bool,
    
    /// Reactions to this message
    pub reactions: Vec<ChatReaction>,
}

/// A reaction to a chat message
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatReaction {
    /// The identity that added this reaction
    pub identity_id: Uuid,
    
    /// The reaction (e.g., an emoji)
    pub reaction: String,
}

/// A chat conversation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatConversation {
    /// Messages in this conversation
    pub messages: Vec<ChatMessage>,
    
    /// Participants in this conversation
    pub participants: HashSet<Uuid>,
    
    /// Whether this conversation is a group chat
    pub is_group: bool,
    
    /// The name of this conversation (for group chats)
    pub name: Option<String>,
}

impl ChatConversation {
    /// Create a new chat conversation
    pub fn new(owner_id: Uuid, name: Option<String>) -> Self {
        let mut participants = HashSet::new();
        participants.insert(owner_id);
        
        Self {
            messages: Vec::new(),
            participants,
            is_group: false,
            name,
        }
    }
    
    /// Add a message to this conversation
    pub fn add_message(&mut self, sender_id: Uuid, content: String) -> Result<Uuid, SDOError> {
        // Check if the sender is a participant
        if !self.participants.contains(&sender_id) {
            return Err(SDOError::AccessDenied);
        }
        
        // Create a new message
        let message_id = Uuid::new_v4();
        let message = ChatMessage {
            id: message_id,
            sender_id,
            content,
            timestamp: chrono::Utc::now(),
            edited: false,
            deleted: false,
            reactions: Vec::new(),
        };
        
        // Add the message to the conversation
        self.messages.push(message);
        
        Ok(message_id)
    }
    
    /// Edit a message
    pub fn edit_message(&mut self, sender_id: Uuid, message_id: Uuid, new_content: String) -> Result<(), SDOError> {
        // Find the message
        let message = self.messages.iter_mut()
            .find(|m| m.id == message_id)
            .ok_or_else(|| SDOError::NotFound(message_id))?;
        
        // Check if the sender is the original sender
        if message.sender_id != sender_id {
            return Err(SDOError::AccessDenied);
        }
        
        // Check if the message has been deleted
        if message.deleted {
            return Err(SDOError::InvalidOperation("Cannot edit a deleted message".into()));
        }
        
        // Update the message
        message.content = new_content;
        message.edited = true;
        
        Ok(())
    }
    
    /// Delete a message
    pub fn delete_message(&mut self, sender_id: Uuid, message_id: Uuid) -> Result<(), SDOError> {
        // Find the message
        let message = self.messages.iter_mut()
            .find(|m| m.id == message_id)
            .ok_or_else(|| SDOError::NotFound(message_id))?;
        
        // Check if the sender is the original sender
        if message.sender_id != sender_id {
            return Err(SDOError::AccessDenied);
        }
        
        // Mark the message as deleted
        message.deleted = true;
        
        Ok(())
    }
    
    /// Add a reaction to a message
    pub fn add_reaction(&mut self, identity_id: Uuid, message_id: Uuid, reaction: String) -> Result<(), SDOError> {
        // Check if the identity is a participant
        if !self.participants.contains(&identity_id) {
            return Err(SDOError::AccessDenied);
        }
        
        // Find the message
        let message = self.messages.iter_mut()
            .find(|m| m.id == message_id)
            .ok_or_else(|| SDOError::NotFound(message_id))?;
        
        // Check if the message has been deleted
        if message.deleted {
            return Err(SDOError::InvalidOperation("Cannot react to a deleted message".into()));
        }
        
        // Check if the identity has already added this reaction
        if message.reactions.iter().any(|r| r.identity_id == identity_id && r.reaction == reaction) {
            return Ok(());
        }
        
        // Add the reaction
        message.reactions.push(ChatReaction {
            identity_id,
            reaction,
        });
        
        Ok(())
    }
    
    /// Remove a reaction from a message
    pub fn remove_reaction(&mut self, identity_id: Uuid, message_id: Uuid, reaction: String) -> Result<(), SDOError> {
        // Find the message
        let message = self.messages.iter_mut()
            .find(|m| m.id == message_id)
            .ok_or_else(|| SDOError::NotFound(message_id))?;
        
        // Find the reaction
        let index = message.reactions.iter()
            .position(|r| r.identity_id == identity_id && r.reaction == reaction)
            .ok_or_else(|| SDOError::NotFound(message_id))?;
        
        // Remove the reaction
        message.reactions.remove(index);
        
        Ok(())
    }
    
    /// Add a participant to this conversation
    pub fn add_participant(&mut self, identity_id: Uuid) -> Result<(), SDOError> {
        self.participants.insert(identity_id);
        Ok(())
    }
    
    /// Remove a participant from this conversation
    pub fn remove_participant(&mut self, identity_id: Uuid) -> Result<(), SDOError> {
        self.participants.remove(&identity_id);
        Ok(())
    }
    
    /// Set whether this conversation is a group chat
    pub fn set_group(&mut self, is_group: bool) -> Result<(), SDOError> {
        self.is_group = is_group;
        Ok(())
    }
    
    /// Set the name of this conversation
    pub fn set_name(&mut self, name: Option<String>) -> Result<(), SDOError> {
        self.name = name;
        Ok(())
    }
}

/// Chat Self-Data Object
///
/// This SDO is used to share chat conversations between devices.
pub type ChatSDO = EventualSDO<ChatConversation>;

impl ChatSDO {
    /// Create a new chat SDO
    pub fn new(name: &str, owner_id: Uuid) -> Self {
        // Create an initial empty conversation
        let initial_data = ChatConversation::new(owner_id, Some(name.to_string()));
        
        // Create the SDO
        EventualSDO::create(name, owner_id, initial_data)
    }
    
    /// Get the conversation
    pub fn get_conversation(&self) -> Result<ChatConversation, SDOError> {
        self.get_value()
    }
    
    /// Add a message to the conversation
    pub async fn add_message(&mut self, identity_id: Uuid, content: String) -> Result<Uuid, SDOError> {
        // Get the current conversation
        let mut conversation = self.get_value()?;
        
        // Add the message
        let message_id = conversation.add_message(identity_id, content)?;
        
        // Update the SDO
        self.apply_change(identity_id, conversation, None)?;
        
        Ok(message_id)
    }
    
    /// Edit a message in the conversation
    pub async fn edit_message(&mut self, identity_id: Uuid, message_id: Uuid, new_content: String) -> Result<(), SDOError> {
        // Get the current conversation
        let mut conversation = self.get_value()?;
        
        // Edit the message
        conversation.edit_message(identity_id, message_id, new_content)?;
        
        // Update the SDO
        self.apply_change(identity_id, conversation, None)?;
        
        Ok(())
    }
    
    /// Delete a message in the conversation
    pub async fn delete_message(&mut self, identity_id: Uuid, message_id: Uuid) -> Result<(), SDOError> {
        // Get the current conversation
        let mut conversation = self.get_value()?;
        
        // Delete the message
        conversation.delete_message(identity_id, message_id)?;
        
        // Update the SDO
        self.apply_change(identity_id, conversation, None)?;
        
        Ok(())
    }
    
    /// Add a reaction to a message
    pub async fn add_reaction(&mut self, identity_id: Uuid, message_id: Uuid, reaction: String) -> Result<(), SDOError> {
        // Get the current conversation
        let mut conversation = self.get_value()?;
        
        // Add the reaction
        conversation.add_reaction(identity_id, message_id, reaction)?;
        
        // Update the SDO
        self.apply_change(identity_id, conversation, None)?;
        
        Ok(())
    }
    
    /// Remove a reaction from a message
    pub async fn remove_reaction(&mut self, identity_id: Uuid, message_id: Uuid, reaction: String) -> Result<(), SDOError> {
        // Get the current conversation
        let mut conversation = self.get_value()?;
        
        // Remove the reaction
        conversation.remove_reaction(identity_id, message_id, reaction)?;
        
        // Update the SDO
        self.apply_change(identity_id, conversation, None)?;
        
        Ok(())
    }
    
    /// Add a participant to the conversation
    pub async fn add_participant(&mut self, identity_id: Uuid, participant_id: Uuid) -> Result<(), SDOError> {
        // Get the current conversation
        let mut conversation = self.get_value()?;
        
        // Add the participant
        conversation.add_participant(participant_id)?;
        
        // Update the SDO
        self.apply_change(identity_id, conversation, None)?;
        
        Ok(())
    }
    
    /// Remove a participant from the conversation
    pub async fn remove_participant(&mut self, identity_id: Uuid, participant_id: Uuid) -> Result<(), SDOError> {
        // Get the current conversation
        let mut conversation = self.get_value()?;
        
        // Remove the participant
        conversation.remove_participant(participant_id)?;
        
        // Update the SDO
        self.apply_change(identity_id, conversation, None)?;
        
        Ok(())
    }
    
    /// Set whether this conversation is a group chat
    pub async fn set_group(&mut self, identity_id: Uuid, is_group: bool) -> Result<(), SDOError> {
        // Get the current conversation
        let mut conversation = self.get_value()?;
        
        // Set the group flag
        conversation.set_group(is_group)?;
        
        // Update the SDO
        self.apply_change(identity_id, conversation, None)?;
        
        Ok(())
    }
    
    /// Set the name of this conversation
    pub async fn set_name(&mut self, identity_id: Uuid, name: Option<String>) -> Result<(), SDOError> {
        // Get the current conversation
        let mut conversation = self.get_value()?;
        
        // Set the name
        conversation.set_name(name)?;
        
        // Update the SDO
        self.apply_change(identity_id, conversation, None)?;
        
        Ok(())
    }
}
