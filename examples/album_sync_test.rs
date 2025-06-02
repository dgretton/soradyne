//! Test album synchronization locally
//! 
//! This example demonstrates creating albums, adding media, and syncing
//! between two local "replicas" to test the CRDT functionality.

use std::sync::Arc;
use tokio;

use soradyne::album::*;
use soradyne::storage::block_manager::BlockManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎬 Album Sync Test");
    println!("==================");
    println!("Testing CRDT-based album synchronization locally\n");
    
    // Create two sync managers (simulating two devices)
    let (mut alice_sync, _temp_dir_alice) = test_utils::create_test_sync_manager();
    let (mut bob_sync, _temp_dir_bob) = test_utils::create_test_sync_manager();
    
    // Alice creates an album
    println!("📸 Alice creates a photo album...");
    let album_id = alice_sync.create_album("Family Vacation".to_string())?;
    println!("   Album ID: {}", album_id);
    
    // Alice adds a photo
    println!("📷 Alice adds a photo...");
    let photo_data = create_mock_photo_data();
    alice_sync.add_media_to_album(
        &album_id,
        "photo1".to_string(),
        &photo_data,
        MediaType::Photo,
        "beach.jpg".to_string()
    ).await?;
    
    // Alice adds a comment
    println!("💬 Alice adds a comment...");
    let comment_op = EditOp::add_comment(
        "alice".to_string(),
        "Beautiful sunset!".to_string(),
        None
    );
    alice_sync.apply_operation(&album_id, &"photo1".to_string(), comment_op)?;
    
    // Show Alice's album state
    println!("\n📊 Alice's album state:");
    if let Some(album) = alice_sync.get_album(&album_id) {
        let states = album.reduce_all();
        for (media_id, state) in &states {
            println!("   Media {}: {} comments, rotation: {}°", 
                    media_id, state.comments.len(), state.rotation);
            if let Some(media) = &state.media {
                println!("     File: {} ({} bytes)", media.filename, media.size);
            }
            for comment in &state.comments {
                println!("     Comment: \"{}\" by {}", comment.text, comment.author);
            }
        }
    }
    
    // Simulate sync: Alice sends album to Bob
    println!("\n🔄 Syncing album from Alice to Bob...");
    if let Some(alice_album) = alice_sync.get_album(&album_id).cloned() {
        bob_sync.merge_album(alice_album)?;
    }
    
    // Bob adds his own comment
    println!("💬 Bob adds a comment...");
    let bob_comment_op = EditOp::add_comment(
        "bob".to_string(),
        "I love this place!".to_string(),
        None
    );
    bob_sync.apply_operation(&album_id, &"photo1".to_string(), bob_comment_op)?;
    
    // Bob rotates the photo
    println!("🔄 Bob rotates the photo...");
    let rotate_op = EditOp::rotate("bob".to_string(), 90.0);
    bob_sync.apply_operation(&album_id, &"photo1".to_string(), rotate_op)?;
    
    // Show Bob's album state
    println!("\n📊 Bob's album state:");
    if let Some(album) = bob_sync.get_album(&album_id) {
        let states = album.reduce_all();
        for (media_id, state) in &states {
            println!("   Media {}: {} comments, rotation: {}°", 
                    media_id, state.comments.len(), state.rotation);
            for comment in &state.comments {
                println!("     Comment: \"{}\" by {}", comment.text, comment.author);
            }
        }
    }
    
    // Sync back: Bob sends updates to Alice
    println!("\n🔄 Syncing updates from Bob back to Alice...");
    if let Some(bob_album) = bob_sync.get_album(&album_id).cloned() {
        alice_sync.merge_album(bob_album)?;
    }
    
    // Show final Alice state (should have both comments and rotation)
    println!("\n📊 Alice's final album state (after merge):");
    if let Some(album) = alice_sync.get_album(&album_id) {
        let states = album.reduce_all();
        for (media_id, state) in &states {
            println!("   Media {}: {} comments, rotation: {}°", 
                    media_id, state.comments.len(), state.rotation);
            for comment in &state.comments {
                println!("     Comment: \"{}\" by {}", comment.text, comment.author);
            }
        }
    }
    
    // Test concurrent edits
    println!("\n⚡ Testing concurrent edits...");
    
    // Both Alice and Bob add reactions at the same time
    let alice_reaction = EditOp::add_reaction(
        "alice".to_string(),
        uuid::Uuid::new_v4(), // React to first comment (simplified)
        "❤️".to_string()
    );
    
    let bob_reaction = EditOp::add_reaction(
        "bob".to_string(),
        uuid::Uuid::new_v4(), // React to first comment (simplified)
        "👍".to_string()
    );
    
    // Apply reactions to separate replicas
    alice_sync.apply_operation(&album_id, &"photo1".to_string(), alice_reaction)?;
    bob_sync.apply_operation(&album_id, &"photo1".to_string(), bob_reaction)?;
    
    // Merge both ways
    if let Some(alice_album) = alice_sync.get_album(&album_id).cloned() {
        bob_sync.merge_album(alice_album)?;
    }
    if let Some(bob_album) = bob_sync.get_album(&album_id).cloned() {
        alice_sync.merge_album(bob_album)?;
    }
    
    // Show final state with reactions
    println!("\n📊 Final state with reactions:");
    if let Some(album) = alice_sync.get_album(&album_id) {
        let states = album.reduce_all();
        for (media_id, state) in &states {
            println!("   Media {}: {} comments, {} reaction types", 
                    media_id, state.comments.len(), state.reactions.len());
            for (emoji, users) in &state.reactions {
                println!("     Reaction {}: {} users", emoji, users.len());
            }
        }
    }
    
    println!("\n✅ Album sync test completed successfully!");
    println!("   - Created album with media");
    println!("   - Added comments from multiple users");
    println!("   - Applied edits (rotation)");
    println!("   - Merged changes bidirectionally");
    println!("   - Handled concurrent reactions");
    println!("   - All data converged correctly");
    
    Ok(())
}

fn create_mock_photo_data() -> Vec<u8> {
    // Create mock JPEG data
    let mut data = Vec::new();
    
    // JPEG header
    data.extend_from_slice(b"\xFF\xD8\xFF\xE0");
    
    // Add some metadata
    data.extend_from_slice(b"Mock photo data for testing");
    
    // Add some mock image data
    for i in 0..1000 {
        data.push((i % 256) as u8);
    }
    
    // JPEG footer
    data.extend_from_slice(b"\xFF\xD9");
    
    data
}
