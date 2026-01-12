//! MagicaVoxel VOX File Loader
//!
//! Parser for the MagicaVoxel .vox file format (RIFF-based).
//! Allows artists to create models in MagicaVoxel and drop them into the assets folder.
//!
//! ## VOX Format Reference
//!
//! ```text
//! VOX File Structure:
//! ├── "VOX " (4 bytes) - Magic number
//! ├── Version (4 bytes) - File version (150)
//! └── MAIN Chunk
//!     ├── SIZE Chunk - Model dimensions
//!     ├── XYZI Chunk - Voxel data
//!     └── RGBA Chunk - Palette (optional)
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use oroboros_rendering::assets::VoxLoader;
//!
//! let vox_file = VoxLoader::load("assets/models/sword.vox")?;
//! let model = vox_file.to_voxel_model();
//! ```

use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::fs::File;

use super::procedural_models::{VoxelModel, VoxelModelBuilder};

/// VOX file magic number.
const VOX_MAGIC: [u8; 4] = *b"VOX ";

/// Expected VOX version.
const VOX_VERSION: u32 = 150;

/// Default MagicaVoxel palette (256 colors).
static DEFAULT_PALETTE: &[u32; 256] = &[
    0x00000000, 0xffffffff, 0xffccffff, 0xff99ffff, 0xff66ffff, 0xff33ffff, 0xff00ffff, 0xffffccff,
    0xffccccff, 0xff99ccff, 0xff66ccff, 0xff33ccff, 0xff00ccff, 0xffff99ff, 0xffcc99ff, 0xff9999ff,
    0xff6699ff, 0xff3399ff, 0xff0099ff, 0xffff66ff, 0xffcc66ff, 0xff9966ff, 0xff6666ff, 0xff3366ff,
    0xff0066ff, 0xffff33ff, 0xffcc33ff, 0xff9933ff, 0xff6633ff, 0xff3333ff, 0xff0033ff, 0xffff00ff,
    0xffcc00ff, 0xff9900ff, 0xff6600ff, 0xff3300ff, 0xff0000ff, 0xffffffcc, 0xffccffcc, 0xff99ffcc,
    0xff66ffcc, 0xff33ffcc, 0xff00ffcc, 0xffffcccc, 0xffcccccc, 0xff99cccc, 0xff66cccc, 0xff33cccc,
    0xff00cccc, 0xffff99cc, 0xffcc99cc, 0xff9999cc, 0xff6699cc, 0xff3399cc, 0xff0099cc, 0xffff66cc,
    0xffcc66cc, 0xff9966cc, 0xff6666cc, 0xff3366cc, 0xff0066cc, 0xffff33cc, 0xffcc33cc, 0xff9933cc,
    0xff6633cc, 0xff3333cc, 0xff0033cc, 0xffff00cc, 0xffcc00cc, 0xff9900cc, 0xff6600cc, 0xff3300cc,
    0xff0000cc, 0xffffff99, 0xffccff99, 0xff99ff99, 0xff66ff99, 0xff33ff99, 0xff00ff99, 0xffffcc99,
    0xffcccc99, 0xff99cc99, 0xff66cc99, 0xff33cc99, 0xff00cc99, 0xffff9999, 0xffcc9999, 0xff999999,
    0xff669999, 0xff339999, 0xff009999, 0xffff6699, 0xffcc6699, 0xff996699, 0xff666699, 0xff336699,
    0xff006699, 0xffff3399, 0xffcc3399, 0xff993399, 0xff663399, 0xff333399, 0xff003399, 0xffff0099,
    0xffcc0099, 0xff990099, 0xff660099, 0xff330099, 0xff000099, 0xffffff66, 0xffccff66, 0xff99ff66,
    0xff66ff66, 0xff33ff66, 0xff00ff66, 0xffffcc66, 0xffcccc66, 0xff99cc66, 0xff66cc66, 0xff33cc66,
    0xff00cc66, 0xffff9966, 0xffcc9966, 0xff999966, 0xff669966, 0xff339966, 0xff009966, 0xffff6666,
    0xffcc6666, 0xff996666, 0xff666666, 0xff336666, 0xff006666, 0xffff3366, 0xffcc3366, 0xff993366,
    0xff663366, 0xff333366, 0xff003366, 0xffff0066, 0xffcc0066, 0xff990066, 0xff660066, 0xff330066,
    0xff000066, 0xffffff33, 0xffccff33, 0xff99ff33, 0xff66ff33, 0xff33ff33, 0xff00ff33, 0xffffcc33,
    0xffcccc33, 0xff99cc33, 0xff66cc33, 0xff33cc33, 0xff00cc33, 0xffff9933, 0xffcc9933, 0xff999933,
    0xff669933, 0xff339933, 0xff009933, 0xffff6633, 0xffcc6633, 0xff996633, 0xff666633, 0xff336633,
    0xff006633, 0xffff3333, 0xffcc3333, 0xff993333, 0xff663333, 0xff333333, 0xff003333, 0xffff0033,
    0xffcc0033, 0xff990033, 0xff660033, 0xff330033, 0xff000033, 0xffffff00, 0xffccff00, 0xff99ff00,
    0xff66ff00, 0xff33ff00, 0xff00ff00, 0xffffcc00, 0xffcccc00, 0xff99cc00, 0xff66cc00, 0xff33cc00,
    0xff00cc00, 0xffff9900, 0xffcc9900, 0xff999900, 0xff669900, 0xff339900, 0xff009900, 0xffff6600,
    0xffcc6600, 0xff996600, 0xff666600, 0xff336600, 0xff006600, 0xffff3300, 0xffcc3300, 0xff993300,
    0xff663300, 0xff333300, 0xff003300, 0xffff0000, 0xffcc0000, 0xff990000, 0xff660000, 0xff330000,
    0xff0000ee, 0xff0000dd, 0xff0000bb, 0xff0000aa, 0xff000088, 0xff000077, 0xff000055, 0xff000044,
    0xff000022, 0xff000011, 0xff00ee00, 0xff00dd00, 0xff00bb00, 0xff00aa00, 0xff008800, 0xff007700,
    0xff005500, 0xff004400, 0xff002200, 0xff001100, 0xffee0000, 0xffdd0000, 0xffbb0000, 0xffaa0000,
    0xff880000, 0xff770000, 0xff550000, 0xff440000, 0xff220000, 0xff110000, 0xffeeeeee, 0xffdddddd,
    0xffbbbbbb, 0xffaaaaaa, 0xff888888, 0xff777777, 0xff555555, 0xff444444, 0xff222222, 0xff111111,
];

