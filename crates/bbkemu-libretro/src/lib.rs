//! BBKEmu libretro core

use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use std::panic;
use std::path::PathBuf;

use bbkemu_core::{input::BbkKey, model, BbkModel, Emulator};

// libretro constants
const RETRO_API_VERSION: u32 = 1;
const RETRO_REGION_NTSC: u32 = 0;
const RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY: u32 = 9;
const RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS: u32 = 11;
const RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK: u32 = 12;

// RetroArch keyboard constants
const RETROK_RETURN: u32 = 13;
const RETROK_ESCAPE: u32 = 27;
const RETROK_SPACE: u32 = 32;
const RETROK_LEFT: u32 = 0x250;
const RETROK_UP: u32 = 0x251;
const RETROK_RIGHT: u32 = 0x252;
const RETROK_DOWN: u32 = 0x253;
#[allow(dead_code)]
const RETROK_A: u32 = 97;
#[allow(dead_code)]
const RETROK_Z: u32 = 122;
const RETROK_BACKSPACE: u32 = 8;
const RETROK_DELETE: u32 = 127;
const RETROK_PAGEUP: u32 = 0x254;
const RETROK_PAGEDOWN: u32 = 0x255;

// libretro input constants
const RETRO_DEVICE_JOYPAD: u32 = 1;
const RETRO_DEVICE_ID_JOYPAD_UP: u32 = 4;
const RETRO_DEVICE_ID_JOYPAD_DOWN: u32 = 5;
const RETRO_DEVICE_ID_JOYPAD_LEFT: u32 = 6;
const RETRO_DEVICE_ID_JOYPAD_RIGHT: u32 = 7;
const RETRO_DEVICE_ID_JOYPAD_A: u32 = 8;
const RETRO_DEVICE_ID_JOYPAD_B: u32 = 0;
const RETRO_DEVICE_ID_JOYPAD_X: u32 = 9;
const RETRO_DEVICE_ID_JOYPAD_Y: u32 = 1;
const RETRO_DEVICE_ID_JOYPAD_L: u32 = 10;
const RETRO_DEVICE_ID_JOYPAD_R: u32 = 11;
const RETRO_DEVICE_ID_JOYPAD_SELECT: u32 = 2;
const RETRO_DEVICE_ID_JOYPAD_START: u32 = 3;

// Type aliases for libretro callbacks
type RetroEnvironmentT = Option<unsafe extern "C" fn(cmd: u32, data: *mut c_void) -> bool>;
type RetroVideoRefreshT =
    Option<unsafe extern "C" fn(data: *const c_void, width: u32, height: u32, pitch: usize)>;
type RetroAudioSampleT = Option<unsafe extern "C" fn(left: i16, right: i16)>;
type RetroAudioSampleBatchT =
    Option<unsafe extern "C" fn(data: *const i16, frames: usize) -> usize>;
type RetroInputPollT = Option<unsafe extern "C" fn()>;
type RetroInputStateT =
    Option<unsafe extern "C" fn(port: u32, device: u32, index: u32, id: u32) -> i16>;
type RetroKeyboardCallbackT =
    Option<unsafe extern "C" fn(down: bool, keycode: u32, character: u32, key_modifiers: u16)>;

// libretro structs
#[repr(C)]
pub struct RetroSystemInfo {
    library_name: *const c_char,
    library_version: *const c_char,
    valid_extensions: *const c_char,
    need_fullpath: bool,
    block_extract: bool,
}

#[repr(C)]
pub struct RetroGameGeometry {
    base_width: u32,
    base_height: u32,
    max_width: u32,
    max_height: u32,
    aspect_ratio: f32,
}

#[repr(C)]
pub struct RetroSystemTiming {
    fps: f64,
    sample_rate: f64,
}

#[repr(C)]
pub struct RetroSystemAvInfo {
    geometry: RetroGameGeometry,
    timing: RetroSystemTiming,
}

#[repr(C)]
pub struct RetroGameInfo {
    path: *const c_char,
    data: *const c_void,
    size: usize,
    meta: *const c_char,
}

/// Global emulator instance
static mut EMULATOR: Option<Emulator> = None;

/// Framebuffer for rendering (RGB565 format)
static mut FRAMEBUFFER: [u16; 159 * 96] = [0; 159 * 96];

/// Environment callback for querying system directory
static mut ENVIRONMENT_CB: RetroEnvironmentT = None;

/// Video callback for sending frames to frontend
static mut VIDEO_CB: RetroVideoRefreshT = None;

/// Audio callback for sending samples to frontend
static mut AUDIO_CB: RetroAudioSampleT = None;

/// Audio batch callback for sending sample buffers to frontend
static mut AUDIO_BATCH_CB: RetroAudioSampleBatchT = None;

