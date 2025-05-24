mod block_compressor;
pub mod decode;
pub mod encode;
mod settings;

use std::hash::{Hash, Hasher};

pub use block_compressor::GpuBlockCompressor;
pub use bytemuck;
pub use settings::BC7Settings;

/// Block compression variants supported by this crate.
#[derive(Clone, Copy)]
pub enum CompressionVariant {
    /// BC7 compression with smooth alpha (RGBA)
    BC7(BC7Settings),
}

impl PartialEq for CompressionVariant {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl Eq for CompressionVariant {}

impl Hash for CompressionVariant {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
    }
}

impl CompressionVariant {
    /// Returns the bytes per row for the given width.
    ///
    /// The width is used to calculate how many blocks are needed per row,
    /// which is then multiplied by the block size.
    /// Width is rounded up to the nearest multiple of 4.
    pub const fn bytes_per_row(self, width: u32) -> u32 {
        let blocks_per_row = (width + 3) / 4;
        blocks_per_row * self.block_byte_size()
    }

    /// Returns the byte size required for storing compressed blocks for the given dimensions.
    ///
    /// The size is calculated based on the block compression format and rounded up dimensions.
    /// Width and height are rounded up to the nearest multiple of 4.
    pub const fn blocks_byte_size(self, width: u32, height: u32) -> usize {
        let block_width = (width as usize + 3) / 4;
        let block_height = (height as usize + 3) / 4;
        let block_count = block_width * block_height;
        let block_size = self.block_byte_size() as usize;
        block_count * block_size
    }

    const fn block_byte_size(self) -> u32 {
        match self {
            Self::BC7(..) => 16,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::BC7(..) => "bc7",
        }
    }

    const fn entry_point(self) -> &'static str {
        match self {
            Self::BC7(..) => "compress_bc7",
        }
    }
}
