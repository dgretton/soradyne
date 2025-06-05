use std::process::Command;
use image::{RgbaImage, Rgba};

#[cfg(feature = "video-thumbnails")]
use ffmpeg_next as ffmpeg;

/// Extract a video frame using FFmpeg
pub fn extract_video_frame(video_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    println!("🎬 extract_video_frame called with {} bytes", video_data.len());
    
    #[cfg(feature = "video-thumbnails")]
    {
        // Try native FFmpeg first
        match extract_video_frame_native(video_data) {
            Ok(frame_data) => {
                println!("✅ Native FFmpeg extraction succeeded: {} bytes", frame_data.len());
                return Ok(frame_data);
            }
            Err(e) => {
                println!("⚠️ Native FFmpeg failed: {}, falling back to system call", e);
            }
        }
    }
    
    // Fallback to system call
    extract_video_frame_system(video_data)
}

#[cfg(feature = "video-thumbnails")]
fn extract_video_frame_native(video_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    println!("🔧 Using native FFmpeg extraction");
    
    // Initialize FFmpeg
    ffmpeg::init()?;
    
    // Write video data to a temporary file for now
    // TODO: Implement proper memory-based input when the API supports it
    let temp_dir = std::env::temp_dir();
    let input_path = temp_dir.join(format!("video_input_{}.mp4", std::process::id()));
    std::fs::write(&input_path, video_data)?;
    
    // Open input file
    let mut input_context = ffmpeg::format::input(&input_path)?;
    
    // Find the best video stream
    let video_stream_index = input_context
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or("No video stream found")?
        .index();
    
    let video_stream = input_context.stream(video_stream_index).unwrap();
    let mut decoder = ffmpeg::codec::decoder::find(video_stream.parameters().id())
        .ok_or("Unsupported codec")?
        .video()?;
    decoder.set_parameters(video_stream.parameters())?;
    decoder.open(None)?;
    
    // Seek to 1 second
    let time_base = video_stream.time_base();
    let seek_timestamp = (1.0 / f64::from(time_base.denominator()) * f64::from(time_base.numerator())) as i64;
    input_context.seek(seek_timestamp, ..)?;
    
    // Decode frames until we get one
    let mut frame = ffmpeg::frame::Video::empty();
    
    for (stream, packet) in input_context.packets() {
        if stream.index() == video_stream_index {
            decoder.send_packet(&packet)?;
            
            while decoder.receive_frame(&mut frame).is_ok() {
                // Convert frame to RGB
                let mut rgb_frame = ffmpeg::frame::Video::empty();
                let mut converter = ffmpeg::software::scaling::context::Context::get(
                    frame.format(),
                    frame.width(),
                    frame.height(),
                    ffmpeg::format::Pixel::RGB24,
                    frame.width(),
                    frame.height(),
                    ffmpeg::software::scaling::flag::Flags::BILINEAR,
                )?;
                
                converter.run(&frame, &mut rgb_frame)?;
                
                // Convert RGB frame to JPEG bytes
                let jpeg_data = rgb_frame_to_jpeg(&rgb_frame)?;
                println!("📸 Native FFmpeg extracted {} bytes", jpeg_data.len());
                
                // Clean up temp file
                let _ = std::fs::remove_file(&input_path);
                
                return Ok(jpeg_data);
            }
        }
    }
    
    // Clean up temp file
    let _ = std::fs::remove_file(&input_path);
    
    Err("No frame could be extracted".into())
}

#[cfg(feature = "video-thumbnails")]
fn rgb_frame_to_jpeg(frame: &ffmpeg::frame::Video) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use image::{ImageBuffer, Rgb};
    
    let width = frame.width();
    let height = frame.height();
    let data = frame.data(0);
    
    // Create image buffer from RGB data
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, data.to_vec())
        .ok_or("Failed to create image buffer from frame data")?;
    
    // Encode as JPEG
    let mut buffer = Vec::new();
    let dynamic_img = image::DynamicImage::ImageRgb8(img);
    dynamic_img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Jpeg(85))?;
    
    Ok(buffer)
}

