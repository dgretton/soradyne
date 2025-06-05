//! Media rendering system for generating thumbnails and multi-resolution outputs
//! 
//! This module handles rendering media with applied edits (crop, rotation, markup)
//! at different resolutions, with proper composition of multiple edits.

use super::album::{MediaState, CropData, MarkupElement};
use super::operations::MarkupType;
use serde::{Serialize, Deserialize};
use image::{DynamicImage, Rgba, RgbaImage, imageops::FilterType};
use std::process::Command;
use std::path::Path;
use imageproc::drawing::{draw_filled_circle_mut, draw_hollow_circle_mut, draw_filled_rect_mut, draw_hollow_rect_mut, draw_line_segment_mut};
use imageproc::rect::Rect;

// === Rendering Configuration ===

#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub thumbnail_size: u32,      // e.g., 150px
    pub preview_size: u32,        // e.g., 800px  
    pub max_full_size: u32,       // e.g., 2048px
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            thumbnail_size: 150,
            preview_size: 800,
            max_full_size: 2048,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RenderResolution {
    Thumbnail,
    Preview,
    Full,
    Custom(u32, u32),
}

// === Markup Data Structures ===

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CircleMarkup {
    pub center_x: f32,      // Normalized coordinates (0.0-1.0)
    pub center_y: f32,
    pub radius: f32,        // Normalized radius
    pub color: [u8; 4],     // RGBA
    pub filled: bool,
    pub stroke_width: f32,  // Normalized stroke width
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RectangleMarkup {
    pub x: f32,             // Normalized coordinates
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: [u8; 4],
    pub filled: bool,
    pub stroke_width: f32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ArrowMarkup {
    pub start_x: f32,       // Normalized coordinates
    pub start_y: f32,
    pub end_x: f32,
    pub end_y: f32,
    pub color: [u8; 4],
    pub stroke_width: f32,
    pub arrow_head_size: f32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TextMarkup {
    pub x: f32,             // Normalized coordinates
    pub y: f32,
    pub text: String,
    pub font_size: f32,     // Normalized font size
    pub color: [u8; 4],
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FreehandMarkup {
    pub points: Vec<(f32, f32)>,  // Normalized coordinates
    pub color: [u8; 4],
    pub stroke_width: f32,
}

// === Main Renderer ===

pub struct MediaRenderer {
    base_image: DynamicImage,
    config: RenderConfig,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("Image processing error: {0}")]
    ImageError(#[from] image::ImageError),
    
    #[error("Invalid markup data: {0}")]
    InvalidMarkup(String),
    
    #[error("Rendering failed: {0}")]
    RenderFailed(String),
}

impl MediaRenderer {
    pub fn new(base_image_data: &[u8], config: Option<RenderConfig>) -> Result<Self, RenderError> {
        let base_image = image::load_from_memory(base_image_data)?;
        let config = config.unwrap_or_default();
        
        Ok(Self {
            base_image,
            config,
        })
    }
    
    /// Render media at specified resolution with all edits applied
    pub fn render(&self, media_state: &MediaState, resolution: RenderResolution) -> Result<Vec<u8>, RenderError> {
        // Step 1: Determine target dimensions
        let (target_width, target_height) = self.calculate_dimensions(resolution);
        
        // Step 2: Apply transforms (crop + rotation) - LWW semantics
        let mut canvas = self.apply_transforms(media_state, target_width, target_height)?;
        
        // Step 3: Apply all markup layers in chronological order
        self.apply_markup_layers(&mut canvas, media_state)?;
        
        // Step 4: Encode to bytes
        let mut output = Vec::new();
        canvas.write_to(&mut std::io::Cursor::new(&mut output), image::ImageOutputFormat::Png)?;
        
        Ok(output)
    }
    
    /// Convenience method for thumbnail generation
    pub fn render_thumbnail(&self, media_state: &MediaState) -> Result<Vec<u8>, RenderError> {
        self.render(media_state, RenderResolution::Thumbnail)
    }
    
    /// Convenience method for preview generation
    pub fn render_preview(&self, media_state: &MediaState) -> Result<Vec<u8>, RenderError> {
        self.render(media_state, RenderResolution::Preview)
    }
    
    /// Convenience method for full resolution
    pub fn render_full(&self, media_state: &MediaState) -> Result<Vec<u8>, RenderError> {
        self.render(media_state, RenderResolution::Full)
    }
    
    /// Generate a thumbnail from video file using ffmpeg
    pub fn generate_video_thumbnail(video_path: &Path, timestamp_seconds: f64) -> Result<Vec<u8>, RenderError> {
        let output_path = std::env::temp_dir().join(format!("thumb_{}.jpg", uuid::Uuid::new_v4()));
        
        let output = Command::new("ffmpeg")
            .args(&[
                "-i", video_path.to_str().unwrap(),
                "-ss", &timestamp_seconds.to_string(),
                "-vframes", "1",
                "-q:v", "2",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| RenderError::RenderFailed(format!("Failed to run ffmpeg: {}", e)))?;
        
        if !output.status.success() {
            return Err(RenderError::RenderFailed(format!(
                "ffmpeg failed: {}", 
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        
        let thumbnail_data = std::fs::read(&output_path)
            .map_err(|e| RenderError::RenderFailed(format!("Failed to read thumbnail: {}", e)))?;
        
        // Clean up temp file
        let _ = std::fs::remove_file(&output_path);
        
        Ok(thumbnail_data)
    }
    
    fn calculate_dimensions(&self, resolution: RenderResolution) -> (u32, u32) {
        let (base_width, base_height) = (self.base_image.width(), self.base_image.height());
        
        match resolution {
            RenderResolution::Thumbnail => {
                let size = self.config.thumbnail_size;
                self.fit_to_square(base_width, base_height, size)
            }
            RenderResolution::Preview => {
                let max_size = self.config.preview_size;
                self.fit_to_max(base_width, base_height, max_size)
            }
            RenderResolution::Full => {
                let max_size = self.config.max_full_size;
                if base_width.max(base_height) <= max_size {
                    (base_width, base_height)
                } else {
                    self.fit_to_max(base_width, base_height, max_size)
                }
            }
            RenderResolution::Custom(width, height) => (width, height),
        }
    }
    
    fn fit_to_square(&self, width: u32, height: u32, size: u32) -> (u32, u32) {
        let aspect_ratio = width as f32 / height as f32;
        if aspect_ratio > 1.0 {
            (size, (size as f32 / aspect_ratio) as u32)
        } else {
            ((size as f32 * aspect_ratio) as u32, size)
        }
    }
    
    fn fit_to_max(&self, width: u32, height: u32, max_size: u32) -> (u32, u32) {
        if width.max(height) <= max_size {
            return (width, height);
        }
        
        let scale = max_size as f32 / width.max(height) as f32;
        ((width as f32 * scale) as u32, (height as f32 * scale) as u32)
    }
    
    fn apply_transforms(&self, media_state: &MediaState, target_width: u32, target_height: u32) -> Result<DynamicImage, RenderError> {
        let mut image = self.base_image.clone();
        
        // Apply crop first (LWW - use the most recent crop)
        if let Some(crop) = &media_state.crop {
            image = self.apply_crop(&image, crop)?;
        }
        
        // Apply rotation (LWW - use the current rotation value)
        if media_state.rotation != 0.0 {
            image = self.apply_rotation(&image, media_state.rotation)?;
        }
        
        // Resize to target dimensions
        Ok(image.resize_exact(target_width, target_height, FilterType::Lanczos3))
    }
    
    fn apply_crop(&self, image: &DynamicImage, crop: &CropData) -> Result<DynamicImage, RenderError> {
        let (width, height) = (image.width(), image.height());
        
        let x = (crop.left * width as f32) as u32;
        let y = (crop.top * height as f32) as u32;
        let crop_width = ((crop.right - crop.left) * width as f32) as u32;
        let crop_height = ((crop.bottom - crop.top) * height as f32) as u32;
        
        // Ensure crop bounds are valid
        let x = x.min(width.saturating_sub(1));
        let y = y.min(height.saturating_sub(1));
        let crop_width = crop_width.min(width - x);
        let crop_height = crop_height.min(height - y);
        
        Ok(image.crop_imm(x, y, crop_width, crop_height))
    }
    
    fn apply_rotation(&self, image: &DynamicImage, angle: f32) -> Result<DynamicImage, RenderError> {
        // Normalize angle to 0-360 range
        let normalized_angle = angle % 360.0;
        let normalized_angle = if normalized_angle < 0.0 { normalized_angle + 360.0 } else { normalized_angle };
        
        // Apply rotation in 90-degree increments for now (can be enhanced later)
        match normalized_angle as i32 {
            0..=44 | 316..=360 => Ok(image.clone()),
            45..=134 => Ok(image.rotate90()),
            135..=224 => Ok(image.rotate180()),
            225..=315 => Ok(image.rotate270()),
            _ => Ok(image.clone()),
        }
    }
    
    fn apply_markup_layers(&self, canvas: &mut DynamicImage, media_state: &MediaState) -> Result<(), RenderError> {
        // Convert to RGBA for drawing operations
        let mut rgba_image = canvas.to_rgba8();
        let (width, height) = (rgba_image.width(), rgba_image.height());
        
        // Sort markup by timestamp to apply in chronological order
        let mut sorted_markup = media_state.markup.clone();
        sorted_markup.sort_by_key(|m| m.timestamp);
        
        // Apply each markup element
        for markup in &sorted_markup {
            // Skip deleted markup
            if media_state.deleted_items.contains(&markup.id) {
                continue;
            }
            
            self.apply_single_markup(&mut rgba_image, markup, width, height)?;
        }
        
        *canvas = DynamicImage::ImageRgba8(rgba_image);
        Ok(())
    }
    
    fn apply_single_markup(&self, canvas: &mut RgbaImage, markup: &MarkupElement, width: u32, height: u32) -> Result<(), RenderError> {
        match markup.markup_type {
            MarkupType::Circle => self.draw_circle(canvas, &markup.data, width, height)?,
            MarkupType::Rectangle => self.draw_rectangle(canvas, &markup.data, width, height)?,
            MarkupType::Arrow => self.draw_arrow(canvas, &markup.data, width, height)?,
            MarkupType::Text => self.draw_text(canvas, &markup.data, width, height)?,
            MarkupType::Freehand => self.draw_freehand(canvas, &markup.data, width, height)?,
        }
        Ok(())
    }
    
    fn draw_circle(&self, canvas: &mut RgbaImage, data: &serde_json::Value, width: u32, height: u32) -> Result<(), RenderError> {
        let circle: CircleMarkup = serde_json::from_value(data.clone())
            .map_err(|e| RenderError::InvalidMarkup(format!("Circle markup: {}", e)))?;
        
        let center_x = (circle.center_x * width as f32) as i32;
        let center_y = (circle.center_y * height as f32) as i32;
        let radius = (circle.radius * width.min(height) as f32) as i32;
        let color = Rgba(circle.color);
        
        if circle.filled {
            draw_filled_circle_mut(canvas, (center_x, center_y), radius, color);
        } else {
            draw_hollow_circle_mut(canvas, (center_x, center_y), radius, color);
        }
        
        Ok(())
    }
    
    fn draw_rectangle(&self, canvas: &mut RgbaImage, data: &serde_json::Value, width: u32, height: u32) -> Result<(), RenderError> {
        let rect: RectangleMarkup = serde_json::from_value(data.clone())
            .map_err(|e| RenderError::InvalidMarkup(format!("Rectangle markup: {}", e)))?;
        
        let x = (rect.x * width as f32) as i32;
        let y = (rect.y * height as f32) as i32;
        let rect_width = (rect.width * width as f32) as u32;
        let rect_height = (rect.height * height as f32) as u32;
        let color = Rgba(rect.color);
        
        let rectangle = Rect::at(x, y).of_size(rect_width, rect_height);
        
        if rect.filled {
            draw_filled_rect_mut(canvas, rectangle, color);
        } else {
            draw_hollow_rect_mut(canvas, rectangle, color);
        }
        
        Ok(())
    }
    
    fn draw_arrow(&self, canvas: &mut RgbaImage, data: &serde_json::Value, width: u32, height: u32) -> Result<(), RenderError> {
        let arrow: ArrowMarkup = serde_json::from_value(data.clone())
            .map_err(|e| RenderError::InvalidMarkup(format!("Arrow markup: {}", e)))?;
        
        let start_x = (arrow.start_x * width as f32) as f32;
        let start_y = (arrow.start_y * height as f32) as f32;
        let end_x = (arrow.end_x * width as f32) as f32;
        let end_y = (arrow.end_y * height as f32) as f32;
        let color = Rgba(arrow.color);
        
        // Draw main line
        draw_line_segment_mut(canvas, (start_x, start_y), (end_x, end_y), color);
        
        // TODO: Add arrow head drawing
        // For now, just draw the line
        
        Ok(())
    }
    
    fn draw_text(&self, _canvas: &mut RgbaImage, data: &serde_json::Value, _width: u32, _height: u32) -> Result<(), RenderError> {
        let _text: TextMarkup = serde_json::from_value(data.clone())
            .map_err(|e| RenderError::InvalidMarkup(format!("Text markup: {}", e)))?;
        
        // TODO: Implement text rendering with ab_glyph
        // For now, skip text rendering
        
        Ok(())
    }
    
    fn draw_freehand(&self, canvas: &mut RgbaImage, data: &serde_json::Value, width: u32, height: u32) -> Result<(), RenderError> {
        let freehand: FreehandMarkup = serde_json::from_value(data.clone())
            .map_err(|e| RenderError::InvalidMarkup(format!("Freehand markup: {}", e)))?;
        
        let color = Rgba(freehand.color);
        
        // Draw lines between consecutive points
        for window in freehand.points.windows(2) {
            let start_x = (window[0].0 * width as f32) as f32;
            let start_y = (window[0].1 * height as f32) as f32;
            let end_x = (window[1].0 * width as f32) as f32;
            let end_y = (window[1].1 * height as f32) as f32;
            
            draw_line_segment_mut(canvas, (start_x, start_y), (end_x, end_y), color);
        }
        
        Ok(())
    }
}

// === Integration with MediaState ===

impl super::album::MediaState {
    /// Render this media state at the specified resolution
    pub fn render(&self, base_image_data: &[u8], resolution: RenderResolution) -> Result<Vec<u8>, RenderError> {
        let renderer = MediaRenderer::new(base_image_data, None)?;
        renderer.render(self, resolution)
    }
    
    /// Generate thumbnail for this media state
    pub fn render_thumbnail(&self, base_image_data: &[u8]) -> Result<Vec<u8>, RenderError> {
        let renderer = MediaRenderer::new(base_image_data, None)?;
        renderer.render_thumbnail(self)
    }
    
    /// Generate preview for this media state
    pub fn render_preview(&self, base_image_data: &[u8]) -> Result<Vec<u8>, RenderError> {
        let renderer = MediaRenderer::new(base_image_data, None)?;
        renderer.render_preview(self)
    }
}
