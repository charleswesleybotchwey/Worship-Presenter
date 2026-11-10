//! Minimal binding to the NDI Runtime (Processing.NDI.Lib), loaded dynamically.
//! Struct layouts follow Processing.NDI.structs.h / Processing.NDI.Send.h (NDI 5/6).

use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::PathBuf;

#[repr(C)]
struct SendCreate {
    p_ndi_name: *const c_char,
    p_groups: *const c_char,
    clock_video: bool,
    clock_audio: bool,
}

#[repr(C)]
struct VideoFrameV2 {
    xres: c_int,
    yres: c_int,
    fourcc: u32,
    frame_rate_n: c_int,
    frame_rate_d: c_int,
    picture_aspect_ratio: f32,
    frame_format_type: c_int,
    timecode: i64,
    p_data: *const u8,
    line_stride_in_bytes: c_int,
    p_metadata: *const c_char,
    timestamp: i64,
}

/// BGRA with straight (not premultiplied) alpha.
const FOURCC_BGRA: u32 = u32::from_le_bytes(*b"BGRA");
const FORMAT_PROGRESSIVE: c_int = 1;
const TIMECODE_SYNTHESIZE: i64 = i64::MAX;

type FnInit = unsafe extern "C" fn() -> bool;
type FnVersion = unsafe extern "C" fn() -> *const c_char;
type FnSendCreate = unsafe extern "C" fn(*const SendCreate) -> *mut c_void;
type FnSendDestroy = unsafe extern "C" fn(*mut c_void);
type FnSendVideo = unsafe extern "C" fn(*mut c_void, *const VideoFrameV2);
type FnConnections = unsafe extern "C" fn(*mut c_void, u32) -> c_int;

/// The loaded runtime. Keep it alive for as long as any sender exists.
pub struct Runtime {
    _lib: Library,
    pub path: PathBuf,
    pub version: String,
    send_create: FnSendCreate,
    send_destroy: FnSendDestroy,
    send_video: FnSendVideo,
    connections: FnConnections,
}

/// Where the official NDI Runtime installers put the library.
pub fn candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(windows)]
    let name = "Processing.NDI.Lib.x64.dll";
    #[cfg(target_os = "macos")]
    let name = "libndi.dylib";
    #[cfg(all(unix, not(target_os = "macos")))]
    let name = "libndi.so.6";
    for var in ["NDI_RUNTIME_DIR_V6", "NDI_RUNTIME_DIR_V5"] {
        if let Some(d) = std::env::var_os(var) {
            v.push(PathBuf::from(d).join(name));
        }
    }
    #[cfg(target_os = "macos")]
    v.push(PathBuf::from("/usr/local/lib/libndi.dylib"));
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        v.push(PathBuf::from("/usr/lib/libndi.so.6"));
        v.push(PathBuf::from("/usr/local/lib/libndi.so.6"));
        v.push(PathBuf::from("/usr/lib/x86_64-linux-gnu/libndi.so.5"));
    }
    v.push(PathBuf::from(name)); // system search path last
    v
}

impl Runtime {
    /// Loads the first runtime found. The error says what to install.
    pub fn load() -> Result<Runtime, String> {
        let mut tried = Vec::new();
        for p in candidates() {
            // SAFETY: loading a shared library runs its initialisers; this is the
            // vendor's runtime installed by its own installer.
            match unsafe { Library::new(&p) } {
                Ok(lib) => return Self::bind(lib, p),
                Err(e) => tried.push(format!("{}: {e}", p.display())),
            }
        }
        Err(format!(
            "The NDI Runtime is not installed. Install “NDI Tools” or the NDI Runtime from ndi.video, then restart Worship Presenter. (Looked in: {})",
            tried.iter().map(|t| t.split(':').next().unwrap_or("")).collect::<Vec<_>>().join(", ")
        ))
    }

