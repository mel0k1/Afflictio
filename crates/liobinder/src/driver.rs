use crate::ioc;
use crate::parcel::{ParcelReader, ParcelWriter, TF_ACCEPT_FDS};
use std::fs::{File, OpenOptions};
use std::io;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

const BINDER_MMAP_SIZE: usize = 1024 * 1024;
const READ_BUFFER_SIZE: usize = 64 * 1024;
const BINDERFS_CONTROL: &str = "/dev/binderfs/binder-control";
const BINDERFS_NODE: &str = "/dev/binderfs/afflictio0";
const LEGACY_DEVICE: &str = "/dev/binder";
const FLUSH_ROUNDS_MAX: usize = 8;

#[derive(Debug)]
pub struct BinderDevice {
    file: File,
    path: String,
    mmap_ptr: *mut libc::c_void,
    mmap_size: usize,
    protocol_version: i32,
}

unsafe impl Send for BinderDevice {}

impl Drop for BinderDevice {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.mmap_ptr, self.mmap_size);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinderError {
    Io(String),
    DeadReply,
    FailedReply,
    Kernel(i32),
    NoDevice,
}

impl std::fmt::Display for BinderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinderError::Io(e) => write!(f, "binder driver i/o: {e}"),
            BinderError::DeadReply => write!(f, "binder: dead reply"),
            BinderError::FailedReply => write!(f, "binder: failed reply"),
            BinderError::Kernel(code) => write!(f, "binder: kernel error {code}"),
            BinderError::NoDevice => write!(f, "no binder device available"),
        }
    }
}

impl From<io::Error> for BinderError {
    fn from(e: io::Error) -> Self {
        BinderError::Io(e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub data: Vec<u8>,
    pub offsets: Vec<u32>,
    pub sender_pid: i32,
    pub sender_euid: u32,
    pub object: Option<crate::parcel::FlatBinderObject>,
}

impl Reply {
    pub fn reader(&self) -> ParcelReader<'_> {
        ParcelReader::new(&self.data, &self.offsets)
    }
}

#[repr(C)]
struct BinderWriteRead {
    write_size: u64,
    write_consumed: u64,
    write_buffer: u64,
    read_size: u64,
    read_consumed: u64,
    read_buffer: u64,
}

#[repr(C)]
struct BinderVersion {
    protocol_version: i32,
}

#[repr(C, packed)]
struct BinderTransactionData {
    target_handle: u32,
    target_pad: u32,
    code: u32,
    flags: u32,
    sender_pid: i32,
    sender_euid: u32,
    data_size: u64,
    offsets_size: u64,
    data_buffer: u64,
    data_offsets: u64,
}

fn build_transaction(handle: u32, code: u32, flags: u32, data: &[u8], offsets: &[u64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 56);
    out.extend_from_slice(&ioc::BC_TRANSACTION.to_le_bytes());
    let btd = BinderTransactionData {
        target_handle: handle,
        target_pad: 0,
        code,
        flags,
        sender_pid: 0,
        sender_euid: 0,
        data_size: data.len() as u64,
        offsets_size: (offsets.len() * 8) as u64,
        data_buffer: data.as_ptr() as u64,
        data_offsets: if offsets.is_empty() {
            0
        } else {
            offsets.as_ptr() as u64
        },
    };
    let btd_bytes: [u8; 56] = unsafe { std::mem::transmute_copy(&btd) };
    out.extend_from_slice(&btd_bytes);
    out
}

fn read_u32(buf: &[u8], pos: usize) -> u32 {
    u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]])
}

fn read_u64(buf: &[u8], pos: usize) -> u64 {
    let bytes: [u8; 8] = buf[pos..pos + 8].try_into().expect("u64 slice");
    u64::from_le_bytes(bytes)
}

fn parse_commands(
    buf: &[u8],
    mmap_size: usize,
) -> Result<(Option<Reply>, Vec<u64>, usize), BinderError> {
    let mut pos = 0usize;
    let mut free_buffers = Vec::new();
    while pos + 4 <= buf.len() {
        let cmd = read_u32(buf, pos);
        pos += 4;
        match cmd {
            ioc::BR_NOOP
            | ioc::BR_OK
            | ioc::BR_TRANSACTION_COMPLETE
            | ioc::BR_SPAWN_LOOPER
            | ioc::BR_INCREFS
            | ioc::BR_ACQUIRE
            | ioc::BR_RELEASE
            | ioc::BR_DECREFS => {}
            ioc::BR_DEAD_REPLY => return Err(BinderError::DeadReply),
            ioc::BR_FAILED_REPLY => return Err(BinderError::FailedReply),
            ioc::BR_ERROR => {
                if pos + 4 > buf.len() {
                    return Err(BinderError::Io(String::from("BR_ERROR truncated")));
                }
                let code = read_u32(buf, pos) as i32;
                return Err(BinderError::Kernel(code));
            }
            ioc::BR_REPLY | ioc::BR_REPLY_SG => {
                let sg = cmd == ioc::BR_REPLY_SG;
                let body = if sg { 64 } else { 56 };
                if pos + body > buf.len() {
                    return Err(BinderError::Io(String::from("BR_REPLY truncated")));
                }
                let sender_pid = read_u32(buf, pos + 16) as i32;
                let sender_euid = read_u32(buf, pos + 20);
                let data_size = read_u64(buf, pos + 24);
                let offsets_size = read_u64(buf, pos + 32);
                let data_buffer = read_u64(buf, pos + 40);
                let data_offsets = read_u64(buf, pos + 48);
                pos += body;
                if data_size as usize > mmap_size {
                    return Err(BinderError::Io(String::from("reply exceeds binder mmap")));
                }
                let raw: &[u8] = unsafe {
                    std::slice::from_raw_parts(data_buffer as *const u8, data_size as usize)
                };
                let data = raw.to_vec();
                let mut offsets = Vec::new();
                for i in 0..((offsets_size as usize) / 8) {
                    let value = unsafe {
                        std::ptr::read_unaligned(
                            (data_offsets as *const u8).add(i * 8) as *const u64
                        )
                    };
                    let offset = u32::try_from(value).map_err(|_| {
                        BinderError::Io(String::from("object offset beyond 32 bits"))
                    })?;
                    offsets.push(offset);
                }
                let reader = ParcelReader::new(&data, &offsets);
                let object = reader.first_object().ok();
                return Ok((
                    Some(Reply {
                        data,
                        offsets,
                        sender_pid,
                        sender_euid,
                        object,
                    }),
                    free_buffers,
                    pos,
                ));
            }
            ioc::BR_TRANSACTION | ioc::BR_TRANSACTION_SG => {
                let sg = cmd == ioc::BR_TRANSACTION_SG;
                let body = if sg { 64 } else { 56 };
                if pos + body > buf.len() {
                    return Err(BinderError::Io(String::from("BR_TRANSACTION truncated")));
                }
                let buffer = read_u64(buf, pos + 48);
                free_buffers.push(buffer);
                pos += body;
            }
            other => {
                return Err(BinderError::Io(format!(
                    "unknown binder command {other:#x}"
                )));
            }
        }
    }
    Ok((None, free_buffers, pos))
}

