//! Test the media renderer with various edits and resolutions
//! 
//! This example demonstrates rendering media with crops, rotations, and markup
//! at different resolutions to verify the rendering pipeline works correctly.

use tokio;
use soradyne::album::*;
use soradyne::album::renderer::*;
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎨 Media Renderer Test");
    println!("======================");
    println!("Testing multi-resolution rendering with edits and markup\n");
    
    // Create a test media state with various edits
    let mut media_state = MediaState::default();
    
    // Add a crop
    media_state.crop = Some(CropData {
        left: 0.1,
        top: 0.1,
        right: 0.9,
        bottom: 0.9,
    });
    
    // Add rotation
    media_state.rotation = 90.0;
    
    // Add some markup
    let circle_data = serde_json::to_value(CircleMarkup {
        center_x: 0.3,
        center_y: 0.3,
        radius: 0.1,
        color: [255, 0, 0, 255], // Red
        filled: false,
        stroke_width: 0.02,
    })?;
    
    media_state.markup.push(MarkupElement {
        id: uuid::Uuid::new_v4(),
        markup_type: MarkupType::Circle,
        data: circle_data,
        author: "test".to_string(),
        timestamp: 1,
    });
    
    let rect_data = serde_json::to_value(RectangleMarkup {
        x: 0.6,
        y: 0.6,
        width: 0.3,
        height: 0.2,
        color: [0, 255, 0, 128], // Semi-transparent green
        filled: true,
        stroke_width: 0.01,
    })?;
    
    media_state.markup.push(MarkupElement {
        id: uuid::Uuid::new_v4(),
        markup_type: MarkupType::Rectangle,
        data: rect_data,
        author: "test".to_string(),
        timestamp: 2,
    });
    
    // Create a simple test image
    let test_image_data = create_test_image();
    
    // Test rendering at different resolutions
    println!("🖼️  Rendering at different resolutions...");
    
    // Thumbnail
    println!("   📱 Generating thumbnail...");
    let thumbnail = media_state.render_thumbnail(&test_image_data)?;
    fs::write("test_thumbnail.png", &thumbnail)?;
    println!("      Saved: test_thumbnail.png ({} bytes)", thumbnail.len());
    
    // Preview
    println!("   🖥️  Generating preview...");
    let preview = media_state.render_preview(&test_image_data)?;
    fs::write("test_preview.png", &preview)?;
    println!("      Saved: test_preview.png ({} bytes)", preview.len());
    
    // Full resolution
    println!("   🎯 Generating full resolution...");
    let full = media_state.render(&test_image_data, RenderResolution::Full)?;
    fs::write("test_full.png", &full)?;
    println!("      Saved: test_full.png ({} bytes)", full.len());
    
    // Custom resolution
    println!("   ⚙️  Generating custom 400x300...");
    let custom = media_state.render(&test_image_data, RenderResolution::Custom(400, 300))?;
    fs::write("test_custom.png", &custom)?;
    println!("      Saved: test_custom.png ({} bytes)", custom.len());
    
    println!("\n✅ Renderer test completed successfully!");
    println!("   Generated files:");
    println!("   - test_thumbnail.png (thumbnail)");
    println!("   - test_preview.png (preview)");
    println!("   - test_full.png (full resolution)");
    println!("   - test_custom.png (400x300 custom)");
    println!("\n💡 Open these files to verify:");
    println!("   - Crop applied (10% border removed)");
    println!("   - 90° rotation applied");
    println!("   - Red circle at top-left");
    println!("   - Green rectangle at bottom-right");
    
    Ok(())
}

fn create_test_image() -> Vec<u8> {
    use image::{RgbImage, Rgb};
    
    // Create a 200x200 test image with a gradient
    let mut img = RgbImage::new(200, 200);
    
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let r = (x as f32 / 200.0 * 255.0) as u8;
        let g = (y as f32 / 200.0 * 255.0) as u8;
        let b = 128;
        *pixel = Rgb([r, g, b]);
    }
    
    // Encode to PNG
    let mut buffer = Vec::new();
    let dynamic_img = image::DynamicImage::ImageRgb8(img);
    dynamic_img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png).unwrap();
    
    buffer
}
