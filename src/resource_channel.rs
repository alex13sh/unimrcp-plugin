use std::{
    collections::HashMap,
    ptr::NonNull,
    sync::{Arc, Mutex},
};

use serde_json::{json, Value as JsonValue};

use crate::{
    engine::MyEngine,
    log, msg_body,
    uni::{self, MRCP_REQUEST_STATE_INPROGRESS},
};

pub const VTABLE: uni::mrcp_engine_channel_method_vtable_t =
    uni::mrcp_engine_channel_method_vtable_t {
        destroy: Some(channel_destroy),
        open: Some(channel_open),
        close: Some(channel_close),
        process_request: Some(channel_process_request),
    };

pub unsafe extern "C" fn channel_destroy(
    _channel: *mut uni::mrcp_engine_channel_t,
) -> uni::apt_bool_t {
    log(&format!("Destroy channel. {:p}", _channel));
    uni::TRUE
}

pub unsafe extern "C" fn channel_open(channel: *mut uni::mrcp_engine_channel_t) -> uni::apt_bool_t {
    log(&format!("Open channel. {:p}", channel));
    helper_engine_channel_open_respond(channel, uni::TRUE)
}

unsafe extern "C" fn channel_close(channel: *mut uni::mrcp_engine_channel_t) -> uni::apt_bool_t {
    log(&format!("Close channel. {:p}", channel));
    helper_engine_channel_close_respond(channel)
}

unsafe extern "C" fn channel_process_request(
    channel: *mut uni::mrcp_engine_channel_t,
    request: *mut uni::mrcp_message_t,
) -> uni::apt_bool_t {
    let mut processed = uni::FALSE;
    let ak_channel = (*channel).method_obj as *mut Arc<Mutex<MyChannel>>;
    let pool = (*request).pool;
    let response = uni::mrcp_response_create(request, pool);
    let method_id = unsafe { (*request).start_line.method_id as u32 };
    let cmd = match method_id {
        uni::SYNTHESIZER_SET_PARAMS => "SYNTHESIZER_SET_PARAMS",
        uni::SYNTHESIZER_GET_PARAMS => "SYNTHESIZER_GET_PARAMS",
        uni::SYNTHESIZER_SPEAK => {
            processed = (*ak_channel).lock().unwrap().speak(request, response);
            "SYNTHESIZER_SPEAK"
        }
        uni::SYNTHESIZER_STOP => "SYNTHESIZER_STOP",
        uni::SYNTHESIZER_PAUSE => "SYNTHESIZER_PAUSE",
        uni::SYNTHESIZER_RESUME => "SYNTHESIZER_RESUME",
        uni::SYNTHESIZER_BARGE_IN_OCCURRED => "SYNTHESIZER_BARGE_IN_OCCURRED",
        uni::SYNTHESIZER_CONTROL => "SYNTHESIZER_CONTROL",
        uni::SYNTHESIZER_DEFINE_LEXICON => "SYNTHESIZER_DEFINE_LEXICON",
        _ => "Other",
    };
    log(&format!(
        "Request {cmd} processing. Channel is {:p}, request {:p}",
        channel, request
    ));
    if processed == uni::FALSE {
        (*ak_channel)
            .lock()
            .unwrap()
            .engine_channel_message_send(response);
    }
    uni::TRUE
}

#[derive(Debug)]
#[repr(C)]
pub struct MyChannel {
    pub engine: NonNull<uni::mrcp_engine_t>,
    pub channel: NonNull<uni::mrcp_engine_channel_t>,
    pub speak_msg: Option<*mut uni::mrcp_message_t>,
    pub speak_bytes: Option<Vec<u8>>,
    pub have_read_bytes: usize,
}

impl MyChannel {
    pub fn new(_pool: *mut uni::apr_pool_t) -> Arc<Mutex<Self>> {
        let channel = Self {
            engine: NonNull::dangling(),
            channel: NonNull::dangling(),
            speak_msg: None,
            speak_bytes: None,
            have_read_bytes: 0,
        };
        Arc::new(Mutex::new(channel))
    }

    pub fn speak(
        &mut self,
        request: *mut uni::mrcp_message_t,
        response: *mut uni::mrcp_message_t,
    ) -> uni::apt_bool_t {
        self.speak_msg = Some(request);
        let text = msg_body(request);
        log(&format!("Speak the text: {:?}", text));
        self.speak_bytes = self.perform_synthesize(text);
        self.have_read_bytes = 0;
        self.log();
        unsafe {
            (*response).start_line.request_state = MRCP_REQUEST_STATE_INPROGRESS as _;
            //            self.engine_channel_message_send(response);
        }
        uni::FALSE
    }

    pub fn reset_speak(&mut self) {
        self.speak_msg = None;
        self.speak_bytes = None;
        self.have_read_bytes = 0;
    }
}

impl MyChannel {
    pub(crate) unsafe fn engine_channel_message_send(&self, msg: *mut uni::mrcp_message_t) {
        let channel_ptr = self.channel.as_ptr();
        log(&format!(
            "Send message {:p} {:?} via channel {:p}",
            msg,
            msg_body(msg),
            channel_ptr
        ));
        (*(*channel_ptr).event_vtable).on_message.unwrap()(channel_ptr, msg);
    }

    pub(crate) fn log(&self) {
        log(&format!(
            "MyChannel on {:p}, speak_msg {:?}",
            self.channel.as_ptr(),
            self.speak_msg
        ))
    }

    fn perform_synthesize(&self, text: &str) -> Option<Vec<u8>> {
        let my_engine = unsafe { (*self.engine.as_ptr()).obj as *mut MyEngine };
//         let iam_token = unsafe { (*my_engine).iam_token.as_str() };
        let token = crate::secret::VOICEKIT_JWT;
        let data = json!({
            "input": {
                "text": text
            },
            "audioConfig": {
                "sampleRateHertz": "48000",
                "audioEncoding": "LINEAR16",
            },
            "voice": {
                "name": "maxim",
            }
        });
        let client = reqwest::blocking::Client::new();
        let req = client
//             .post("https://tts.api.cloud.yandex.net/speech/v1/tts:synthesize")
            .post("https://api.tinkoff.ai:443/v1/tts:synthesize")
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {token}"),
            )
            .json(&data);
        let res = req
            .send()
            .expect("(2) ask for synthezised speech but network fails");

        if !res.status().is_success() {
            log(&format!(
                "ERROR: Response status is {:?}\n{:#?}",
                res.status(),
                res.json::<HashMap<String, String>>()
            ));
            None
        } else {
            let res = res.json::<HashMap<String, String>>().unwrap();
            Some(base64::decode(res["audio_content"].as_bytes()).unwrap())
        }
    }
}

unsafe fn helper_engine_channel_open_respond(
    channel: *mut uni::mrcp_engine_channel_t,
    status: uni::apt_bool_t,
) -> uni::apt_bool_t {
    (*(*channel).event_vtable).on_open.unwrap()(channel, status)
}

unsafe fn helper_engine_channel_close_respond(
    channel: *mut uni::mrcp_engine_channel_t,
) -> uni::apt_bool_t {
    (*(*channel).event_vtable).on_close.unwrap()(channel)
}
