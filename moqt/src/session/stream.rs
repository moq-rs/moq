use crate::session::session_parameters::SessionParameters;
use bytes::BytesMut;

pub struct Stream {
    session_parameter: SessionParameters,
    // none means "incoming stream, and we don't know if it's the control
    // stream or a data stream yet".
    is_control_stream: Option<bool>,
    partial_object: BytesMut,
}

impl Stream {
    pub fn new(session_parameter: SessionParameters, is_control_stream: Option<bool>) -> Self {
        Self {
            session_parameter,
            is_control_stream,
            partial_object: Default::default(),
        }
    }
}