/// Error types for VOX loading.
#[derive(Debug)]
pub enum VoxError {
    /// File I/O error.
    Io(io::Error),
    /// Invalid VOX file format.
    InvalidFormat(String),
    /// Unsupported VOX version.
    UnsupportedVersion(u32),
    /// Missing required chunk.
    MissingChunk(&'static str),
    /// Invalid chunk data.
    InvalidChunk(String),
}

impl std::fmt::Display for VoxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VoxError::Io(e) => write!(f, "IO error: {e}"),
            VoxError::InvalidFormat(msg) => write!(f, "Invalid VOX format: {msg}"),
            VoxError::UnsupportedVersion(v) => write!(f, "Unsupported VOX version: {v}"),
            VoxError::MissingChunk(name) => write!(f, "Missing required chunk: {name}"),
            VoxError::InvalidChunk(msg) => write!(f, "Invalid chunk: {msg}"),
        }
    }
}

impl std::error::Error for VoxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VoxError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for VoxError {
    fn from(e: io::Error) -> Self {
        VoxError::Io(e)
    }
}

/// Color entry in VOX palette.
#[derive(Debug, Clone, Copy, Default)]
pub struct VoxColor {
    /// Red component (0-255).
    pub r: u8,
    /// Green component (0-255).
    pub g: u8,
    /// Blue component (0-255).
    pub b: u8,
    /// Alpha component (0-255).
    pub a: u8,
}

impl VoxColor {
    /// Creates from packed RGBA u32.
    #[inline]
    #[must_use]
    pub const fn from_packed(packed: u32) -> Self {
        Self {
            r: (packed & 0xFF) as u8,
            g: ((packed >> 8) & 0xFF) as u8,
            b: ((packed >> 16) & 0xFF) as u8,
            a: ((packed >> 24) & 0xFF) as u8,
        }
    }
    
    /// Returns as normalized float array [r, g, b, a].
    #[inline]
    #[must_use]
    pub fn as_float(&self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }
    
    /// Returns approximate brightness (0.0 - 1.0).
    #[inline]
    #[must_use]
    pub fn brightness(&self) -> f32 {
        (0.299 * self.r as f32 + 0.587 * self.g as f32 + 0.114 * self.b as f32) / 255.0
    }
}

/// VOX file palette (256 colors).
#[derive(Debug, Clone)]
pub struct VoxPalette {
    /// Color entries.
    pub colors: [VoxColor; 256],
}

