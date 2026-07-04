//! BBKEmu libretro core

use std::os::raw::{c_char, c_void};

use bbkemu_core::{Emulator, model};

// libretro constants
const RETRO_API_VERSION: u32 = 1;
const RETRO_REGION_NTSC: u32 = 0;

// Type aliases for libretro callbacks
type RetroEnvironmentT = Option<unsafe extern "C" fn(cmd: u32, data: *mut c_void) -> bool>;
type RetroVideoRefreshT = Option<unsafe extern "C" fn(data: *const c_void, width: u32, height: u32, pitch: usize)>;
type RetroAudioSampleT = Option<unsafe extern "C" fn(left: i16, right: i16)>;
type RetroInputPollT = Option<unsafe extern "C" fn()>;
type RetroInputStateT = Option<unsafe extern "C" fn(port: u32, device: u32, index: u32, id: u32) -> i16>;

// libretro structs
#[repr(C)]
struct RetroSystemInfo {
    library_name: *const c_char,
    library_version: *const c_char,
    valid_extensions: *const c_char,
    need_fullpath: bool,
    block_extract: bool,
}

#[repr(C)]
struct RetroGameGeometry {
    base_width: u32,
    base_height: u32,
    max_width: u32,
    max_height: u32,
    aspect_ratio: f32,
}

#[repr(C)]
struct RetroSystemTiming {
    fps: f64,
    sample_rate: f64,
}

#[repr(C)]
struct RetroSystemAvInfo {
    geometry: RetroGameGeometry,
    timing: RetroSystemTiming,
}

#[repr(C)]
struct RetroGameInfo {
    path: *const c_char,
    data: *const c_void,
    size: usize,
    meta: *const c_char,
}

/// Global emulator instance
static mut EMULATOR: Option<Emulator> = None;

/// Framebuffer for rendering
static mut FRAMEBUFFER: [u16; 159 * 96] = [0; 159 * 96];

#[no_mangle]
pub extern "C" fn retro_api_version() -> u32 {
    RETRO_API_VERSION
}

#[no_mangle]
pub extern "C" fn retro_init() {
    unsafe {
        EMULATOR = Some(Emulator::new(&model::MODEL_4980));
    }
    log::info!("BBKEmu libretro core initialized");
}

#[no_mangle]
pub extern "C" fn retro_deinit() {
    unsafe {
        EMULATOR = None;
    }
    log::info!("BBKEmu libretro core deinitialized");
}

#[no_mangle]
pub extern "C" fn retro_set_environment(_cb: RetroEnvironmentT) {
    // TODO: Store callback
}

#[no_mangle]
pub extern "C" fn retro_set_video_refresh(_cb: RetroVideoRefreshT) {
    // TODO: Store callback
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample(_cb: RetroAudioSampleT) {
    // TODO: Store callback
}

#[no_mangle]
pub extern "C" fn retro_set_input_poll(_cb: RetroInputPollT) {
    // TODO: Store callback
}

#[no_mangle]
pub extern "C" fn retro_set_input_state(_cb: RetroInputStateT) {
    // TODO: Store callback
}

#[no_mangle]
pub extern "C" fn retro_get_system_info(info: *mut RetroSystemInfo) {
    unsafe {
        (*info).library_name = b"BBKEmu\0".as_ptr() as *const c_char;
        (*info).library_version = b"0.1.0\0".as_ptr() as *const c_char;
        (*info).valid_extensions = b"gam\0".as_ptr() as *const c_char;
        (*info).need_fullpath = false;
        (*info).block_extract = false;
    }
}

#[no_mangle]
pub extern "C" fn retro_get_system_av_info(info: *mut RetroSystemAvInfo) {
    unsafe {
        (*info).geometry.base_width = 159;
        (*info).geometry.base_height = 96;
        (*info).geometry.max_width = 159;
        (*info).geometry.max_height = 96;
        (*info).geometry.aspect_ratio = 0.0;
        (*info).timing.fps = 60.0;
        (*info).timing.sample_rate = 44100.0;
    }
}

#[no_mangle]
pub extern "C" fn retro_load_game(info: *const RetroGameInfo) -> bool {
    unsafe {
        if info.is_null() {
            return false;
        }
        let data = (*info).data as *const u8;
        let size = (*info).size;
        if data.is_null() || size == 0 {
            return false;
        }
        let game_data = std::slice::from_raw_parts(data, size);

        if let Some(ref mut emu) = EMULATOR {
            match emu.load_gam(game_data) {
                Ok(()) => {
                    log::info!("Game loaded successfully");
                    true
                }
                Err(e) => {
                    log::error!("Failed to load game: {}", e);
                    false
                }
            }
        } else {
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_unload_game() {}

#[no_mangle]
pub extern "C" fn retro_run() {
    unsafe {
        if let Some(ref mut emu) = EMULATOR {
            emu.run_frame();
            emu.render_lcd(&mut FRAMEBUFFER, false);
            // TODO: Send video frame via callback
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_serialize_size() -> usize {
    1024 * 1024
}

#[no_mangle]
pub extern "C" fn retro_serialize(_data: *mut c_void, _size: usize) -> bool {
    false
}

#[no_mangle]
pub extern "C" fn retro_unserialize(_data: *const c_void, _size: usize) -> bool {
    false
}

#[no_mangle]
pub extern "C" fn retro_reset() {}

#[no_mangle]
pub extern "C" fn retro_cheat_reset() {}

#[no_mangle]
pub extern "C" fn retro_cheat_set(_index: u32, _enabled: bool, _code: *const c_char) {}

#[no_mangle]
pub extern "C" fn retro_load_game_special(
    _game_type: u32,
    _info: *const RetroGameInfo,
    _num_info: usize,
) -> bool {
    false
}

#[no_mangle]
pub extern "C" fn retro_set_controller_port_device(_port: u32, _device: u32) {}

#[no_mangle]
pub extern "C" fn retro_get_region() -> u32 {
    RETRO_REGION_NTSC
}

#[no_mangle]
pub extern "C" fn retro_get_memory_data(_id: u32) -> *mut c_void {
    std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn retro_get_memory_size(_id: u32) -> usize {
    0
}
