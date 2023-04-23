use std::os::fd::AsRawFd;
use io_uring_async::IoUringAsync;
use io_uring::{squeue, cqueue};
use io_uring::opcode::UringCmd80;
use io_uring::types::Fd;
use zerocopy::{AsBytes};
use bdev_async::bdev::{BlockDevice, BlockDeviceQueue};
use async_trait::async_trait;

use nix::request_code_readwrite;

pub type __u8 = std::os::raw::c_uchar;
pub type __u16 = std::os::raw::c_ushort;
pub type __u32 = std::os::raw::c_uint;
pub type __u64 = std::os::raw::c_ulonglong;

#[repr(C)]
#[derive(Debug, Default, AsBytes, Copy, Clone)]
pub struct nvme_uring_cmd {
    pub opcode: __u8,
    pub flags: __u8,
    pub rsvd1: __u16,
    pub nsid: __u32,
    pub cdw2: __u32,
    pub cdw3: __u32,
    pub metadata: __u64,
    pub addr: __u64,
    pub metadata_len: __u32,
    pub data_len: __u32,
    pub cdw10: __u32,
    pub cdw11: __u32,
    pub cdw12: __u32,
    pub cdw13: __u32,
    pub cdw14: __u32,
    pub cdw15: __u32,
    pub timeout_ms: __u32,
    pub rsvd2: __u32,
}

pub const NVME_URING_CMD_IO: u32 =
    request_code_readwrite!('N', 0x80, core::mem::size_of::<nvme_uring_cmd>()) as u32;

pub const NVME_OPCODE_FLUSH: u8 = 0x00;
pub const NVME_OPCODE_WRITE: u8 = 0x01;
pub const NVME_OPCODE_READ: u8 = 0x02;


pub struct NvmeBlockDevice {
    handle: NvmeBlockDeviceHandle
}

impl BlockDevice for NvmeBlockDevice {
    fn logical_block_size(&self) -> usize {
        self.handle.logical_block_size()
    }
    fn size(&self) -> usize {
        self.handle.size()
    }
}

impl NvmeBlockDevice {
    pub fn create_queue(&self, uring: std::rc::Rc<IoUringAsync<squeue::Entry128, cqueue::Entry32>>) -> NvmeBlockDeviceQueue {
        self.handle.create_queue(uring)
    }
    pub fn handle<'a>(&'a self) -> &'a NvmeBlockDeviceHandle {
        &self.handle
    }
}


#[derive(Clone)]
pub struct NvmeBlockDeviceHandle {
    inner: std::sync::Arc<NvmeBlockDeviceHandleInner>
}

struct NvmeBlockDeviceHandleInner {
    fd: std::fs::File,
    nsid: u32,
    logical_block_size: usize
}

impl NvmeBlockDeviceHandle {
    pub fn create_queue(&self, uring: std::rc::Rc<IoUringAsync<squeue::Entry128, cqueue::Entry32>>) -> NvmeBlockDeviceQueue {
        NvmeBlockDeviceQueue { handle: self.clone(), uring }
    }
}

impl BlockDevice for NvmeBlockDeviceHandle {
    fn logical_block_size(&self) -> usize {
        self.inner.logical_block_size
    }
    fn size(&self) -> usize {
        0
    }
}

pub struct NvmeBlockDeviceQueue {
    handle: NvmeBlockDeviceHandle,
    uring: std::rc::Rc<IoUringAsync<squeue::Entry128, cqueue::Entry32>>,
}

#[async_trait(?Send)]
impl BlockDeviceQueue for NvmeBlockDeviceQueue {
    fn logical_block_size(&self) -> usize {
       self.handle.inner.logical_block_size
    }
    
    fn size(&self) -> usize {
        0
    }

    async fn read_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
        let nlb = (buf.len() >> self.logical_block_size()) as u32;
        let slba = (offset >> self.logical_block_size()) as u64;
        let cmd = nvme_uring_cmd {
            opcode: NVME_OPCODE_READ,
            nsid: self.handle.inner.nsid,
            addr: &mut buf[0] as *mut u8 as u64,
            data_len: buf.len() as u32,
            cdw10: (slba & 0xffffffff) as u32,
            cdw11: (slba >> 32) as u32,
            cdw12: nlb - 1,
            ..Default::default()
        };

        let mut cmd_bytes = [0u8; 80];
        cmd.as_bytes().write_to_prefix(&mut cmd_bytes[..]);

        let sqe = UringCmd80::new(Fd(self.handle.inner.fd.as_raw_fd()), NVME_URING_CMD_IO)
            .cmd(cmd_bytes)
            .build();

        let cqe = self.uring.push(sqe).await;
        if cqe.result() < 0 {
            return Err(std::io::Error::from_raw_os_error(-cqe.result()))
        }

        Ok(buf.len())
    }

    async fn write_at(&self, buf: &[u8], offset: u64) -> std::io::Result<usize> {
        let nlb = (buf.len() >> self.logical_block_size()) as u32;
        let slba = (offset >> self.logical_block_size()) as u64;
        let cmd = nvme_uring_cmd {
            opcode: NVME_OPCODE_WRITE,
            nsid: self.handle.inner.nsid,
            addr: &buf[0] as *const u8 as u64,
            data_len: buf.len() as u32,
            cdw10: (slba & 0xffffffff) as u32,
            cdw11: (slba >> 32) as u32,
            cdw12: nlb - 1,
            ..Default::default()
        };

        let mut cmd_bytes = [0u8; 80];
        cmd.as_bytes().write_to_prefix(&mut cmd_bytes[..]);

        let sqe = UringCmd80::new(Fd(self.handle.inner.fd.as_raw_fd()), NVME_URING_CMD_IO)
            .cmd(cmd_bytes)
            .build();

        let cqe = self.uring.push(sqe).await;
        if cqe.result() < 0 {
            return Err(std::io::Error::from_raw_os_error(-cqe.result()))
        }

        Ok(buf.len())
    }
}

impl NvmeBlockDevice {
    pub fn open(path: &str, nsid: u32) -> std::io::Result<Self> {
        let fd = std::fs::File::open(path)?;
        Ok(Self { handle: NvmeBlockDeviceHandle { inner: std::sync::Arc::new(
            NvmeBlockDeviceHandleInner {fd, nsid, logical_block_size: 9}
        )}})
    }
}
