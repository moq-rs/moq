use crate::{Result, StreamId};
use retty::transport::TransportContext;

#[allow(clippy::upper_case_acronyms)]
pub enum Connection {
    QUIC,
    WebTransport,
}

impl Connection {
    pub fn transport(&self) -> TransportContext {
        TransportContext::default()
    }
    pub fn open_bi_stream(&mut self) -> Result<StreamId> {
        Ok(0)
    }
    pub fn open_uni_stream(&mut self) -> Result<StreamId> {
        Ok(0)
    }
    pub fn accept_uni_stream(&mut self) -> Result<StreamId> {
        Ok(0)
    }
    pub fn send_datagram(&mut self, _data: &[u8]) -> Result<usize> {
        Ok(0)
    }
    pub fn recv_datagram(&mut self, _data: &mut [u8]) -> Result<usize> {
        Ok(0)
    }
    pub fn send_stream_data(&mut self, _stream_id: StreamId, _data: &[u8]) -> Result<usize> {
        Ok(0)
    }
    pub fn recv_stream_data(&mut self, _stream_id: StreamId, _data: &mut [u8]) -> Result<usize> {
        Ok(0)
    }
    pub fn close_with_error(&mut self, _error_code: u64, _error_reason: &str) -> Result<()> {
        Ok(())
    }
}
