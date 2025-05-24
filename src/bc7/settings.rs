use bytemuck::{Pod, Zeroable};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct BC7Settings {
    pub refine_iterations: [u32; 8],
    pub mode_selection: [u32; 4],
    pub skip_mode2: u32,
    pub fast_skip_threshold_mode1: u32,
    pub fast_skip_threshold_mode3: u32,
    pub fast_skip_threshold_mode7: u32,
    pub mode45_channel0: u32,
    pub refine_iterations_channel: u32,
    pub channels: u32,
}

impl BC7Settings {
    /// Opaque ultra fast settings.
    pub const fn opaque_ultra_fast() -> Self {
        Self {
            channels: 3,
            mode_selection: [false as _, false as _, false as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 3,
            fast_skip_threshold_mode3: 1,
            fast_skip_threshold_mode7: 0,
            mode45_channel0: 0,
            refine_iterations_channel: 0,
            refine_iterations: [2, 2, 2, 1, 2, 2, 1, 0],
        }
    }

    /// Opaque very fast settings.
    pub const fn opaque_very_fast() -> Self {
        Self {
            channels: 3,
            mode_selection: [false as _, true as _, false as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 3,
            fast_skip_threshold_mode3: 1,
            fast_skip_threshold_mode7: 0,
            mode45_channel0: 0,
            refine_iterations_channel: 0,
            refine_iterations: [2, 2, 2, 1, 2, 2, 1, 0],
        }
    }

    /// Opaque fast settings.
    pub const fn opaque_fast() -> Self {
        Self {
            channels: 3,
            mode_selection: [false as _, true as _, false as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 12,
            fast_skip_threshold_mode3: 4,
            fast_skip_threshold_mode7: 0,
            mode45_channel0: 0,
            refine_iterations_channel: 0,
            refine_iterations: [2, 2, 2, 1, 2, 2, 2, 0],
        }
    }

    /// Opaque basic settings.
    pub const fn opaque_basic() -> Self {
        Self {
            channels: 3,
            mode_selection: [true as _, true as _, true as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 12,
            fast_skip_threshold_mode3: 8,
            fast_skip_threshold_mode7: 0,
            mode45_channel0: 0,
            refine_iterations_channel: 2,
            refine_iterations: [2, 2, 2, 2, 2, 2, 2, 0],
        }
    }

    /// Opaque slow settings.
    pub const fn opaque_slow() -> Self {
        Self {
            channels: 3,
            mode_selection: [true as _, true as _, true as _, true as _],
            skip_mode2: false as _,
            fast_skip_threshold_mode1: 64,
            fast_skip_threshold_mode3: 64,
            fast_skip_threshold_mode7: 0,
            mode45_channel0: 0,
            refine_iterations_channel: 4,
            refine_iterations: [4, 4, 4, 4, 4, 4, 4, 0],
        }
    }

    /// Alpha ultra fast settings.
    pub const fn alpha_ultrafast() -> Self {
        Self {
            channels: 4,
            mode_selection: [false as _, false as _, true as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 0,
            fast_skip_threshold_mode3: 0,
            fast_skip_threshold_mode7: 4,
            mode45_channel0: 3,
            refine_iterations_channel: 1,
            refine_iterations: [2, 1, 2, 1, 1, 1, 2, 2],
        }
    }

    /// Alpha very fast settings.
    pub const fn alpha_very_fast() -> Self {
        Self {
            channels: 4,
            mode_selection: [false as _, true as _, true as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 0,
            fast_skip_threshold_mode3: 0,
            fast_skip_threshold_mode7: 4,
            mode45_channel0: 3,
            refine_iterations_channel: 2,
            refine_iterations: [2, 1, 2, 1, 2, 2, 2, 2],
        }
    }

    /// Alpha fast settings.
    pub const fn alpha_fast() -> Self {
        Self {
            channels: 4,
            mode_selection: [false as _, true as _, true as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 4,
            fast_skip_threshold_mode3: 4,
            fast_skip_threshold_mode7: 8,
            mode45_channel0: 3,
            refine_iterations_channel: 2,
            refine_iterations: [2, 1, 2, 1, 2, 2, 2, 2],
        }
    }

    /// Alpha basic settings.
    pub const fn alpha_basic() -> Self {
        Self {
            channels: 4,
            mode_selection: [true as _, true as _, true as _, true as _],
            skip_mode2: true as _,
            fast_skip_threshold_mode1: 12,
            fast_skip_threshold_mode3: 8,
            fast_skip_threshold_mode7: 8,
            mode45_channel0: 0,
            refine_iterations_channel: 2,
            refine_iterations: [2, 2, 2, 2, 2, 2, 2, 2],
        }
    }

    /// Alpha slow settings.
    pub const fn alpha_slow() -> Self {
        Self {
            channels: 4,
            mode_selection: [true as _, true as _, true as _, true as _],
            skip_mode2: false as _,
            fast_skip_threshold_mode1: 64,
            fast_skip_threshold_mode3: 64,
            fast_skip_threshold_mode7: 64,
            mode45_channel0: 0,
            refine_iterations_channel: 4,
            refine_iterations: [4, 4, 4, 4, 4, 4, 4, 4],
        }
    }
}