impl Default for VoxPalette {
    fn default() -> Self {
        let mut colors = [VoxColor::default(); 256];
        for (i, &packed) in DEFAULT_PALETTE.iter().enumerate() {
            colors[i] = VoxColor::from_packed(packed);
        }
        Self { colors }
    }
}

impl VoxPalette {
    /// Gets color at index.
    #[inline]
    #[must_use]
    pub fn get(&self, index: u8) -> VoxColor {
        self.colors[index as usize]
    }
}

/// Single voxel from VOX file.
#[derive(Debug, Clone, Copy)]
pub struct VoxVoxel {
    /// X position.
    pub x: u8,
    /// Y position.
    pub y: u8,
    /// Z position.
    pub z: u8,
    /// Color index (palette reference).
    pub color_index: u8,
}

/// Parsed VOX file.
#[derive(Debug, Clone)]
pub struct VoxFile {
    /// Model name (from filename).
    pub name: String,
    /// Model width (X).
    pub size_x: u32,
    /// Model height (Y in VOX = Z in engine).
    pub size_y: u32,
    /// Model depth (Z in VOX = Y in engine).
    pub size_z: u32,
    /// All voxels.
    pub voxels: Vec<VoxVoxel>,
    /// Color palette.
    pub palette: VoxPalette,
}

impl VoxFile {
    /// Converts to engine VoxelModel format.
    ///
    /// Note: VOX uses Y-up, engine uses Z-up. We swap Y/Z during conversion.
    #[must_use]
    pub fn to_voxel_model(&self) -> VoxelModel {
        let mut builder = VoxelModelBuilder::new(Box::leak(self.name.clone().into_boxed_str()))
            .with_origin(
                self.size_x as f32 / 2.0,
                0.0,
                self.size_z as f32 / 2.0,
            );
        
        for voxel in &self.voxels {
            let color = self.palette.get(voxel.color_index);
            
            // Swap Y/Z for coordinate system conversion
            let x = voxel.x;
            let y = voxel.z; // VOX Z -> Engine Y
            let z = voxel.y; // VOX Y -> Engine Z
            
            // Determine if this is an emissive voxel (bright colors)
            if color.brightness() > 0.9 && color.a == 255 {
                // Treat very bright colors as emissive
                let [r, g, b, _] = color.as_float();
                builder.add_emissive(x, y, z, voxel.color_index, r, g, b, 2.0);
            } else {
                builder.add_voxel(x, y, z, voxel.color_index);
            }
        }
        
        builder.build()
    }
    
    /// Returns the number of voxels.
    #[inline]
    #[must_use]
    pub fn voxel_count(&self) -> usize {
        self.voxels.len()
    }
}

/// VOX file loader.
pub struct VoxLoader;

impl VoxLoader {
    /// Loads a VOX file from disk.
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be read or has invalid format.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<VoxFile, VoxError> {
        let path = path.as_ref();
        let name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string();
        
        let file = File::open(path)?;
        let mut reader = io::BufReader::new(file);
        
        Self::parse(&mut reader, name)
    }
    
    /// Loads a VOX file from a byte slice.
    ///
    /// # Errors
    ///
    /// Returns error if data has invalid format.
    pub fn load_from_bytes(data: &[u8], name: String) -> Result<VoxFile, VoxError> {
        let mut cursor = io::Cursor::new(data);
        Self::parse(&mut cursor, name)
    }
    
