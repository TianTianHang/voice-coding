use libloading::Library;
use std::fmt;
use std::path::Path;

type TenVadCreateFn = unsafe extern "C" fn(*mut *mut std::ffi::c_void, i32, f32) -> i32;
type TenVadProcessFn =
    unsafe extern "C" fn(*mut std::ffi::c_void, *const i16, i32, *mut f32, *mut i32) -> i32;
type TenVadDestroyFn = unsafe extern "C" fn(*mut std::ffi::c_void);

#[derive(Debug)]
pub enum VadError {
    Load(String),
    Init(String),
    Process(i32),
}

impl fmt::Display for VadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VadError::Load(s) => write!(f, "VAD load error: {}", s),
            VadError::Init(s) => write!(f, "VAD init error: {}", s),
            VadError::Process(code) => write!(f, "VAD process error code: {}", code),
        }
    }
}

impl std::error::Error for VadError {}

pub struct VadEngine {
    _library: Library,
    handle: *mut std::ffi::c_void,
    process_fn: TenVadProcessFn,
    destroy_fn: TenVadDestroyFn,
}

impl VadEngine {
    pub fn new(lib_path: &Path, hop_size: i32, threshold: f32) -> Result<Self, VadError> {
        unsafe {
            log::info!(
                "loading TEN VAD engine: path={} hop_size={} threshold={}",
                lib_path.display(),
                hop_size,
                threshold
            );
            let library = Library::new(lib_path)
                .map_err(|e| VadError::Load(format!("Failed to load library: {}", e)))?;

            let create_fn: TenVadCreateFn = *library
                .get(b"ten_vad_create\0")
                .map_err(|e| VadError::Load(format!("Symbol ten_vad_create: {}", e)))?;
            let process_fn: TenVadProcessFn = *library
                .get(b"ten_vad_process\0")
                .map_err(|e| VadError::Load(format!("Symbol ten_vad_process: {}", e)))?;
            let destroy_fn: TenVadDestroyFn = *library
                .get(b"ten_vad_destroy\0")
                .map_err(|e| VadError::Load(format!("Symbol ten_vad_destroy: {}", e)))?;

            let mut handle: *mut std::ffi::c_void = std::ptr::null_mut();
            let result = create_fn(&mut handle, hop_size, threshold);
            if result != 0 || handle.is_null() {
                log::error!("TEN VAD initialization failed: code={result}");
                return Err(VadError::Init(format!(
                    "ten_vad_create failed with code {}",
                    result
                )));
            }

            log::info!("TEN VAD engine initialized");
            Ok(Self {
                _library: library,
                handle,
                process_fn,
                destroy_fn,
            })
        }
    }

    pub fn process(&self, audio: &[i16]) -> Result<(f32, i32), VadError> {
        unsafe {
            let mut prob: f32 = 0.0;
            let mut flag: i32 = 0;
            let ret = (self.process_fn)(
                self.handle,
                audio.as_ptr(),
                audio.len() as i32,
                &mut prob,
                &mut flag,
            );
            if ret != 0 {
                log::error!("TEN VAD process failed: code={ret}");
                return Err(VadError::Process(ret));
            }
            Ok((prob, flag))
        }
    }
}

impl Drop for VadEngine {
    fn drop(&mut self) {
        unsafe {
            log::debug!("destroying TEN VAD engine");
            (self.destroy_fn)(self.handle);
        }
    }
}

unsafe impl Send for VadEngine {}
