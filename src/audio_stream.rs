use std::{
    cmp::min,
    sync::{Arc, Mutex},
};

use crate::{log, resource_channel::MyChannel, uni};

pub static VTABLE: uni::mpf_audio_stream_vtable_t = uni::mpf_audio_stream_vtable_t {
    destroy: Some(stream_destroy),
    open_rx: Some(stream_open),
    close_rx: Some(stream_close),
    read_frame: Some(stream_read),
    open_tx: None,
    close_tx: None,
    write_frame: None,
    trace: Some(trace),
};

pub unsafe extern "C" fn stream_destroy(_stream: *mut uni::mpf_audio_stream_t) -> uni::apt_bool_t {
    log(&format!("Destroy audio stream {:p}", _stream));
    uni::TRUE
}

pub unsafe extern "C" fn stream_open(
    _stream: *mut uni::mpf_audio_stream_t,
    _codec: *mut uni::mpf_codec_t,
) -> uni::apt_bool_t {
    log(&format!("Open audio stream: {:p}", _stream));
    uni::TRUE
}

pub unsafe extern "C" fn stream_close(_stream: *mut uni::mpf_audio_stream_t) -> uni::apt_bool_t {
    log(&format!("Close audio stream: {:p}", _stream));
    uni::TRUE
}

pub unsafe extern "C" fn stream_read(
    stream: *mut uni::mpf_audio_stream_t,
    frame: *mut uni::mpf_frame_t,
) -> uni::apt_bool_t {
    log(&format!(
        "Read audio stream {:p}. Frame size is {}",
        stream,
        (*frame).codec_frame.size
    ));
    let my_channel = (*stream).obj as *mut Arc<Mutex<MyChannel>>;
    let mut channel_lock = (*my_channel).lock().unwrap();
    log(&format!(
        "The channel {:p} has speak_bytes {:?}, have read {} of them.",
        my_channel,
        (channel_lock.speak_bytes.as_ref())
            .map(|x| x.len())
            .unwrap_or_default(),
        channel_lock.have_read_bytes
    ));
    if let Some(msg) = channel_lock.speak_msg {
        // the problem with dead lock is because of this^
        let speak_bytes = &channel_lock.speak_bytes;
        if let Some(speech) = speak_bytes {
            (*frame).type_ |= uni::MEDIA_FRAME_TYPE_AUDIO as i32;
            let speech_len = speech.len();
            let frame_size = (*frame).codec_frame.size;
            let frame_buffer = (*frame).codec_frame.buffer as *mut u8;
            let have_read_bytes = channel_lock.have_read_bytes;
            let bytes_to_read = min(speech_len - have_read_bytes, frame_size);
            let src = &speech[have_read_bytes] as *const u8;
            unsafe { std::ptr::copy_nonoverlapping(src, frame_buffer, bytes_to_read) }
            drop(channel_lock);
            let mut channel_lock = (*my_channel).lock().unwrap();
            channel_lock.have_read_bytes += bytes_to_read;
            if channel_lock.have_read_bytes == speech_len {
                channel_lock.speak_bytes = None;
                channel_lock.have_read_bytes = 0;
            }
        } else {
            channel_lock.speak_msg = None;
            drop(channel_lock);
            helper_send_complete_msg(my_channel, msg);
        }
    }
    uni::TRUE
}

pub unsafe extern "C" fn trace(
    _stream: *mut uni::mpf_audio_stream_t,
    _direction: uni::mpf_stream_direction_e,
    _output: *mut uni::apt_text_stream_t,
) {
    log(&format!(
        "Trace audio stream {:p} in direction {}",
        _stream, _direction
    ))
}

unsafe fn helper_send_complete_msg(
    my_channel: *mut Arc<Mutex<MyChannel>>,
    msg: *mut uni::mrcp_message_t,
) {
    let complete_msg =
        uni::mrcp_event_create(msg, uni::SYNTHESIZER_SPEAK_COMPLETE as _, (*msg).pool);
    if !complete_msg.is_null() {
        helper_complete_msg_prepare(complete_msg);
        (*my_channel)
            .lock()
            .unwrap()
            .engine_channel_message_send(complete_msg);
        log(&format!("Complete msg successfully sent."));
    }
}

unsafe fn helper_complete_msg_prepare(complete_msg: *mut uni::mrcp_message_t) {
    (*complete_msg).start_line.request_state = uni::MRCP_REQUEST_STATE_COMPLETE;
    let pool = (*complete_msg).pool;
    let h_accessor: *mut uni::mrcp_header_accessor_t =
        &mut (*complete_msg).header.resource_header_accessor;
    let header = helper_message_header_allocate(h_accessor, pool);
    if !header.is_null() {
        (*header).completion_cause = uni::SYNTHESIZER_COMPLETION_CAUSE_NORMAL;
        uni::mrcp_resource_header_property_add(
            complete_msg,
            uni::SYNTHESIZER_HEADER_COMPLETION_CAUSE as _,
        );
    }
}

unsafe fn helper_message_header_allocate(
    accessor: *mut uni::mrcp_header_accessor_t,
    pool: *mut uni::apr_pool_t,
) -> *mut uni::mrcp_synth_header_t {
    let data_ptr = (*accessor).data;
    if !data_ptr.is_null() {
        return data_ptr as _;
    }
    let v_table_ptr = (*accessor).vtable;
    if v_table_ptr.is_null() {
        return std::ptr::null_mut() as _;
    }
    let allocate = (*v_table_ptr).allocate;
    if allocate.is_none() {
        return std::ptr::null_mut() as _;
    }
    allocate.unwrap()(accessor, pool) as _
}
