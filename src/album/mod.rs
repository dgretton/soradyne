//! Album CRDT implementation for collaborative photo/video/audio albums
//! 
//! This module provides CRDT-based data structures for managing shared albums
//! with edit histories, comments, reactions, and media metadata.

pub mod crdt;
pub mod album;
pub mod operations;
pub mod sync;

pub use crdt::*;
pub use album::*;
pub use operations::*;
pub use sync::*;
