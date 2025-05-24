mod block;
pub use self::block::decode_block_bc7;
use crate::bc7::{BC7Settings, CompressionVariant};

trait BlockRgba8Decoder {
    fn decode_block_rgba8(compressed: &[u8], decompressed: &mut [u8], pitch: usize);
    fn block_byte_size() -> u32;
}

struct BC7Decoder;

impl BlockRgba8Decoder for BC7Decoder {
    #[inline(always)]
    fn decode_block_rgba8(compressed: &[u8], decompressed: &mut [u8], pitch: usize) {
        decode_block_bc7(compressed, decompressed, pitch)
    }

    fn block_byte_size() -> u32 {
        CompressionVariant::BC7(BC7Settings::alpha_basic()).block_byte_size()
    }
}

fn decompress_rgba8<D: BlockRgba8Decoder>(
    width: u32,
    height: u32,
    blocks_data: &[u8],
    rgba_data: &mut [u8],
) {
    let blocks_x = (width + 3) / 4;
    let blocks_y = (height + 3) / 4;
    let block_byte_size = D::block_byte_size() as usize;
    let output_row_pitch = width as usize * 4; // Always RGBA

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_index = (by * blocks_x + bx) as usize;
            let block_offset = block_index * block_byte_size;

            if block_offset + block_byte_size > blocks_data.len() {
                break;
            }

            let output_offset = (by * 4 * output_row_pitch as u32 + bx * 16) as usize;

            if output_offset < rgba_data.len() {
                D::decode_block_rgba8(
                    &blocks_data[block_offset..block_offset + block_byte_size],
                    &mut rgba_data[output_offset..],
                    output_row_pitch,
                );
            }
        }
    }
}

pub fn decompress_blocks_as_rgba8(
    variant: CompressionVariant,
    width: u32,
    height: u32,
    blocks_data: &[u8],
    rgba_data: &mut [u8],
) {
    let expected_input_size = variant.blocks_byte_size(width, height);
    assert_eq!(
        blocks_data.len(),
        expected_input_size,
        "the input bitstream slice has not the expected size"
    );

    let expected_output_size = width as usize * height as usize * 4;
    assert_eq!(
        rgba_data.len(),
        expected_output_size,
        "the output slice has not the expected size"
    );

    match variant {
        CompressionVariant::BC7(..) => {
            decompress_rgba8::<BC7Decoder>(width, height, blocks_data, rgba_data)
        }
    }
}
