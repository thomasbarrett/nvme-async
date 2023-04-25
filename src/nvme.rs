use std::os::fd::AsRawFd;
use io_uring_async::IoUringAsync;
use io_uring::{squeue, cqueue};
use io_uring::opcode::UringCmd80;
use io_uring::types::Fd;
use zerocopy::{FromBytes, AsBytes};
use bdev_async::bdev::{BlockDevice, BlockDeviceQueue};
use async_trait::async_trait;

use nix::{request_code_readwrite, ioctl_none, ioctl_readwrite};

pub type __u8 = std::os::raw::c_uchar;
pub type __u16 = std::os::raw::c_ushort;
pub type __u32 = std::os::raw::c_uint;
pub type __u64 = std::os::raw::c_ulonglong;


#[repr(C)]
#[derive(FromBytes, AsBytes, Copy, Clone)]
pub struct nvme_passthru_cmd {
	opcode: __u8,
	flags: __u8,
	rsvd1: __u16,
	nsid: __u32,
	cdw2: __u32,
	cdw3: __u32,
	metadata: __u64,
	addr: __u64,
	metadata_len: __u32,
	data_len: __u32,
	cdw10: __u32,
	cdw11: __u32,
	cdw12: __u32,
	cdw13: __u32,
	cdw14: __u32,
	cdw15: __u32,
	timeout_ms: __u32,
	result: __u32,
}

#[repr(C)]
#[derive(FromBytes, AsBytes, Copy, Clone)]
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

#[repr(C)]
#[derive(Debug, FromBytes, AsBytes, Copy, Clone)]
struct nvme_lbaf {
	ms: __u16,
	ds: __u8,
	rp: __u8,
}

#[repr(C)]
#[derive(Debug, FromBytes, AsBytes, Copy, Clone)]
pub struct nvme_id_ns {
	nsze: __u64,
    ncap: __u64,
    nuse: __u64,
    nsfeat: __u8,
    nlbaf: __u8,
    flbas: __u8,
    mc: __u8,
    dpc: __u8,
    dps: __u8,
    nmic: __u8,
    rescap: __u8,
    fpi: __u8,
    dlfeat: __u8,
    nawun: __u16,
    nawupf: __u16,
	nacwu: __u16,
	nabsn: __u16,
	nabo: __u16,
	nabspf: __u16,
	noiob: __u16,
	nvmcap: [__u8; 16],
	npwg: __u16,
	npwa: __u16,
	npdg: __u16,
	npda: __u16,
	nows: __u16,
	mssrl: __u16,
	mcl: __u32,
	msrc: __u8,
	rsvd81: [__u8; 11],
	anagrpid: u32,
	rsvd96: [__u8; 3],
	nsattr: __u8,
	nvmsetid: __u16,
	endgid: __u16,
	nguid: [__u8; 16],
	eui64: [__u8; 8],
	lbaf: [nvme_lbaf; 16],
	rsvd192: [__u8; 192],
	vs: [__u8; 3712],
}

pub const NVME_URING_CMD_IO: u32 =
    request_code_readwrite!('N', 0x80, core::mem::size_of::<nvme_uring_cmd>()) as u32;

ioctl_none!(nvme_ioctl_id, 'N', 0x40);
ioctl_readwrite!(nvme_ioctl_admin_cmd, 'N', 0x41, nvme_passthru_cmd);

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

const NVME_IDENTIFY_CNS_NS: __u32		    = 0x00;
const NVME_IDENTIFY_CNS_CSI_NS: __u32	    = 0x05;
const NVME_IDENTIFY_CNS_CSI_CTRL: __u32	= 0x06;

const NVME_IDENTIFY_CSI_SHIFT: __u32 = 24;

const NVME_CSI_NVM: __u32 = 0;
const NVME_CSI_KV: __u32  = 1;
const NVME_CSI_ZNS: __u32 = 2;

const NVME_ADMIN_OPCODE_IDENTIFY: __u8 = 0x06;

#[derive(Clone)]
pub struct NvmeBlockDeviceHandle {
    inner: std::sync::Arc<NvmeBlockDeviceHandleInner>
}

struct NvmeBlockDeviceHandleInner {
    fd: std::fs::File,
    nsid: u32,
    logical_block_size: usize,
    size: usize,
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
        self.inner.size
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
            ..FromBytes::new_zeroed()
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
            ..FromBytes::new_zeroed()
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
    pub fn open(path: &str) -> std::io::Result<Self> {
        let fd = std::fs::File::open(path)?;
        let nsid = unsafe { nvme_ioctl_id(fd.as_raw_fd())? } as u32;

        let mut ns: nvme_id_ns = FromBytes::new_zeroed();
        let mut cmd = nvme_passthru_cmd {
            opcode: NVME_ADMIN_OPCODE_IDENTIFY,
            nsid,
            addr: std::ptr::addr_of_mut!(ns) as __u64,
            data_len: std::mem::size_of::<nvme_passthru_cmd>() as __u32,
            cdw10: NVME_IDENTIFY_CNS_NS as __u32,
            cdw11: NVME_CSI_NVM << NVME_IDENTIFY_CSI_SHIFT,
            ..FromBytes::new_zeroed()
        };
        
        let _ = unsafe { nvme_ioctl_admin_cmd(fd.as_raw_fd(), &mut cmd)? };
        let lba_sz = ns.lbaf[(ns.flbas & 0x0f) as usize].ds as usize;
        let nlba = ns.nsze as usize;

        Ok(Self { handle: NvmeBlockDeviceHandle { inner: std::sync::Arc::new(
            NvmeBlockDeviceHandleInner {fd, nsid, logical_block_size: lba_sz, size: nlba}
        )}})
    }
}