/// Input poll callback
static mut INPUT_POLL_CB: RetroInputPollT = None;

/// Input state callback
static mut INPUT_STATE_CB: RetroInputStateT = None;

/// Keyboard callback
#[allow(dead_code)]
static mut KEYBOARD_CB: RetroKeyboardCallbackT = None;

/// Keyboard callback struct for RetroArch
#[repr(C)]
struct RetroKeyboardCallback {
    callback: RetroKeyboardCallbackT,
}

/// Get the system directory from RetroArch
fn get_system_directory() -> Option<PathBuf> {
    unsafe {
        let cb = ENVIRONMENT_CB?;
        let mut dir: *const c_char = std::ptr::null();
        if cb(
            RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY,
            &mut dir as *mut _ as *mut c_void,
        ) && !dir.is_null()
        {
            let cstr = CStr::from_ptr(dir);
            return Some(PathBuf::from(cstr.to_str().ok()?));
        }
        None
    }
}

/// Try to load ROM files for the given model
fn load_roms_for_model(emu: &mut Emulator, model: &BbkModel) {
    let Some(system_dir) = get_system_directory() else {
        return;
    };

    let model_name = model.name;
    let rom_dir = system_dir.join("BBKEmu").join(model_name);

    // Load font ROM (8.BIN)
    let rom8_path = rom_dir.join("8.BIN");
    if let Ok(data) = std::fs::read(&rom8_path) {
        emu.load_rom_8(&data);
    }

    // Load OS ROM (E.BIN)
    let rom_e_path = rom_dir.join("E.BIN");
    if let Ok(data) = std::fs::read(&rom_e_path) {
        emu.load_rom_e(&data);
    }
}

#[no_mangle]
pub extern "C" fn retro_api_version() -> u32 {
    RETRO_API_VERSION
}

#[no_mangle]
pub extern "C" fn retro_init() {
    // Set up panic handler to prevent UB from unwinding across FFI boundary
    panic::set_hook(Box::new(|info| {
        eprintln!("BBKEmu panic: {}", info);
    }));

    let result = panic::catch_unwind(|| unsafe {
        EMULATOR = Some(Emulator::new(&model::MODEL_4980));
    });

    if result.is_err() {
        eprintln!("BBKEmu: panic during retro_init");
    }
}

#[no_mangle]
pub extern "C" fn retro_deinit() {
    unsafe {
        EMULATOR = None;
    }
}