fn extract_video_frame_system(video_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    println!("🔧 Using system FFmpeg extraction");
    
    // Check if ffmpeg is available
    let ffmpeg_check = Command::new("ffmpeg").arg("-version").output();
    match ffmpeg_check {
        Ok(output) if output.status.success() => {
            println!("✅ System ffmpeg is available");
        }
        Ok(output) => {
            println!("❌ System ffmpeg version check failed: {}", String::from_utf8_lossy(&output.stderr));
            return Err("System ffmpeg version check failed".into());
        }
        Err(e) => {
            println!("❌ System ffmpeg not found: {}", e);
            return Err(format!("System ffmpeg not found: {}", e).into());
        }
    }
    
    // Create temporary files
    let temp_dir = std::env::temp_dir();
    println!("📁 Using temp dir: {:?}", temp_dir);
    let input_path = temp_dir.join(format!("video_input_{}.mp4", std::process::id()));
    let output_path = temp_dir.join(format!("frame_output_{}.jpg", std::process::id()));
    
    // Write video data to temporary file
    println!("📝 Writing {} bytes to temp file: {:?}", video_data.len(), input_path);
    std::fs::write(&input_path, video_data)?;
    println!("✅ Successfully wrote video data to temp file");
    
    // Extract frame at 1 second using FFmpeg
    println!("🎬 Running system ffmpeg to extract frame...");
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", input_path.to_str().unwrap(),
            "-ss", "00:00:01.000",  // Seek to 1 second
            "-vframes", "1",        // Extract 1 frame
            "-q:v", "2",           // High quality
            "-y",                  // Overwrite output
            output_path.to_str().unwrap()
        ])
        .output();
    
    // Clean up input file
    let _ = std::fs::remove_file(&input_path);
    
    match output {
        Ok(result) if result.status.success() => {
            println!("✅ System ffmpeg succeeded, reading extracted frame");
            // Read the extracted frame
            let frame_data = std::fs::read(&output_path)?;
            println!("📸 Successfully read {} bytes from extracted frame", frame_data.len());
            // Clean up output file
            let _ = std::fs::remove_file(&output_path);
            Ok(frame_data)
        }
        Ok(result) => {
            println!("❌ System ffmpeg failed with exit code: {:?}", result.status.code());
            println!("❌ System ffmpeg stderr: {}", String::from_utf8_lossy(&result.stderr));
            println!("❌ System ffmpeg stdout: {}", String::from_utf8_lossy(&result.stdout));
            // Clean up output file if it exists
            let _ = std::fs::remove_file(&output_path);
            Err("System FFmpeg frame extraction failed".into())
        }
        Err(e) => {
            println!("❌ Failed to execute system ffmpeg: {}", e);
            // Clean up output file if it exists
            let _ = std::fs::remove_file(&output_path);
            Err(format!("Failed to execute system ffmpeg: {}", e).into())
        }
    }
}

/// Generate video thumbnail at specific size
pub fn generate_video_at_size(video_data: &[u8], max_size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Try to extract a frame using FFmpeg first
    if let Ok(frame_data) = extract_video_frame(video_data) {
        // Generate resized image from the extracted frame
        return generate_image_at_size(&frame_data, max_size);
    }
    
    // Fall back to placeholder if FFmpeg extraction fails
    create_video_placeholder_at_size(max_size)
}

/// Generate resized image at specific size
pub fn generate_image_at_size(image_data: &[u8], max_size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Load the image from the data
    let img = image::load_from_memory(image_data)?;
    
    // Resize while maintaining aspect ratio
    let resized = img.thumbnail(max_size, max_size);
    
    // Use higher quality for larger sizes
    let quality = match max_size {
        0..=200 => 70,
        201..=800 => 85,
        _ => 95,
    };
    
    // Encode as JPEG
    let mut buffer = Vec::new();
    resized.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Jpeg(quality))?;
    
    Ok(buffer)
}

/// Create video placeholder at specific size
pub fn create_video_placeholder_at_size(size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Create a size x size image with a prominent play button
    let mut img = RgbaImage::new(size, size);
    let center_x = size / 2;
    let center_y = size / 2;
    
    // Create a dark video-like background
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = Rgba([30, 30, 30, 255]);
        
        // Draw a large white play triangle scaled to size
        let triangle_size = size / 5;
        
        // Draw a proper right-pointing triangle (play button)
        let triangle_left = center_x - triangle_size / 3;
        let triangle_right = center_x + triangle_size / 3;
        
        // Check if we're in the triangle area
        if x >= triangle_left && x <= triangle_right {
            let relative_x = x as i32 - triangle_left as i32;
            let triangle_width = (triangle_right - triangle_left) as i32;
            
            // Calculate the triangle bounds at this x position (right-pointing)
            let half_height_at_x = (relative_x * triangle_size as i32) / (triangle_width * 2);
            let top_bound = center_y as i32 - half_height_at_x;
            let bottom_bound = center_y as i32 + half_height_at_x;
            
            if y as i32 >= top_bound && y as i32 <= bottom_bound {
                *pixel = Rgba([255, 255, 255, 255]); // White play button
            }
        }
        
        // Add a subtle border to make it look more like a video thumbnail
        let border_width = (size / 50).max(1);
        if x < border_width || x >= size - border_width || y < border_width || y >= size - border_width {
            *pixel = Rgba([100, 100, 100, 255]); // Gray border
        }
    }
    
    // Encode as PNG
    let mut buffer = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;
    
    Ok(buffer)
}