    fn bind(lib: Library, path: PathBuf) -> Result<Runtime, String> {
        unsafe {
            let sym = |n: &[u8]| -> Result<*mut c_void, String> {
                lib.get::<*mut c_void>(n).map(|s| *s).map_err(|e| format!("{} is not a usable NDI Runtime ({e})", path.display()))
            };
            let init: FnInit = std::mem::transmute(sym(b"NDIlib_initialize\0")?);
            if !init() {
                return Err("The NDI Runtime refused to start: this processor is not supported by NDI.".into());
            }
            let version: FnVersion = std::mem::transmute(sym(b"NDIlib_version\0")?);
            let v = version();
            let version = if v.is_null() { "unknown".into() } else { CStr::from_ptr(v).to_string_lossy().into_owned() };
            Ok(Runtime {
                send_create: std::mem::transmute(sym(b"NDIlib_send_create\0")?),
                send_destroy: std::mem::transmute(sym(b"NDIlib_send_destroy\0")?),
                send_video: std::mem::transmute(sym(b"NDIlib_send_send_video_v2\0")?),
                connections: std::mem::transmute(sym(b"NDIlib_send_get_no_connections\0")?),
                version,
                path,
                _lib: lib,
            })
        }
    }

    pub fn sender(self: &std::sync::Arc<Self>, name: &str) -> Result<Sender, String> {
        let c = CString::new(name).map_err(|_| "The NDI name cannot contain a zero byte.".to_string())?;
        // clock_video: the runtime paces send calls to the frame rate we give it
        let desc = SendCreate { p_ndi_name: c.as_ptr(), p_groups: std::ptr::null(), clock_video: true, clock_audio: false };
        let inst = unsafe { (self.send_create)(&desc) };
        if inst.is_null() {
            return Err(format!("NDI could not create the source “{name}”. Is the name already in use on this computer?"));
        }
        Ok(Sender { rt: self.clone(), inst, _name: c })
    }
}

/// One NDI source. Dropping it removes the source from the network.
pub struct Sender {
    rt: std::sync::Arc<Runtime>,
    inst: *mut c_void,
    _name: CString,
}

// The NDI send instance may be used from one thread at a time; the pump owns it.
unsafe impl Send for Sender {}

impl Sender {
    /// Sends one BGRA frame (straight alpha). Blocks to keep the frame rate.
    pub fn send_bgra(&self, width: u32, height: u32, stride: u32, data: &[u8], fps: u32) {
        assert!(data.len() >= (stride * height) as usize, "frame buffer too small");
        let f = VideoFrameV2 {
            xres: width as c_int,
            yres: height as c_int,
            fourcc: FOURCC_BGRA,
            frame_rate_n: fps as c_int * 1000,
            frame_rate_d: 1000,
            picture_aspect_ratio: width as f32 / height as f32,
            frame_format_type: FORMAT_PROGRESSIVE,
            timecode: TIMECODE_SYNTHESIZE,
            p_data: data.as_ptr(),
            line_stride_in_bytes: stride as c_int,
            p_metadata: std::ptr::null(),
            timestamp: 0,
        };
        unsafe { (self.rt.send_video)(self.inst, &f) };
    }

    /// How many receivers (OBS, vMix, Studio Monitor) are connected.
    pub fn connections(&self) -> i32 {
        unsafe { (self.rt.connections)(self.inst, 0) }
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        unsafe { (self.rt.send_destroy)(self.inst) };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn layout_matches_the_c_header() {
        // NDIlib_video_frame_v2_t on 64-bit: 4+4+4+4+4+4+4 (+4 pad) +8 +8 +4 (+4 pad) +8 +8 = 72
        assert_eq!(std::mem::size_of::<super::VideoFrameV2>(), 72);
        assert_eq!(std::mem::size_of::<super::SendCreate>(), 24);
    }

    #[test]
    fn missing_runtime_explains_what_to_install() {
        if super::Runtime::load().is_ok() {
            return; // a machine with NDI installed
        }
        let e = super::Runtime::load().err().unwrap();
        assert!(e.contains("ndi.video"));
    }
}
