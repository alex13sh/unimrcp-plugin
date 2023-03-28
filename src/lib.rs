#[macro_use]
extern crate log;

mod logging;
pub mod audio_stream;
pub mod engine;
pub mod resource_channel;

pub(crate) mod secret {
    include!(".secret");
}

use uni as ffi;

pub mod uni {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    #![allow(clippy::all)]
    #![allow(rustdoc::broken_intra_doc_links)]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

    pub const FALSE: apt_bool_t = 0;
    pub const TRUE: apt_bool_t = 1;
}

#[no_mangle]
pub static mut mrcp_plugin_version: uni::mrcp_plugin_version_t = uni::mrcp_plugin_version_t {
    major: uni::PLUGIN_MAJOR_VERSION as i32,
    minor: uni::PLUGIN_MINOR_VERSION as i32,
    patch: uni::PLUGIN_PATCH_VERSION as i32,
    is_dev: 0,
};

#[no_mangle]
pub extern "C" fn mrcp_plugin_create(pool: *mut uni::apr_pool_t) -> *mut uni::mrcp_engine_t {
    match log::set_logger(&logging::Logger) {
        Err(err) => eprintln!("FAILED TO SET LOGGER: {}", err),
        Ok(()) => log::set_max_level(log::LevelFilter::max()),
    }

    log("plugin create");
    unsafe {
        // Engines's object pointer set
        // to null. It will be initialized in `engine_open`.
        uni::mrcp_engine_create(
            uni::MRCP_SYNTHESIZER_RESOURCE as _,
            std::ptr::null_mut(),
            &engine::ENGINE_VTABLE as *const _,
            pool,
        )
    }
}

pub(crate) fn log(s: &str) {
    info!("[My-Plugin] {s}");
}

pub(crate) fn msg_body<'a>(msg: *mut uni::mrcp_message_t) -> &'a str {
    unsafe {
        let raw_body = (*msg).body;
        let body_buf = std::slice::from_raw_parts(raw_body.buf as *const u8, raw_body.length);
        std::str::from_utf8_unchecked(body_buf)
    }
}