fn build_free_buffer(buffer: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(12);
    out.extend_from_slice(&ioc::BC_FREE_BUFFER.to_le_bytes());
    out.extend_from_slice(&buffer.to_le_bytes());
    out
}

impl BinderDevice {
    pub fn open() -> Result<Self, BinderError> {
        if create_binderfs_node() {
            if let Ok(device) = Self::open_path(BINDERFS_NODE) {
                return Ok(device);
            }
        }
        match Self::open_path(LEGACY_DEVICE) {
            Ok(device) => Ok(device),
            Err(_) => Err(BinderError::NoDevice),
        }
    }

    pub fn open_path(path: &str) -> Result<Self, BinderError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_CLOEXEC)
            .open(path)?;
        let fd = file.as_raw_fd();
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                BINDER_MMAP_SIZE,
                libc::PROT_READ,
                libc::MAP_PRIVATE | libc::MAP_NORESERVE,
                fd,
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error().into());
        }
        let mut version = BinderVersion {
            protocol_version: 0,
        };
        let rc = unsafe {
            libc::ioctl(
                fd,
                ioc::BINDER_VERSION as libc::Ioctl,
                &mut version as *mut BinderVersion,
            )
        };
        if rc < 0 {
            unsafe { libc::munmap(ptr, BINDER_MMAP_SIZE) };
            return Err(io::Error::last_os_error().into());
        }
        Ok(Self {
            file,
            path: path.to_string(),
            mmap_ptr: ptr,
            mmap_size: BINDER_MMAP_SIZE,
            protocol_version: version.protocol_version,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn protocol_version(&self) -> i32 {
        self.protocol_version
    }

    fn write_read(&self, write: &[u8], read: &mut [u8]) -> Result<usize, BinderError> {
        let mut wbr = BinderWriteRead {
            write_size: write.len() as u64,
            write_consumed: 0,
            write_buffer: write.as_ptr() as u64,
            read_size: read.len() as u64,
            read_consumed: 0,
            read_buffer: read.as_mut_ptr() as u64,
        };
        let rc = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                ioc::BINDER_WRITE_READ as libc::Ioctl,
                &mut wbr as *mut BinderWriteRead,
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error().into());
        }
        let consumed = wbr.read_consumed as usize;
        if consumed > read.len() {
            return Err(BinderError::Io(String::from("binder read overrun")));
        }
        Ok(consumed)
    }

    pub fn call(
        &self,
        handle: u32,
        code: u32,
        parcel: &ParcelWriter,
        one_way: bool,
    ) -> Result<Option<Reply>, BinderError> {
        let (data, offsets32) = parcel.clone().into_parts();
        let offsets: Vec<u64> = offsets32.iter().map(|o| *o as u64).collect();
        let flags = TF_ACCEPT_FDS | (if one_way { 1 } else { 0 });
        let transaction = build_transaction(handle, code, flags, &data, &offsets);
        let mut read = vec![0u8; READ_BUFFER_SIZE];
        let mut write = transaction;
        for _ in 0..FLUSH_ROUNDS_MAX {
            let consumed = self.write_read(&write, &mut read)?;
            let (found, free, _) = parse_commands(&read[..consumed], self.mmap_size)?;
            if let Some(reply) = found {
                if one_way {
                    return Ok(None);
                }
                return Ok(Some(reply));
            }
            if free.is_empty() {
                break;
            }
            write.clear();
            for buffer in free {
                write.extend_from_slice(&build_free_buffer(buffer));
            }
        }
        if one_way {
            Ok(None)
        } else {
            Err(BinderError::FailedReply)
        }
    }
}

pub fn create_binderfs_node() -> bool {
    let Ok(mut control) = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_CLOEXEC)
        .open(BINDERFS_CONTROL)
    else {
        return false;
    };
    control.write_all(b"afflictio0\0").is_ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutcome {
    pub path: String,
    pub protocol_version: i32,
}

pub fn probe() -> Result<ProbeOutcome, BinderError> {
    let device = BinderDevice::open()?;
    Ok(ProbeOutcome {
        path: device.path().to_string(),
        protocol_version: device.protocol_version(),
    })
}
