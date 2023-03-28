
use crate::ffi;
//utils::cell::RacyUnsafeCell};
use std::ffi::CString;

pub struct Logger;

const LOG_NAME: &str = "My-Plugin";

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(&record.metadata()) {
            unsafe {
//                 use crate::ffi::apt_log_priority_e::*;
                let priority = match record.level() {
                    log::Level::Error => ffi::APT_PRIO_ERROR,
                    log::Level::Warn => ffi::APT_PRIO_WARNING,
                    log::Level::Info => ffi::APT_PRIO_NOTICE,
                    log::Level::Debug => ffi::APT_PRIO_INFO,
                    log::Level::Trace => ffi::APT_PRIO_DEBUG,
                };

                let file = CString::new(record.file().unwrap_or("")).unwrap();
                // Internally, apt_log will use this string as a
                // printf style format string, so it's important that
                // we escape `%` characters or else it will try to
                // substitute values from uninitialized memory.
                let format = CString::new(
                    format!("[DG :: {}] {}", record.target(), record.args()).replace("%", "%%"),
                )
                .unwrap();

                ffi::apt_log(
                    *RECOG_PLUGIN.get(),
                    file.as_ptr(),
                    record.line().unwrap_or(0) as i32,
                    priority,
                    format.as_ptr(),
                );
            }
        }
    }

    fn flush(&self) {}
}

/// The functional equivalent of `MRCP_PLUGIN_LOG_SOURCE_IMPLEMENT`.
#[no_mangle]
pub static RECOG_PLUGIN: RacyUnsafeCell<*mut ffi::apt_log_source_t> =
    unsafe { RacyUnsafeCell::new(&ffi::def_log_source as *const _ as *mut _) };

#[no_mangle]
pub unsafe extern "C" fn mrcp_plugin_logger_set(logger: *mut ffi::apt_logger_t) -> ffi::apt_bool_t {
    ffi::apt_log_instance_set(logger);
    ffi::TRUE
}

#[no_mangle]
pub unsafe extern "C" fn mrcp_plugin_log_source_set(orig_log_source: *mut ffi::apt_log_source_t) {
    let name = CString::new(LOG_NAME).unwrap();
    ffi::apt_def_log_source_set(orig_log_source);
    ffi::apt_log_source_assign(name.as_ptr(), RECOG_PLUGIN.get());
}


use std::cell::UnsafeCell;
use std::ops::Deref;

/// A transparent representation of a non-const global variable (and a "safer"
/// version of Rust's `static mut`). `UnsafeCell` gets us 90% there, but for
/// the variables to be static (global), they need to be `Sync`.
#[repr(transparent)]
pub struct RacyUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T> Sync for RacyUnsafeCell<T> {}

impl<T> RacyUnsafeCell<T> {
    pub const fn new(x: T) -> Self {
        Self(UnsafeCell::new(x))
    }
}

impl<T> Deref for RacyUnsafeCell<T> {
    type Target = UnsafeCell<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