    /// Parses VOX data from a reader.
    fn parse<R: Read + Seek>(reader: &mut R, name: String) -> Result<VoxFile, VoxError> {
        // Read magic number
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        
        if magic != VOX_MAGIC {
            return Err(VoxError::InvalidFormat(format!(
                "Expected 'VOX ', got '{}'",
                String::from_utf8_lossy(&magic)
            )));
        }
        
        // Read version
        let version = Self::read_u32(reader)?;
        if version != VOX_VERSION {
            // We support version 150, warn but continue for close versions
            if version < 150 || version > 200 {
                return Err(VoxError::UnsupportedVersion(version));
            }
        }
        
        // Read MAIN chunk
        let main_id = Self::read_chunk_id(reader)?;
        if main_id != *b"MAIN" {
            return Err(VoxError::InvalidFormat("Expected MAIN chunk".to_string()));
        }
        
        let _main_content_size = Self::read_u32(reader)?;
        let main_children_size = Self::read_u32(reader)?;
        
        // Parse child chunks
        let mut size_x = 0u32;
        let mut size_y = 0u32;
        let mut size_z = 0u32;
        let mut voxels = Vec::new();
        let mut palette = VoxPalette::default();
        
        let end_pos = reader.stream_position()? + main_children_size as u64;
        
        while reader.stream_position()? < end_pos {
            let chunk_id = Self::read_chunk_id(reader)?;
            let content_size = Self::read_u32(reader)?;
            let children_size = Self::read_u32(reader)?;
            
            match &chunk_id {
                b"SIZE" => {
                    size_x = Self::read_u32(reader)?;
                    size_y = Self::read_u32(reader)?;
                    size_z = Self::read_u32(reader)?;
                }
                b"XYZI" => {
                    let num_voxels = Self::read_u32(reader)?;
                    voxels.reserve(num_voxels as usize);
                    
                    for _ in 0..num_voxels {
                        let x = Self::read_u8(reader)?;
                        let y = Self::read_u8(reader)?;
                        let z = Self::read_u8(reader)?;
                        let color_index = Self::read_u8(reader)?;
                        
                        voxels.push(VoxVoxel { x, y, z, color_index });
                    }
                }
                b"RGBA" => {
                    // Custom palette
                    for i in 0..255 {
                        let r = Self::read_u8(reader)?;
                        let g = Self::read_u8(reader)?;
                        let b = Self::read_u8(reader)?;
                        let a = Self::read_u8(reader)?;
                        palette.colors[i + 1] = VoxColor { r, g, b, a };
                    }
                    // Skip the last entry
                    let _ = Self::read_u32(reader)?;
                }
                _ => {
                    // Skip unknown chunks
                    let skip = content_size + children_size;
                    reader.seek(SeekFrom::Current(skip as i64))?;
                }
            }
        }
        
        if size_x == 0 || size_y == 0 || size_z == 0 {
            return Err(VoxError::MissingChunk("SIZE"));
        }
        
        if voxels.is_empty() {
            return Err(VoxError::MissingChunk("XYZI"));
        }
        
        Ok(VoxFile {
            name,
            size_x,
            size_y,
            size_z,
            voxels,
            palette,
        })
    }
    
    /// Reads a little-endian u32.
    fn read_u32<R: Read>(reader: &mut R) -> Result<u32, VoxError> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    
    /// Reads a single byte.
    fn read_u8<R: Read>(reader: &mut R) -> Result<u8, VoxError> {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf)?;
        Ok(buf[0])
    }
    
    /// Reads a 4-byte chunk ID.
    fn read_chunk_id<R: Read>(reader: &mut R) -> Result<[u8; 4], VoxError> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        Ok(buf)
    }
}

/// Asset loader for models directory.
pub struct ModelAssetLoader {
    /// Base path to models directory.
    base_path: std::path::PathBuf,
}

impl ModelAssetLoader {
    /// Creates a new loader with the given base path.
    #[must_use]
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }
    
    /// Loads a VOX model by name.
    ///
    /// Searches for `{name}.vox` in the models directory.
    ///
    /// # Errors
    ///
    /// Returns error if model cannot be found or loaded.
    pub fn load_vox(&self, name: &str) -> Result<VoxFile, VoxError> {
        let path = self.base_path.join(format!("{name}.vox"));
        VoxLoader::load(path)
    }
    
    /// Lists all available VOX files.
    #[must_use]
    pub fn list_vox_files(&self) -> Vec<String> {
        let mut files = Vec::new();
        
        if let Ok(entries) = std::fs::read_dir(&self.base_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "vox") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        files.push(name.to_string());
                    }
                }
            }
        }
        
        files.sort();
        files
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_vox_color() {
        let color = VoxColor::from_packed(0xFF112233);
        assert_eq!(color.r, 0x33);
        assert_eq!(color.g, 0x22);
        assert_eq!(color.b, 0x11);
        assert_eq!(color.a, 0xFF);
    }
    
    #[test]
    fn test_default_palette() {
        let palette = VoxPalette::default();
        // Index 0 should be transparent
        assert_eq!(palette.get(0).a, 0);
        // Index 1 should be white
        let white = palette.get(1);
        assert_eq!(white.r, 255);
        assert_eq!(white.g, 255);
        assert_eq!(white.b, 255);
    }
    
    #[test]
    fn test_load_from_bytes_invalid() {
        let result = VoxLoader::load_from_bytes(b"invalid data", "test".to_string());
        assert!(result.is_err());
    }
    
    #[test]
    fn test_vox_color_brightness() {
        let white = VoxColor { r: 255, g: 255, b: 255, a: 255 };
        let black = VoxColor { r: 0, g: 0, b: 0, a: 255 };
        
        assert!(white.brightness() > 0.9);
        assert!(black.brightness() < 0.1);
    }
    
    // NOTE: Full integration tests require actual .vox files
    // They are located in tests/integration/
}
