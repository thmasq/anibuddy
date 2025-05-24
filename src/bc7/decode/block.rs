#[allow(clippy::needless_range_loop)]
pub fn decode_block_bc7(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    static ACTUAL_BITS_COUNT: &[[u8; 8]; 2] = &[
        [4, 6, 5, 7, 5, 7, 7, 5], // RGBA
        [0, 0, 0, 0, 6, 8, 7, 5], // Alpha
    ];

    // There are 64 possible partition sets for a two-region tile.
    // Each 4x4 block represents a single shape.
    // Here also every fix-up index has MSB bit set.
    static PARTITION_SETS: &[[[[u8; 4]; 4]; 64]; 2] = &[
        [
            // Partition table for 2-subset BPTC
            [[128, 0, 1, 1], [0, 0, 1, 1], [0, 0, 1, 1], [0, 0, 1, 129]], //  0
            [[128, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 129]], //  1
            [[128, 1, 1, 1], [0, 1, 1, 1], [0, 1, 1, 1], [0, 1, 1, 129]], //  2
            [[128, 0, 0, 1], [0, 0, 1, 1], [0, 0, 1, 1], [0, 1, 1, 129]], //  3
            [[128, 0, 0, 0], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 1, 129]], //  4
            [[128, 0, 1, 1], [0, 1, 1, 1], [0, 1, 1, 1], [1, 1, 1, 129]], //  5
            [[128, 0, 0, 1], [0, 0, 1, 1], [0, 1, 1, 1], [1, 1, 1, 129]], //  6
            [[128, 0, 0, 0], [0, 0, 0, 1], [0, 0, 1, 1], [0, 1, 1, 129]], //  7
            [[128, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 1], [0, 0, 1, 129]], //  8
            [[128, 0, 1, 1], [0, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 129]], //  9
            [[128, 0, 0, 0], [0, 0, 0, 1], [0, 1, 1, 1], [1, 1, 1, 129]], // 10
            [[128, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 1], [0, 1, 1, 129]], // 11
            [[128, 0, 0, 1], [0, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 129]], // 12
            [[128, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1], [1, 1, 1, 129]], // 13
            [[128, 0, 0, 0], [1, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 129]], // 14
            [[128, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 129]], // 15
            [[128, 0, 0, 0], [1, 0, 0, 0], [1, 1, 1, 0], [1, 1, 1, 129]], // 16
            [[128, 1, 129, 1], [0, 0, 0, 1], [0, 0, 0, 0], [0, 0, 0, 0]], // 17
            [[128, 0, 0, 0], [0, 0, 0, 0], [129, 0, 0, 0], [1, 1, 1, 0]], // 18
            [[128, 1, 129, 1], [0, 0, 1, 1], [0, 0, 0, 1], [0, 0, 0, 0]], // 19
            [[128, 0, 129, 1], [0, 0, 0, 1], [0, 0, 0, 0], [0, 0, 0, 0]], // 20
            [[128, 0, 0, 0], [1, 0, 0, 0], [129, 1, 0, 0], [1, 1, 1, 0]], // 21
            [[128, 0, 0, 0], [0, 0, 0, 0], [129, 0, 0, 0], [1, 1, 0, 0]], // 22
            [[128, 1, 1, 1], [0, 0, 1, 1], [0, 0, 1, 1], [0, 0, 0, 129]], // 23
            [[128, 0, 129, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 0]], // 24
            [[128, 0, 0, 0], [1, 0, 0, 0], [129, 0, 0, 0], [1, 1, 0, 0]], // 25
            [[128, 1, 129, 0], [0, 1, 1, 0], [0, 1, 1, 0], [0, 1, 1, 0]], // 26
            [[128, 0, 129, 1], [0, 1, 1, 0], [0, 1, 1, 0], [1, 1, 0, 0]], // 27
            [[128, 0, 0, 1], [0, 1, 1, 1], [129, 1, 1, 0], [1, 0, 0, 0]], // 28
            [[128, 0, 0, 0], [1, 1, 1, 1], [129, 1, 1, 1], [0, 0, 0, 0]], // 29
            [[128, 1, 129, 1], [0, 0, 0, 1], [1, 0, 0, 0], [1, 1, 1, 0]], // 30
            [[128, 0, 129, 1], [1, 0, 0, 1], [1, 0, 0, 1], [1, 1, 0, 0]], // 31
            [[128, 1, 0, 1], [0, 1, 0, 1], [0, 1, 0, 1], [0, 1, 0, 129]], // 32
            [[128, 0, 0, 0], [1, 1, 1, 1], [0, 0, 0, 0], [1, 1, 1, 129]], // 33
            [[128, 1, 0, 1], [1, 0, 129, 0], [0, 1, 0, 1], [1, 0, 1, 0]], // 34
            [[128, 0, 1, 1], [0, 0, 1, 1], [129, 1, 0, 0], [1, 1, 0, 0]], // 35
            [[128, 0, 129, 1], [1, 1, 0, 0], [0, 0, 1, 1], [1, 1, 0, 0]], // 36
            [[128, 1, 0, 1], [0, 1, 0, 1], [129, 0, 1, 0], [1, 0, 1, 0]], // 37
            [[128, 1, 1, 0], [1, 0, 0, 1], [0, 1, 1, 0], [1, 0, 0, 129]], // 38
            [[128, 1, 0, 1], [1, 0, 1, 0], [1, 0, 1, 0], [0, 1, 0, 129]], // 39
            [[128, 1, 129, 1], [0, 0, 1, 1], [1, 1, 0, 0], [1, 1, 1, 0]], // 40
            [[128, 0, 0, 1], [0, 0, 1, 1], [129, 1, 0, 0], [1, 0, 0, 0]], // 41
            [[128, 0, 129, 1], [0, 0, 1, 0], [0, 1, 0, 0], [1, 1, 0, 0]], // 42
            [[128, 0, 129, 1], [1, 0, 1, 1], [1, 1, 0, 1], [1, 1, 0, 0]], // 43
            [[128, 1, 129, 0], [1, 0, 0, 1], [1, 0, 0, 1], [0, 1, 1, 0]], // 44
            [[128, 0, 1, 1], [1, 1, 0, 0], [1, 1, 0, 0], [0, 0, 1, 129]], // 45
            [[128, 1, 1, 0], [0, 1, 1, 0], [1, 0, 0, 1], [1, 0, 0, 129]], // 46
            [[128, 0, 0, 0], [0, 1, 129, 0], [0, 1, 1, 0], [0, 0, 0, 0]], // 47
            [[128, 1, 0, 0], [1, 1, 129, 0], [0, 1, 0, 0], [0, 0, 0, 0]], // 48
            [[128, 0, 129, 0], [0, 1, 1, 1], [0, 0, 1, 0], [0, 0, 0, 0]], // 49
            [[128, 0, 0, 0], [0, 0, 129, 0], [0, 1, 1, 1], [0, 0, 1, 0]], // 50
            [[128, 0, 0, 0], [0, 1, 0, 0], [129, 1, 1, 0], [0, 1, 0, 0]], // 51
            [[128, 1, 1, 0], [1, 1, 0, 0], [1, 0, 0, 1], [0, 0, 1, 129]], // 52
            [[128, 0, 1, 1], [0, 1, 1, 0], [1, 1, 0, 0], [1, 0, 0, 129]], // 53
            [[128, 1, 129, 0], [0, 0, 1, 1], [1, 0, 0, 1], [1, 1, 0, 0]], // 54
            [[128, 0, 129, 1], [1, 0, 0, 1], [1, 1, 0, 0], [0, 1, 1, 0]], // 55
            [[128, 1, 1, 0], [1, 1, 0, 0], [1, 1, 0, 0], [1, 0, 0, 129]], // 56
            [[128, 1, 1, 0], [0, 0, 1, 1], [0, 0, 1, 1], [1, 0, 0, 129]], // 57
            [[128, 1, 1, 1], [1, 1, 1, 0], [1, 0, 0, 0], [0, 0, 0, 129]], // 58
            [[128, 0, 0, 1], [1, 0, 0, 0], [1, 1, 1, 0], [0, 1, 1, 129]], // 59
            [[128, 0, 0, 0], [1, 1, 1, 1], [0, 0, 1, 1], [0, 0, 1, 129]], // 60
            [[128, 0, 129, 1], [0, 0, 1, 1], [1, 1, 1, 1], [0, 0, 0, 0]], // 61
            [[128, 0, 129, 0], [0, 0, 1, 0], [1, 1, 1, 0], [1, 1, 1, 0]], // 62
            [[128, 1, 0, 0], [0, 1, 0, 0], [0, 1, 1, 1], [0, 1, 1, 129]], // 63
        ],
        [
            // Partition table for 3-subset BPTC
            [[128, 0, 1, 129], [0, 0, 1, 1], [0, 2, 2, 1], [2, 2, 2, 130]], //  0
            [[128, 0, 0, 129], [0, 0, 1, 1], [130, 2, 1, 1], [2, 2, 2, 1]], //  1
            [[128, 0, 0, 0], [2, 0, 0, 1], [130, 2, 1, 1], [2, 2, 1, 129]], //  2
            [[128, 2, 2, 130], [0, 0, 2, 2], [0, 0, 1, 1], [0, 1, 1, 129]], //  3
            [[128, 0, 0, 0], [0, 0, 0, 0], [129, 1, 2, 2], [1, 1, 2, 130]], //  4
            [[128, 0, 1, 129], [0, 0, 1, 1], [0, 0, 2, 2], [0, 0, 2, 130]], //  5
            [[128, 0, 2, 130], [0, 0, 2, 2], [1, 1, 1, 1], [1, 1, 1, 129]], //  6
            [[128, 0, 1, 1], [0, 0, 1, 1], [130, 2, 1, 1], [2, 2, 1, 129]], //  7
            [[128, 0, 0, 0], [0, 0, 0, 0], [129, 1, 1, 1], [2, 2, 2, 130]], //  8
            [[128, 0, 0, 0], [1, 1, 1, 1], [129, 1, 1, 1], [2, 2, 2, 130]], //  9
            [[128, 0, 0, 0], [1, 1, 129, 1], [2, 2, 2, 2], [2, 2, 2, 130]], // 10
            [[128, 0, 1, 2], [0, 0, 129, 2], [0, 0, 1, 2], [0, 0, 1, 130]], // 11
            [[128, 1, 1, 2], [0, 1, 129, 2], [0, 1, 1, 2], [0, 1, 1, 130]], // 12
            [[128, 1, 2, 2], [0, 129, 2, 2], [0, 1, 2, 2], [0, 1, 2, 130]], // 13
            [[128, 0, 1, 129], [0, 1, 1, 2], [1, 1, 2, 2], [1, 2, 2, 130]], // 14
            [[128, 0, 1, 129], [2, 0, 0, 1], [130, 2, 0, 0], [2, 2, 2, 0]], // 15
            [[128, 0, 0, 129], [0, 0, 1, 1], [0, 1, 1, 2], [1, 1, 2, 130]], // 16
            [[128, 1, 1, 129], [0, 0, 1, 1], [130, 0, 0, 1], [2, 2, 0, 0]], // 17
            [[128, 0, 0, 0], [1, 1, 2, 2], [129, 1, 2, 2], [1, 1, 2, 130]], // 18
            [[128, 0, 2, 130], [0, 0, 2, 2], [0, 0, 2, 2], [1, 1, 1, 129]], // 19
            [[128, 1, 1, 129], [0, 1, 1, 1], [0, 2, 2, 2], [0, 2, 2, 130]], // 20
            [[128, 0, 0, 129], [0, 0, 0, 1], [130, 2, 2, 1], [2, 2, 2, 1]], // 21
            [[128, 0, 0, 0], [0, 0, 129, 1], [0, 1, 2, 2], [0, 1, 2, 130]], // 22
            [[128, 0, 0, 0], [1, 1, 0, 0], [130, 2, 129, 0], [2, 2, 1, 0]], // 23
            [[128, 1, 2, 130], [0, 129, 2, 2], [0, 0, 1, 1], [0, 0, 0, 0]], // 24
            [[128, 0, 1, 2], [0, 0, 1, 2], [129, 1, 2, 2], [2, 2, 2, 130]], // 25
            [[128, 1, 1, 0], [1, 2, 130, 1], [129, 2, 2, 1], [0, 1, 1, 0]], // 26
            [[128, 0, 0, 0], [0, 1, 129, 0], [1, 2, 130, 1], [1, 2, 2, 1]], // 27
            [[128, 0, 2, 2], [1, 1, 0, 2], [129, 1, 0, 2], [0, 0, 2, 130]], // 28
            [[128, 1, 1, 0], [0, 129, 1, 0], [2, 0, 0, 2], [2, 2, 2, 130]], // 29
            [[128, 0, 1, 1], [0, 1, 2, 2], [0, 1, 130, 2], [0, 0, 1, 129]], // 30
            [[128, 0, 0, 0], [2, 0, 0, 0], [130, 2, 1, 1], [2, 2, 2, 129]], // 31
            [[128, 0, 0, 0], [0, 0, 0, 2], [129, 1, 2, 2], [1, 2, 2, 130]], // 32
            [[128, 2, 2, 130], [0, 0, 2, 2], [0, 0, 1, 2], [0, 0, 1, 129]], // 33
            [[128, 0, 1, 129], [0, 0, 1, 2], [0, 0, 2, 2], [0, 2, 2, 130]], // 34
            [[128, 1, 2, 0], [0, 129, 2, 0], [0, 1, 130, 0], [0, 1, 2, 0]], // 35
            [[128, 0, 0, 0], [1, 1, 129, 1], [2, 2, 130, 2], [0, 0, 0, 0]], // 36
            [[128, 1, 2, 0], [1, 2, 0, 1], [130, 0, 129, 2], [0, 1, 2, 0]], // 37
            [[128, 1, 2, 0], [2, 0, 1, 2], [129, 130, 0, 1], [0, 1, 2, 0]], // 38
            [[128, 0, 1, 1], [2, 2, 0, 0], [1, 1, 130, 2], [0, 0, 1, 129]], // 39
            [[128, 0, 1, 1], [1, 1, 130, 2], [2, 2, 0, 0], [0, 0, 1, 129]], // 40
            [[128, 1, 0, 129], [0, 1, 0, 1], [2, 2, 2, 2], [2, 2, 2, 130]], // 41
            [[128, 0, 0, 0], [0, 0, 0, 0], [130, 1, 2, 1], [2, 1, 2, 129]], // 42
            [[128, 0, 2, 2], [1, 129, 2, 2], [0, 0, 2, 2], [1, 1, 2, 130]], // 43
            [[128, 0, 2, 130], [0, 0, 1, 1], [0, 0, 2, 2], [0, 0, 1, 129]], // 44
            [[128, 2, 2, 0], [1, 2, 130, 1], [0, 2, 2, 0], [1, 2, 2, 129]], // 45
            [[128, 1, 0, 1], [2, 2, 130, 2], [2, 2, 2, 2], [0, 1, 0, 129]], // 46
            [[128, 0, 0, 0], [2, 1, 2, 1], [130, 1, 2, 1], [2, 1, 2, 129]], // 47
            [[128, 1, 0, 129], [0, 1, 0, 1], [0, 1, 0, 1], [2, 2, 2, 130]], // 48
            [[128, 2, 2, 130], [0, 1, 1, 1], [0, 2, 2, 2], [0, 1, 1, 129]], // 49
            [[128, 0, 0, 2], [1, 129, 1, 2], [0, 0, 0, 2], [1, 1, 1, 130]], // 50
            [[128, 0, 0, 0], [2, 129, 1, 2], [2, 1, 1, 2], [2, 1, 1, 130]], // 51
            [[128, 2, 2, 2], [0, 129, 1, 1], [0, 1, 1, 1], [0, 2, 2, 130]], // 52
            [[128, 0, 0, 2], [1, 1, 1, 2], [129, 1, 1, 2], [0, 0, 0, 130]], // 53
            [[128, 1, 1, 0], [0, 129, 1, 0], [0, 1, 1, 0], [2, 2, 2, 130]], // 54
            [[128, 0, 0, 0], [0, 0, 0, 0], [2, 1, 129, 2], [2, 1, 1, 130]], // 55
            [[128, 1, 1, 0], [0, 129, 1, 0], [2, 2, 2, 2], [2, 2, 2, 130]], // 56
            [[128, 0, 2, 2], [0, 0, 1, 1], [0, 0, 129, 1], [0, 0, 2, 130]], // 57
            [[128, 0, 2, 2], [1, 1, 2, 2], [129, 1, 2, 2], [0, 0, 2, 130]], // 58
            [[128, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [2, 129, 1, 130]], // 59
            [[128, 0, 0, 130], [0, 0, 0, 1], [0, 0, 0, 2], [0, 0, 0, 129]], // 60
            [[128, 2, 2, 2], [1, 2, 2, 2], [0, 2, 2, 2], [129, 2, 2, 130]], // 61
            [[128, 1, 0, 129], [2, 2, 2, 2], [2, 2, 2, 2], [2, 2, 2, 130]], // 62
            [[128, 1, 1, 129], [2, 0, 1, 1], [130, 2, 0, 1], [2, 2, 2, 0]], // 63
        ],
    ];

    static WEIGHT2: &[i32] = &[0, 21, 43, 64];
    static WEIGHT3: &[i32] = &[0, 9, 18, 27, 37, 46, 55, 64];
    static WEIGHT4: &[i32] = &[0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

    const MODE_HAS_P_BITS: u8 = 0b11001011;

    let mut bstream = BitStream::new(compressed_block);

    // Find mode
    let mut mode = 0;
    while mode < 8 && bstream.read_bit() == 0 {
        mode += 1;
    }

    // Unexpected mode, clear the block (transparent black)
    if mode >= 8 {
        for i in 0..4 {
            for j in 0..4 {
                let offset = i * destination_pitch + j * 4;
                decompressed_block[offset..offset + 4].copy_from_slice(&[0, 0, 0, 0]);
            }
        }
        return;
    }

    let mut partition = 0;
    let mut num_partitions = 1;
    let mut rotation = 0;
    let mut index_selection_bit = 0;

    if mode == 0 || mode == 1 || mode == 2 || mode == 3 || mode == 7 {
        num_partitions = if mode == 0 || mode == 2 { 3 } else { 2 };
        partition = bstream.read_bits(if mode == 0 { 4 } else { 6 }) as usize;
    }

    let num_endpoints = num_partitions * 2;

    if mode == 4 || mode == 5 {
        rotation = bstream.read_bits(2);
        if mode == 4 {
            index_selection_bit = bstream.read_bit();
        }
    }

    // Extract endpoints
    let mut endpoints = [[0i32; 4]; 6];

    // RGB
    for i in 0..3 {
        for j in 0..num_endpoints {
            endpoints[j][i] = bstream.read_bits(ACTUAL_BITS_COUNT[0][mode as usize] as u32) as i32;
        }
    }

    // Alpha (if any)
    if ACTUAL_BITS_COUNT[1][mode as usize] > 0 {
        for j in 0..num_endpoints {
            endpoints[j][3] = bstream.read_bits(ACTUAL_BITS_COUNT[1][mode as usize] as u32) as i32;
        }
    }

    // Fully decode endpoints
    // Handle modes that have P-bits
    if mode == 0 || mode == 1 || mode == 3 || mode == 6 || mode == 7 {
        // Component-wise left-shift
        for endpoint in endpoints.iter_mut().take(num_endpoints) {
            for component in endpoint.iter_mut() {
                *component <<= 1;
            }
        }

        // If P-bit is shared
        if mode == 1 {
            let i = bstream.read_bit() as i32;
            let j = bstream.read_bit() as i32;

            // RGB component-wise insert pbits
            for k in 0..3 {
                endpoints[0][k] |= i;
                endpoints[1][k] |= i;
                endpoints[2][k] |= j;
                endpoints[3][k] |= j;
            }
        } else if MODE_HAS_P_BITS & (1 << mode) != 0 {
            // Unique P-bit per endpoint
            for endpoint in endpoints.iter_mut().take(num_endpoints) {
                let j = bstream.read_bit() as i32;
                for component in endpoint.iter_mut() {
                    *component |= j;
                }
            }
        }
    }

    // Fully decode endpoints
    // Component-wise precision adjustment
    for i in 0..num_endpoints {
        // Get color components precision including pbit
        let j = ACTUAL_BITS_COUNT[0][mode as usize] + ((MODE_HAS_P_BITS >> mode) & 1);

        // RGB components
        for k in 0..3 {
            // Left shift endpoint components so that their MSB lies in bit 7
            endpoints[i][k] <<= 8 - j;
            // Replicate each component's MSB into the LSBs revealed by the left-shift operation
            endpoints[i][k] |= endpoints[i][k] >> j as i32;
        }

        // Get alpha component precision including pbit
        let j = ACTUAL_BITS_COUNT[1][mode as usize] + ((MODE_HAS_P_BITS >> mode) & 1);

        // Alpha component
        endpoints[i][3] <<= 8 - j;
        endpoints[i][3] |= endpoints[i][3] >> j as i32;
    }

    // If this mode does not explicitly define the alpha component, set alpha to 255 (1.0)
    if ACTUAL_BITS_COUNT[1][mode as usize] == 0 {
        for endpoint in endpoints.iter_mut().take(num_endpoints) {
            endpoint[3] = 0xFF;
        }
    }

    // Determine weights tables
    let index_bits = match mode {
        0 | 1 => 3,
        6 => 4,
        _ => 2,
    };

    let index_bits2 = match mode {
        4 => 3,
        5 => 2,
        _ => 0,
    };

    let weights = match index_bits {
        2 => WEIGHT2,
        3 => WEIGHT3,
        _ => WEIGHT4,
    };

    let weights2 = match index_bits2 {
        2 => WEIGHT2,
        _ => WEIGHT3,
    };

    // Collect indices in two passes
    // Pass #1: collecting color indices
    let mut indices = [[0i32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            let partition_set = if num_partitions == 1 {
                if i | j == 0 { 128 } else { 0 }
            } else {
                PARTITION_SETS[num_partitions - 2][partition][i][j]
            };

            let mut idx_bits = match mode {
                0 | 1 => 3,
                6 => 4,
                _ => 2,
            };

            // Fix-up index is specified with one less bit
            // The fix-up index for subset 0 is always index 0
            if partition_set & 0x80 != 0 {
                idx_bits -= 1;
            }

            indices[i][j] = bstream.read_bits(idx_bits) as i32;
        }
    }

    // Pass #2: reading alpha indices (if any) and interpolating & rotating
    for i in 0..4 {
        for j in 0..4 {
            let partition_set = if num_partitions == 1 {
                if i | j == 0 { 128 } else { 0 }
            } else {
                PARTITION_SETS[num_partitions - 2][partition][i][j]
            };
            let partition_set = (partition_set & 0x03) as usize;

            let index = indices[i][j];

            let (mut r, mut g, mut b, mut a) = if index_bits2 == 0 {
                // No secondary index bits
                (
                    interpolate(
                        endpoints[partition_set * 2][0],
                        endpoints[partition_set * 2 + 1][0],
                        weights,
                        index,
                    ),
                    interpolate(
                        endpoints[partition_set * 2][1],
                        endpoints[partition_set * 2 + 1][1],
                        weights,
                        index,
                    ),
                    interpolate(
                        endpoints[partition_set * 2][2],
                        endpoints[partition_set * 2 + 1][2],
                        weights,
                        index,
                    ),
                    interpolate(
                        endpoints[partition_set * 2][3],
                        endpoints[partition_set * 2 + 1][3],
                        weights,
                        index,
                    ),
                )
            } else {
                let index2 = bstream.read_bits(if i | j == 0 {
                    index_bits2 - 1
                } else {
                    index_bits2
                }) as i32;

                if index_selection_bit == 0 {
                    (
                        interpolate(
                            endpoints[partition_set * 2][0],
                            endpoints[partition_set * 2 + 1][0],
                            weights,
                            index,
                        ),
                        interpolate(
                            endpoints[partition_set * 2][1],
                            endpoints[partition_set * 2 + 1][1],
                            weights,
                            index,
                        ),
                        interpolate(
                            endpoints[partition_set * 2][2],
                            endpoints[partition_set * 2 + 1][2],
                            weights,
                            index,
                        ),
                        interpolate(
                            endpoints[partition_set * 2][3],
                            endpoints[partition_set * 2 + 1][3],
                            weights2,
                            index2,
                        ),
                    )
                } else {
                    (
                        interpolate(
                            endpoints[partition_set * 2][0],
                            endpoints[partition_set * 2 + 1][0],
                            weights2,
                            index2,
                        ),
                        interpolate(
                            endpoints[partition_set * 2][1],
                            endpoints[partition_set * 2 + 1][1],
                            weights2,
                            index2,
                        ),
                        interpolate(
                            endpoints[partition_set * 2][2],
                            endpoints[partition_set * 2 + 1][2],
                            weights2,
                            index2,
                        ),
                        interpolate(
                            endpoints[partition_set * 2][3],
                            endpoints[partition_set * 2 + 1][3],
                            weights,
                            index,
                        ),
                    )
                }
            };

            // Handle rotation
            match rotation {
                1 => std::mem::swap(&mut a, &mut r), // 01 – Block format is Scalar(R) Vector(AGB) - swap A and R
                2 => std::mem::swap(&mut a, &mut g), // 10 – Block format is Scalar(G) Vector(RAB) - swap A and G
                3 => std::mem::swap(&mut a, &mut b), // 11 - Block format is Scalar(B) Vector(RGA) - swap A and B
                _ => {}
            }

            let offset = i * destination_pitch + j * 4;
            decompressed_block[offset] = r as u8;
            decompressed_block[offset + 1] = g as u8;
            decompressed_block[offset + 2] = b as u8;
            decompressed_block[offset + 3] = a as u8;
        }
    }
}

#[inline]
fn interpolate(a: i32, b: i32, weights: &[i32], index: i32) -> i32 {
    (a * (64 - weights[index as usize]) + b * weights[index as usize] + 32) >> 6
}

#[derive(Debug, Clone, Copy)]
struct BitStream {
    low: u64,
    high: u64,
}

impl BitStream {
    /// Create a new bitstream from raw data.
    #[inline]
    fn new(data: &[u8]) -> Self {
        let low = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let high = u64::from_le_bytes(data[8..16].try_into().unwrap());
        Self { low, high }
    }

    #[inline]
    fn read_bit(&mut self) -> u32 {
        self.read_bits(1)
    }

    #[inline]
    pub fn read_bits(&mut self, num_bits: u32) -> u32 {
        let mask = (1u64 << num_bits) - 1;
        // Read the low N bits.
        let bits = (self.low & mask) as u32;
        self.low >>= num_bits;

        // Put the low N bits of "high" into the high 64-N bits of "low".
        self.low |= (self.high & mask) << (64 - num_bits);
        self.high >>= num_bits;

        bits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_block(
        decode_block: fn(&[u8], &mut [u8], usize),
        pitch: usize,
        compressed_block: &[u8],
        expected_output: &[u8],
        name: &str,
    ) {
        let mut decoded = [0u8; 64];
        decode_block(compressed_block, &mut decoded, pitch);

        for y in 0..4 {
            let start = y * pitch;
            let end = start + pitch;
            assert_eq!(
                &decoded[start..end],
                &expected_output[start..end],
                "{}: Mismatch at row {}",
                name,
                y
            );
        }
    }

    #[test]
    fn test_bc7_block_0() {
        let compressed_block = [
            0x40, 0xAF, 0xF6, 0xB, 0xFD, 0x2E, 0xFF, 0xFF, 0x11, 0x71, 0x10, 0xA1, 0x21, 0xF2,
            0x33, 0x73,
        ];
        let expected_output = [
            0xBD, 0xBF, 0xBF, 0xFF, 0xBD, 0xBD, 0xBD, 0xFF, 0xBD, 0xBF, 0xBF, 0xFF, 0xBD, 0xBD,
            0xBD, 0xFF, 0xBD, 0xBD, 0xBD, 0xFF, 0xBC, 0xBB, 0xB9, 0xFF, 0xBB, 0xB9, 0xB7, 0xFF,
            0xBB, 0xB9, 0xB7, 0xFF, 0xBB, 0xB9, 0xB7, 0xFF, 0xB9, 0xB1, 0xAC, 0xFF, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0,
        ];
        test_block(
            decode_block_bc7,
            8,
            &compressed_block,
            &expected_output,
            "BC7 block 0",
        );
    }

    #[test]
    fn test_bc7_block_1() {
        let compressed_block = [
            0xC0, 0x8C, 0xEF, 0xA2, 0xBB, 0xDC, 0xFE, 0x7F, 0x6C, 0x55, 0x6A, 0x34, 0x4F, 0x0,
            0x5D, 0x0,
        ];
        let expected_output = [
            0x50, 0x4A, 0x48, 0xFE, 0x50, 0x4A, 0x48, 0xFE, 0x64, 0x5D, 0x59, 0xFE, 0x50, 0x4A,
            0x48, 0xFE, 0x7C, 0x74, 0x6E, 0xFE, 0x46, 0x41, 0x3F, 0xFE, 0x72, 0x6A, 0x65, 0xFE,
            0x4A, 0x45, 0x43, 0xFE, 0x32, 0x2E, 0x2E, 0xFE, 0x32, 0x2E, 0x2E, 0xFE, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0,
        ];
        test_block(
            decode_block_bc7,
            8,
            &compressed_block,
            &expected_output,
            "BC7 block 1",
        );
    }
}
