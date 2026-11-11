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

/// NDIlib_audio_frame_v2_t: 32-bit float, planar (each channel's samples together).
#[repr(C)]
struct AudioFrameV2 {
    sample_rate: c_int,
    no_channels: c_int,
    no_samples: c_int,
    timecode: i64,
    p_data: *const f32,
    channel_stride_in_bytes: c_int,
    p_metadata: *const c_char,
    timestamp: i64,
}

#[repr(C)]
struct FindCreate {
    show_local_sources: bool,
    p_groups: *const c_char,
    p_extra_ips: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SourceT {
    p_ndi_name: *const c_char,
    p_url_address: *const c_char,
}

#[repr(C)]
struct RecvCreateV3 {
    source: SourceT,
    color_format: c_int,
    bandwidth: c_int,
    allow_video_fields: bool,
    p_ndi_recv_name: *const c_char,
}

const RECV_COLOR_BGRX_BGRA: c_int = 0;
const RECV_BANDWIDTH_HIGHEST: c_int = 100;
const FRAME_TYPE_VIDEO: c_int = 1;

/// BGRA with straight (not premultiplied) alpha.
const FOURCC_BGRA: u32 = u32::from_le_bytes(*b"BGRA");
/// UYVY 4:2:2 plane followed by an alpha plane: NDI's own layout.
const FOURCC_UYVA: u32 = u32::from_le_bytes(*b"UYVA");
const FORMAT_PROGRESSIVE: c_int = 1;
const TIMECODE_SYNTHESIZE: i64 = i64::MAX;

type FnInit = unsafe extern "C" fn() -> bool;
type FnVersion = unsafe extern "C" fn() -> *const c_char;
type FnSendCreate = unsafe extern "C" fn(*const SendCreate) -> *mut c_void;
type FnSendDestroy = unsafe extern "C" fn(*mut c_void);
type FnSendVideo = unsafe extern "C" fn(*mut c_void, *const VideoFrameV2);
type FnConnections = unsafe extern "C" fn(*mut c_void, u32) -> c_int;
type FnSendAudio = unsafe extern "C" fn(*mut c_void, *const AudioFrameV2);
type FnFindCreate = unsafe extern "C" fn(*const FindCreate) -> *mut c_void;
type FnFindWait = unsafe extern "C" fn(*mut c_void, u32) -> bool;
type FnFindSources = unsafe extern "C" fn(*mut c_void, *mut u32) -> *const SourceT;
type FnDestroy = unsafe extern "C" fn(*mut c_void);
type FnRecvCreate = unsafe extern "C" fn(*const RecvCreateV3) -> *mut c_void;
type FnRecvCapture = unsafe extern "C" fn(*mut c_void, *mut VideoFrameV2, *mut c_void, *mut c_void, u32) -> c_int;
type FnRecvFreeVideo = unsafe extern "C" fn(*mut c_void, *const VideoFrameV2);

/// The loaded runtime. Keep it alive for as long as any sender exists.
pub struct Runtime {
    _lib: Library,
    pub path: PathBuf,
    pub version: String,
    send_create: FnSendCreate,
    send_destroy: FnSendDestroy,
    send_video: FnSendVideo,
    connections: FnConnections,
    send_audio: FnSendAudio,
    find_create: FnFindCreate,
    find_wait: FnFindWait,
    find_sources: FnFindSources,
    find_destroy: FnDestroy,
    recv_create: FnRecvCreate,
    recv_destroy: FnDestroy,
    recv_capture: FnRecvCapture,
    recv_free_video: FnRecvFreeVideo,
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
    // elsewhere the system library path is trusted; on Windows a bare name would also
    // search the current folder, where a planted DLL could be picked up
    if !cfg!(windows) {
        v.push(PathBuf::from(name));
    }
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
                send_audio: std::mem::transmute(sym(b"NDIlib_send_send_audio_v2\0")?),
                find_create: std::mem::transmute(sym(b"NDIlib_find_create_v2\0")?),
                find_wait: std::mem::transmute(sym(b"NDIlib_find_wait_for_sources\0")?),
                find_sources: std::mem::transmute(sym(b"NDIlib_find_get_current_sources\0")?),
                find_destroy: std::mem::transmute(sym(b"NDIlib_find_destroy\0")?),
                recv_create: std::mem::transmute(sym(b"NDIlib_recv_create_v3\0")?),
                recv_destroy: std::mem::transmute(sym(b"NDIlib_recv_destroy\0")?),
                recv_capture: std::mem::transmute(sym(b"NDIlib_recv_capture_v2\0")?),
                recv_free_video: std::mem::transmute(sym(b"NDIlib_recv_free_video_v2\0")?),
                version,
                path,
                _lib: lib,
            })
        }
    }

    /// A source called `name`, in `groups` (comma separated; None = the public group).
    /// The app paces frames itself (clock off), so a slow frame never stalls the loop.
    pub fn sender(self: &std::sync::Arc<Self>, name: &str, groups: Option<&str>) -> Result<Sender, String> {
        let c = CString::new(name).map_err(|_| "The NDI name cannot contain a zero byte.".to_string())?;
        let g = groups.map(str::trim).filter(|g| !g.is_empty()).map(CString::new).transpose().map_err(|_| "The NDI group cannot contain a zero byte.".to_string())?;
        let desc = SendCreate { p_ndi_name: c.as_ptr(), p_groups: g.as_ref().map_or(std::ptr::null(), |g| g.as_ptr()), clock_video: false, clock_audio: false };
        let inst = unsafe { (self.send_create)(&desc) };
        if inst.is_null() {
            return Err(format!("NDI could not create the source “{name}”. Is the name already in use on this computer?"));
        }
        Ok(Sender { rt: self.clone(), inst, _name: c, _groups: g })
    }
}