#[no_mangle]
pub extern "C" fn retro_set_environment(cb: RetroEnvironmentT) {
    unsafe {
        ENVIRONMENT_CB = cb;

        // Set keyboard callback
        let kbd = RetroKeyboardCallback {
            callback: Some(keyboard_callback),
        };
        if let Some(env_cb) = cb {
            env_cb(
                RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK,
                &kbd as *const _ as *mut c_void,
            );
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_set_video_refresh(cb: RetroVideoRefreshT) {
    unsafe {
        VIDEO_CB = cb;
    }
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample(cb: RetroAudioSampleT) {
    unsafe {
        AUDIO_CB = cb;
    }
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample_batch(cb: RetroAudioSampleBatchT) {
    unsafe {
        AUDIO_BATCH_CB = cb;
    }
}

#[no_mangle]
pub extern "C" fn retro_set_input_poll(cb: RetroInputPollT) {
    unsafe {
        INPUT_POLL_CB = cb;
    }
}

#[no_mangle]
pub extern "C" fn retro_set_input_state(cb: RetroInputStateT) {
    unsafe {
        INPUT_STATE_CB = cb;
    }
}

/// Map RetroArch keyboard key to BBK key
fn map_keyboard_to_bbk_key(keycode: u32) -> Option<BbkKey> {
    match keycode {
        RETROK_RETURN => Some(BbkKey::Enter),
        RETROK_ESCAPE => Some(BbkKey::Exit),
        RETROK_SPACE => Some(BbkKey::Space),
        RETROK_LEFT => Some(BbkKey::Left),
        RETROK_RIGHT => Some(BbkKey::Right),
        RETROK_UP => Some(BbkKey::Up),
        RETROK_DOWN => Some(BbkKey::Down),
        RETROK_BACKSPACE | RETROK_DELETE => Some(BbkKey::Del),
        RETROK_PAGEUP => Some(BbkKey::PgUp),
        RETROK_PAGEDOWN => Some(BbkKey::PgDn),
        _ => None,
    }
}

/// Keyboard callback function
unsafe extern "C" fn keyboard_callback(
    down: bool,
    keycode: u32,
    _character: u32,
    _key_modifiers: u16,
) {
    if !down {
        return;
    }

    if let Some(ref mut emu) = EMULATOR {
        if let Some(bbk_key) = map_keyboard_to_bbk_key(keycode) {
            emu.key_down(bbk_key);
        }
    }
}

/// # Safety
///
/// `info` must be a valid pointer to a `RetroSystemInfo` struct.
#[no_mangle]
pub unsafe extern "C" fn retro_get_system_info(info: *mut RetroSystemInfo) {
    (*info).library_name = c"BBKEmu".as_ptr();
    (*info).library_version = c"0.1.0".as_ptr();
    (*info).valid_extensions = c"gam".as_ptr();
    (*info).need_fullpath = false;
    (*info).block_extract = false;
}

/// # Safety
///
/// `info` must be a valid pointer to a `RetroSystemAvInfo` struct.
#[no_mangle]
pub unsafe extern "C" fn retro_get_system_av_info(info: *mut RetroSystemAvInfo) {
    (*info).geometry.base_width = 159;
    (*info).geometry.base_height = 96;
    (*info).geometry.max_width = 159;
    (*info).geometry.max_height = 96;
    (*info).geometry.aspect_ratio = 0.0;
    (*info).timing.fps = 60.0;
    (*info).timing.sample_rate = 44100.0;
}

/// Input descriptor for RetroArch
#[repr(C)]
struct RetroInputDescriptor {
    port: u32,
    device: u32,
    index: u32,
    id: u32,
    description: *const c_char,
}

/// # Safety
///
/// `info` must be a valid pointer to a `RetroGameInfo` struct.
/// `(*info).data` must be a valid pointer to `(*info).size` bytes of game data.
#[no_mangle]
pub unsafe extern "C" fn retro_load_game(info: *const RetroGameInfo) -> bool {
    let result = panic::catch_unwind(|| {
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
            // Load ROMs before loading the game
            let bbk_model = &model::MODEL_4980;
            load_roms_for_model(emu, bbk_model);

            // Set input descriptors
            let inputs = [
                RetroInputDescriptor {
                    port: 0,
                    device: RETRO_DEVICE_JOYPAD,
                    index: 0,
                    id: RETRO_DEVICE_ID_JOYPAD_B,
                    description: c"EXIT".as_ptr(),
                },
                RetroInputDescriptor {
                    port: 0,
                    device: RETRO_DEVICE_JOYPAD,
                    index: 0,
                    id: RETRO_DEVICE_ID_JOYPAD_Y,
                    description: c"HELP".as_ptr(),
                },
                RetroInputDescriptor {
                    port: 0,
                    device: RETRO_DEVICE_JOYPAD,
                    index: 0,
                    id: RETRO_DEVICE_ID_JOYPAD_SELECT,
                    description: c"INSERT".as_ptr(),
                },
                RetroInputDescriptor {
                    port: 0,
                    device: RETRO_DEVICE_JOYPAD,
                    index: 0,
                    id: RETRO_DEVICE_ID_JOYPAD_START,
                    description: c"SEARCH".as_ptr(),
                },
                RetroInputDescriptor {
                    port: 0,
                    device: RETRO_DEVICE_JOYPAD,
                    index: 0,
                    id: RETRO_DEVICE_ID_JOYPAD_UP,
                    description: c"UP".as_ptr(),
                },
                RetroInputDescriptor {
                    port: 0,
                    device: RETRO_DEVICE_JOYPAD,
                    index: 0,
                    id: RETRO_DEVICE_ID_JOYPAD_DOWN,
                    description: c"DOWN".as_ptr(),
                },
                RetroInputDescriptor {
                    port: 0,
                    device: RETRO_DEVICE_JOYPAD,
                    index: 0,
                    id: RETRO_DEVICE_ID_JOYPAD_LEFT,
                    description: c"LEFT".as_ptr(),
                },
                RetroInputDescriptor {
                    port: 0,
                    device: RETRO_DEVICE_JOYPAD,
                    index: 0,
                    id: RETRO_DEVICE_ID_JOYPAD_RIGHT,
                    description: c"RIGHT".as_ptr(),
                },
                RetroInputDescriptor {
                    port: 0,
                    device: RETRO_DEVICE_JOYPAD,
                    index: 0,
                    id: RETRO_DEVICE_ID_JOYPAD_A,
                    description: c"ENTER".as_ptr(),
                },
                RetroInputDescriptor {
                    port: 0,
                    device: RETRO_DEVICE_JOYPAD,
                    index: 0,
                    id: RETRO_DEVICE_ID_JOYPAD_X,
                    description: c"R".as_ptr(),
                },
                RetroInputDescriptor {
                    port: 0,
                    device: RETRO_DEVICE_JOYPAD,
                    index: 0,
                    id: RETRO_DEVICE_ID_JOYPAD_L,
                    description: c"PGUP".as_ptr(),
                },
                RetroInputDescriptor {
                    port: 0,
                    device: RETRO_DEVICE_JOYPAD,
                    index: 0,
                    id: RETRO_DEVICE_ID_JOYPAD_R,
                    description: c"PGDN".as_ptr(),
                },
                RetroInputDescriptor {
                    port: 0,
                    device: 0,
                    index: 0,
                    id: 0,
                    description: std::ptr::null(),
                },
            ];

            if let Some(env_cb) = ENVIRONMENT_CB {
                env_cb(
                    RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS,
                    inputs.as_ptr() as *mut c_void,
                );
            }

            emu.load_gam(game_data).is_ok()
        } else {
            false
        }
    });

    match result {
        Ok(val) => val,
        Err(_) => {
            eprintln!("BBKEmu: panic during retro_load_game");
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_unload_game() {}

/// Joypad button to BBK key mapping
const JOYK: [BbkKey; 16] = [
    BbkKey::Exit,   // RETRO_DEVICE_ID_JOYPAD_B (0) -> EXIT
    BbkKey::Help,   // RETRO_DEVICE_ID_JOYPAD_Y (1) -> HELP
    BbkKey::Insert, // RETRO_DEVICE_ID_JOYPAD_SELECT (2) -> INSERT
    BbkKey::Search, // RETRO_DEVICE_ID_JOYPAD_START (3) -> SEARCH
    BbkKey::Up,     // RETRO_DEVICE_ID_JOYPAD_UP (4) -> UP
    BbkKey::Down,   // RETRO_DEVICE_ID_JOYPAD_DOWN (5) -> DOWN
    BbkKey::Left,   // RETRO_DEVICE_ID_JOYPAD_LEFT (6) -> LEFT
    BbkKey::Right,  // RETRO_DEVICE_ID_JOYPAD_RIGHT (7) -> RIGHT
    BbkKey::Enter,  // RETRO_DEVICE_ID_JOYPAD_A (8) -> ENTER
    BbkKey::R,      // RETRO_DEVICE_ID_JOYPAD_X (9) -> R
    BbkKey::PgUp,   // RETRO_DEVICE_ID_JOYPAD_L (10) -> PGUP
    BbkKey::PgDn,   // RETRO_DEVICE_ID_JOYPAD_R (11) -> PGDN
    BbkKey::Modify, // RETRO_DEVICE_ID_JOYPAD_L2 (12) -> MODIFY
    BbkKey::Del,    // RETRO_DEVICE_ID_JOYPAD_R2 (13) -> DEL
    BbkKey::A,      // RETRO_DEVICE_ID_JOYPAD_L3 (14) -> A
    BbkKey::Z,      // RETRO_DEVICE_ID_JOYPAD_R3 (15) -> Z
];

/// Static state for joypad input handling
static mut JOYPAD_PRESSED: i32 = -1;
static mut JOYPAD_REPEAT: i32 = 0;

#[no_mangle]
pub extern "C" fn retro_run() {
    let _ = panic::catch_unwind(|| {
        unsafe {
            if let Some(ref mut emu) = EMULATOR {
                // 1. Poll input
                if let Some(poll_cb) = INPUT_POLL_CB {
                    poll_cb();
                }

                // 2. Handle joypad input
                if let Some(input_cb) = INPUT_STATE_CB {
                    // Check if previously pressed button is still held
                    if JOYPAD_PRESSED >= 0 {
                        if input_cb(0, RETRO_DEVICE_JOYPAD, 0, JOYPAD_PRESSED as u32) == 0 {
                            // Button released
                            JOYPAD_PRESSED = -1;
                            JOYPAD_REPEAT = 0;
                        } else {
                            // Button still held - handle repeat
                            JOYPAD_REPEAT += 1;
                            if JOYPAD_REPEAT > 20 {
                                JOYPAD_REPEAT -= 5;
                                emu.key_down(JOYK[JOYPAD_PRESSED as usize]);
                            }
                        }
                    }

                    // Check for new button press
                    if JOYPAD_PRESSED == -1 {
                        for i in 0..16 {
                            if input_cb(0, RETRO_DEVICE_JOYPAD, 0, i) != 0 {
                                // New button pressed
                                JOYPAD_PRESSED = i as i32;
                                JOYPAD_REPEAT = 0;
                                emu.key_down(JOYK[i as usize]);
                                break;
                            }
                        }
                    }
                }

                // 3. Run emulation frame
                emu.run_frame();

                // 4. Render LCD
                #[allow(static_mut_refs)]
                emu.render_lcd(&mut FRAMEBUFFER, false);

                // 5. Send video frame to frontend
                if let Some(video_cb) = VIDEO_CB {
                    #[allow(static_mut_refs)]
                    video_cb(
                        FRAMEBUFFER.as_ptr() as *const c_void,
                        159,
                        96,
                        159 * 2, // pitch = width * bytes_per_pixel (RGB565 = 2 bytes)
                    );
                }
            }
        }
    });
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
