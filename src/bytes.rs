//! Bytes from TSX: an `ArrayBuffer`, a typed array or a `DataView`, as
//! WebGPU's `AllowSharedBufferSource` takes them (`writeBuffer`'s data,
//! `writeTexture`'s). The bytes are copied in, the view's range only.
//! Bytes going back to TSX are a `Vec<u8>`, a fresh `Uint8Array`.

use nsh_core::{
    element::{JsHost, JsIn, JsOwned, PropError, TsTypes},
    prelude::*,
};

/// The bytes of an `ArrayBuffer` or `ArrayBufferView`, copied.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bytes(pub Vec<u8>);

impl JsValue for Bytes {
    fn ts_type(_types: &mut TsTypes) -> String {
        "ArrayBuffer | ArrayBufferView".into()
    }

    fn from_js(value: JsIn<'_>) -> Result<Self, PropError> {
        value
            .as_buffer()
            .map(Bytes)
            .ok_or_else(|| value.expected("an ArrayBuffer or ArrayBufferView"))
    }

    fn to_js<'a>(&self, host: &'a dyn JsHost) -> Result<JsOwned<'a>, PropError> {
        Ok(JsOwned::new(host, host.new_bytes(&self.0)?))
    }
}