/// One NDI source. Dropping it removes the source from the network.
pub struct Sender {
    rt: std::sync::Arc<Runtime>,
    inst: *mut c_void,
    _name: CString,
    _groups: Option<CString>,
}

// NDI allows video and audio to be sent from different threads on one sender:
// the pump sends video, the audio thread sends sound.
unsafe impl Send for Sender {}
unsafe impl Sync for Sender {}

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

    /// Sends one frame already in UYVA layout (see `convert::bgra_to_uyva`).
    pub fn send_uyva(&self, width: u32, height: u32, data: &[u8], fps: u32) {
        let w = width & !1;
        assert!(data.len() >= (w * height * 3) as usize, "frame buffer too small");
        let f = VideoFrameV2 {
            xres: w as c_int,
            yres: height as c_int,
            fourcc: FOURCC_UYVA,
            frame_rate_n: fps.max(1) as c_int * 1000,
            frame_rate_d: 1000,
            picture_aspect_ratio: w as f32 / height as f32,
            frame_format_type: FORMAT_PROGRESSIVE,
            timecode: TIMECODE_SYNTHESIZE,
            p_data: data.as_ptr(),
            line_stride_in_bytes: (w * 2) as c_int,
            p_metadata: std::ptr::null(),
            timestamp: 0,
        };
        unsafe { (self.rt.send_video)(self.inst, &f) };
    }

    /// Sends sound: `planar` holds each channel's samples one after another.
    pub fn send_audio(&self, sample_rate: u32, channels: u32, planar: &[f32]) {
        let samples = planar.len() / channels.max(1) as usize;
        if samples == 0 {
            return;
        }
        let f = AudioFrameV2 {
            sample_rate: sample_rate as c_int,
            no_channels: channels as c_int,
            no_samples: samples as c_int,
            timecode: TIMECODE_SYNTHESIZE,
            p_data: planar.as_ptr(),
            channel_stride_in_bytes: (samples * 4) as c_int,
            p_metadata: std::ptr::null(),
            timestamp: 0,
        };
        unsafe { (self.rt.send_audio)(self.inst, &f) };
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

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Source {
    pub name: String,
    pub url: String,
}

impl Runtime {
    /// NDI sources on the network (cameras, other computers, OBS…), waiting up to `ms`.
    pub fn find_sources(&self, groups: Option<&str>, ms: u32) -> Vec<Source> {
        let g = groups.and_then(|g| CString::new(g).ok());
        let desc = FindCreate { show_local_sources: true, p_groups: g.as_ref().map_or(std::ptr::null(), |g| g.as_ptr()), p_extra_ips: std::ptr::null() };
        let f = unsafe { (self.find_create)(&desc) };
        if f.is_null() {
            return vec![];
        }
        unsafe { (self.find_wait)(f, ms) };
        let mut n = 0u32;
        let list = unsafe { (self.find_sources)(f, &mut n) };
        let mut out = vec![];
        if !list.is_null() {
            for i in 0..n as usize {
                let s = unsafe { *list.add(i) };
                let text = |p: *const c_char| if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() };
                out.push(Source { name: text(s.p_ndi_name), url: text(s.p_url_address) });
            }
        }
        unsafe { (self.find_destroy)(f) };
        out
    }

    /// Connects to a source by name; frames arrive as BGRA.
    pub fn receiver(self: &std::sync::Arc<Self>, source: &Source) -> Result<Receiver, String> {
        let name = CString::new(source.name.clone()).map_err(|_| "Bad source name.".to_string())?;
        let url = CString::new(source.url.clone()).unwrap_or_default();
        let me = CString::new("Worship Presenter").unwrap();
        let desc = RecvCreateV3 {
            source: SourceT { p_ndi_name: name.as_ptr(), p_url_address: if source.url.is_empty() { std::ptr::null() } else { url.as_ptr() } },
            color_format: RECV_COLOR_BGRX_BGRA,
            bandwidth: RECV_BANDWIDTH_HIGHEST,
            allow_video_fields: false,
            p_ndi_recv_name: me.as_ptr(),
        };
        let inst = unsafe { (self.recv_create)(&desc) };
        if inst.is_null() {
            return Err(format!("Could not connect to the NDI source “{}”.", source.name));
        }
        Ok(Receiver { rt: self.clone(), inst })
    }
}