/// Create audio placeholder at specific size
pub fn create_audio_placeholder_at_size(size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use image::{RgbaImage, Rgba};
    
    // Create a size x size image with audio waveform visualization
    let mut img = RgbaImage::new(size, size);
    let dark_bg = Rgba([20, 25, 35, 255]);
    let waveform_color = Rgba([100, 150, 255, 255]);
    
    // Fill with dark background
    for pixel in img.pixels_mut() {
        *pixel = dark_bg;
    }
    
    // Draw simplified waveform bars
    let bar_width = (size / 40).max(2);
    let bar_spacing = (size / 25).max(3);
    let num_bars = size / bar_spacing;
    let center_y = size / 2;
    
    for i in 0..num_bars {
        let bar_x = i * bar_spacing + size / 15;
        let bar_height = (size / 8) + ((i * 7) % (size / 4)); // Varying heights
        let bar_top = center_y.saturating_sub(bar_height / 2);
        let bar_bottom = center_y + bar_height / 2;
        
        for y in bar_top..bar_bottom.min(size) {
            for x in bar_x..(bar_x + bar_width).min(size) {
                img.put_pixel(x, y, waveform_color);
            }
        }
    }
    
    // Encode as PNG
    let mut buffer = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;
    
    Ok(buffer)
}

/// Detect if data is a video file
pub fn is_video_file(data: &[u8]) -> bool {
    if data.len() < 12 {
        return false;
    }
    
    // Check for common video file signatures
    // MP4/MOV files start with specific patterns
    if data.len() >= 8 {
        // Check for MP4 ftyp box
        if &data[4..8] == b"ftyp" {
            return true;
        }
    }
    
    // Check for WebM signature
    if data.len() >= 4 && &data[0..4] == b"\x1A\x45\xDF\xA3" {
        return true;
    }
    
    // Check for AVI signature
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"AVI " {
        return true;
    }
    
    // Check for QuickTime/MOV signature
    if data.len() >= 8 && &data[4..8] == b"moov" {
        return true;
    }
    
    // Check for additional MP4 variants
    if data.len() >= 12 {
        let ftyp_slice = &data[4..8];
        if ftyp_slice == b"ftyp" {
            let brand = &data[8..12];
            // Common MP4 brands
            if brand == b"isom" || brand == b"mp41" || brand == b"mp42" || 
               brand == b"avc1" || brand == b"dash" || brand == b"iso2" {
                return true;
            }
        }
    }
    
    false
}

/// Detect if data is an audio file
pub fn is_audio_file(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    
    // Skip very small files that are likely just metadata/headers
    if data.len() < 1000 {
        return false;
    }
    
    // Check for MP3 signature - improved detection
    if data.len() >= 3 {
        // MP3 with ID3v2 tag
        if &data[0..3] == b"ID3" {
            return true;
        }
    }
    
    // Check for MP3 frame sync pattern - scan first few KB for frame headers
    for i in 0..(data.len().min(4096) - 1) {
        if data[i] == 0xFF && (data[i + 1] & 0xE0) == 0xE0 {
            // Additional validation: check if this looks like a valid MP3 frame header
            if i + 4 < data.len() {
                let header = u32::from_be_bytes([data[i], data[i+1], data[i+2], data[i+3]]);
                if is_valid_mp3_header(header) {
                    return true;
                }
            }
        }
    }
    
    // Check for FLAC signature
    if data.len() >= 4 && &data[0..4] == b"fLaC" {
        return true;
    }
    
    // Check for OGG signature (Vorbis/Opus)
    if data.len() >= 4 && &data[0..4] == b"OggS" {
        return true;
    }
    
    // Check for WAV signature - ensure it's a reasonable size
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WAVE" {
        return true;
    }
    
    // Check for AIFF signature
    if data.len() >= 12 && &data[0..4] == b"FORM" && &data[8..12] == b"AIFF" {
        return true;
    }
    
    // Check for M4A (AAC in MP4 container)
    if data.len() >= 12 {
        let ftyp_slice = &data[4..8];
        if ftyp_slice == b"ftyp" {
            let brand = &data[8..12];
            // Common M4A brands
            if brand == b"M4A " || brand == b"mp42" || brand == b"isom" {
                return true;
            }
        }
    }
    
    // Check for AAC ADTS header
    if data.len() >= 2 && data[0] == 0xFF && (data[1] & 0xF0) == 0xF0 {
        return true;
    }
    
    // Check for AC3 signature
    if data.len() >= 2 && data[0] == 0x0B && data[1] == 0x77 {
        return true;
    }
    
    false
}

fn is_valid_mp3_header(header: u32) -> bool {
    // Check MP3 frame header validity
    // Bits 31-21: Frame sync (all 1s) - already checked
    // Bits 20-19: MPEG Audio version ID
    let version = (header >> 19) & 0x3;
    if version == 1 { return false; } // Reserved
    
    // Bits 18-17: Layer description
    let layer = (header >> 17) & 0x3;
    if layer == 0 { return false; } // Reserved
    
    // Bits 15-12: Bitrate index
    let bitrate = (header >> 12) & 0xF;
    if bitrate == 0 || bitrate == 15 { return false; } // Free or bad bitrate
    
    // Bits 11-10: Sampling rate frequency index
    let sample_rate = (header >> 10) & 0x3;
    if sample_rate == 3 { return false; } // Reserved
    
    true
}
