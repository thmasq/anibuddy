mod bc7;

use bc7::BlockCompressorBC7;

use crate::bc7::{BC7Settings, CompressionVariant};

pub fn compress_rgba8(
    variation: CompressionVariant,
    rgba_data: &[u8],
    blocks_buffer: &mut [u8],
    width: u32,
    height: u32,
    stride: u32,
) {
    assert_eq!(height % 4, 0);
    assert_eq!(width % 4, 0);

    let required_size = variation.blocks_byte_size(width, height);

    assert!(
        blocks_buffer.len() >= required_size,
        "blocks_buffer size ({}) is too small to hold compressed blocks. Required size: {}",
        blocks_buffer.len(),
        required_size
    );

    let stride = stride as usize;
    let block_width = (width as usize + 3) / 4;
    let block_height = (height as usize + 3) / 4;

    match variation {
        CompressionVariant::BC7(settings) => {
            compress_bc7(
                rgba_data,
                blocks_buffer,
                block_width,
                block_height,
                stride,
                &settings,
            );
        }
    }
}

fn compress_bc7(
    rgba_data: &[u8],
    blocks_buffer: &mut [u8],
    block_width: usize,
    block_height: usize,
    stride: usize,
    settings: &BC7Settings,
) {
    for yy in 0..block_height {
        for xx in 0..block_width {
            let mut block_compressor = BlockCompressorBC7::new(settings);

            block_compressor.load_block_interleaved_rgba(rgba_data, xx, yy, stride);
            block_compressor.compute_opaque_err();
            block_compressor.compress_block_bc7_core();
            block_compressor.store_data(blocks_buffer, block_width, xx, yy);
        }
    }
}