/// A connection to one NDI source.
pub struct Receiver {
    rt: std::sync::Arc<Runtime>,
    inst: *mut c_void,
}

unsafe impl Send for Receiver {}

/// One received picture: BGRA (or BGRX), rows packed.
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Receiver {
    /// The next video frame, waiting up to `ms`; None if nothing came.
    pub fn next_picture(&self, ms: u32) -> Option<Picture> {
        let mut f = VideoFrameV2 {
            xres: 0, yres: 0, fourcc: 0, frame_rate_n: 0, frame_rate_d: 0, picture_aspect_ratio: 0.0, frame_format_type: 0, timecode: 0,
            p_data: std::ptr::null(), line_stride_in_bytes: 0, p_metadata: std::ptr::null(), timestamp: 0,
        };
        let t = unsafe { (self.rt.recv_capture)(self.inst, &mut f, std::ptr::null_mut(), std::ptr::null_mut(), ms) };
        if t != FRAME_TYPE_VIDEO || f.p_data.is_null() || f.xres <= 0 || f.yres <= 0 {
            return None;
        }
        let (w, h, stride) = (f.xres as usize, f.yres as usize, f.line_stride_in_bytes.max(f.xres * 4) as usize);
        let mut data = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            let row = unsafe { std::slice::from_raw_parts(f.p_data.add(y * stride), w * 4) };
            data.extend_from_slice(row);
        }
        unsafe { (self.rt.recv_free_video)(self.inst, &f) };
        Some(Picture { width: w as u32, height: h as u32, data })
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        unsafe { (self.rt.recv_destroy)(self.inst) };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn find_and_receive_layouts() {
        // NDIlib_audio_frame_v2_t: 3×int (12) +4 pad, int64 @16, ptr @24, int @32 +4 pad, ptr @40, int64 @48 = 56
        assert_eq!(std::mem::size_of::<super::AudioFrameV2>(), 56);
        // NDIlib_find_create_t: bool (+7 pad), ptr, ptr = 24
        assert_eq!(std::mem::size_of::<super::FindCreate>(), 24);
        assert_eq!(std::mem::size_of::<super::SourceT>(), 16);
        // NDIlib_recv_create_v3_t: source 16, color 4, bandwidth 4, bool (+7), ptr = 40
        assert_eq!(std::mem::size_of::<super::RecvCreateV3>(), 40);
    }

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
