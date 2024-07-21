use crate::{Result, StreamId};
use bytes::{Buf, BufMut};

pub trait Connection {
    fn open_bi_stream(&mut self) -> Result<StreamId>;
    fn open_uni_stream(&mut self) -> Result<StreamId>;
    fn accept_uni_stream(&mut self) -> Result<StreamId>;
    fn send_datagram<R: Buf>(&mut self, r: &mut R) -> Result<usize>;
    fn recv_datagram<W: BufMut>(&mut self, w: &mut W) -> Result<usize>;
    fn close_with_error(&mut self, error_code: u64, error_reason: &str) -> Result<()>;
}
