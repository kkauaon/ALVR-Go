//! Fixed identity and tuning the driver reports to SteamVR. This fork only supports the Oculus Go,
//! so what used to be user settings (headset and controller emulation, haptics tuning, 10-bit
//! encoding, ...) is constant. The values are the ones that were the defaults of those settings.

pub const UNIVERSE_ID: u64 = 2;
pub const HEADSET_SERIAL_NUMBER: &str = "1WMGH000XX0000";
pub const HEADSET_TRACKING_SYSTEM_NAME: &str = "oculus";
pub const HEADSET_MODEL_NUMBER: &str = "Miramar";
pub const HEADSET_DRIVER_VERSION: &str = "1.55.0";
pub const HEADSET_MANUFACTURER_NAME: &str = "Oculus";
pub const HEADSET_RENDER_MODEL_NAME: &str = "generic_hmd";
pub const HEADSET_REGISTERED_DEVICE_TYPE: &str = "oculus/1WMGH000XX0000";

pub const CONTROLLERS_TRACKING_SYSTEM_NAME: &str = "oculus";
pub const CONTROLLERS_MANUFACTURER_NAME: &str = "Oculus";
pub const CONTROLLERS_MODEL_NUMBER: &str = "Miramar";
pub const CONTROLLERS_RENDER_MODEL_NAME_LEFT: &str = "oculus_quest2_controller_left";
pub const CONTROLLERS_RENDER_MODEL_NAME_RIGHT: &str = "oculus_quest2_controller_right";
pub const CONTROLLERS_SERIAL_NUMBER: &str = "1WMGH000XX0000_Controller";
pub const CONTROLLERS_TYPE_LEFT: &str = "oculus_touch";
pub const CONTROLLERS_TYPE_RIGHT: &str = "oculus_touch";
pub const CONTROLLERS_REGISTERED_DEVICE_TYPE: &str = "oculus/1WMGH000XX0000_Controller";
pub const CONTROLLERS_INPUT_PROFILE_PATH: &str = "{oculus}/input/touch_profile.json";
pub const CONTROLLERS_MODE_IDX: i32 = 7;

// The Go controller has no haptics motor: these only shape feedback that is never played.
pub const HAPTICS_INTENSITY: f32 = 1.;
pub const HAPTICS_AMPLITUDE_CURVE: f32 = 0.4;
pub const HAPTICS_MIN_DURATION: f32 = 0.01;
pub const HAPTICS_LOW_DURATION_AMPLITUDE_MULTIPLIER: f32 = 2.5;
pub const HAPTICS_LOW_DURATION_RANGE: f32 = 0.5;

/// The Go decodes 8-bit video only.
pub const USE_10BIT_ENCODER: bool = false;
/// The Go always asks MediaCodec for its low latency, real-time decoder.
pub const CLIENT_REQUEST_REALTIME_DECODER: bool = true;
/// Extra latency mode is a Quest workaround (see the client renderer).
pub const EXTRA_LATENCY_MODE: bool = false;
