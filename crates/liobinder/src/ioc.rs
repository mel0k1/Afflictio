const _IOC_NRBITS: u32 = 8;
const _IOC_TYPEBITS: u32 = 8;
const _IOC_SIZEBITS: u32 = 14;
const _IOC_DIRBITS: u32 = 2;

const _IOC_NRSHIFT: u32 = 0;
const _IOC_TYPESHIFT: u32 = _IOC_NRSHIFT + _IOC_NRBITS;
const _IOC_SIZESHIFT: u32 = _IOC_TYPESHIFT + _IOC_TYPEBITS;
const _IOC_DIRSHIFT: u32 = _IOC_SIZESHIFT + _IOC_SIZEBITS;

const _IOC_NONE: u32 = 0;
const _IOC_WRITE: u32 = 1;
const _IOC_READ: u32 = 2;

pub const fn ioc(dir: u32, kind: u8, nr: u8, size: u32) -> u32 {
    (dir << _IOC_DIRSHIFT)
        | ((size & ((1 << _IOC_SIZEBITS) - 1)) << _IOC_SIZESHIFT)
        | ((kind as u32) << _IOC_TYPESHIFT)
        | ((nr as u32) << _IOC_NRSHIFT)
}

pub const fn iow(kind: u8, nr: u8, size: u32) -> u32 {
    ioc(_IOC_WRITE, kind, nr, size)
}

pub const fn ior(kind: u8, nr: u8, size: u32) -> u32 {
    ioc(_IOC_READ, kind, nr, size)
}

pub const fn iowr(kind: u8, nr: u8, size: u32) -> u32 {
    ioc(_IOC_READ | _IOC_WRITE, kind, nr, size)
}

pub const fn io(kind: u8, nr: u8) -> u32 {
    ioc(_IOC_NONE, kind, nr, 0)
}

const BINDER_MAGIC: u8 = b'b';
const BR_MAGIC: u8 = b'r';
const BC_MAGIC: u8 = b'c';
const DRM_MAGIC: u8 = b'd';

pub const BINDER_WRITE_READ: u32 = iowr(BINDER_MAGIC, 1, 48);
pub const BINDER_SET_IDLE_TIMEOUT: u32 = iow(BINDER_MAGIC, 3, 8);
pub const BINDER_SET_MAX_THREADS: u32 = iow(BINDER_MAGIC, 5, 4);
pub const BINDER_SET_CONTEXT_MGR: u32 = iow(BINDER_MAGIC, 6, 8);
pub const BINDER_THREAD_EXIT: u32 = iow(BINDER_MAGIC, 8, 4);
pub const BINDER_VERSION: u32 = iowr(BINDER_MAGIC, 7, 4);

pub const BC_TRANSACTION: u32 = iow(BC_MAGIC, 0, 56);
pub const BC_REPLY: u32 = iow(BC_MAGIC, 1, 56);
pub const BC_FREE_BUFFER: u32 = iow(BC_MAGIC, 3, 8);
pub const BC_INCREFS: u32 = io(BC_MAGIC, 4);
pub const BC_ACQUIRE: u32 = io(BC_MAGIC, 5);
pub const BC_RELEASE: u32 = io(BC_MAGIC, 6);
pub const BC_DECREFS: u32 = io(BC_MAGIC, 7);
pub const BC_INCREFS_DONE: u32 = iow(BC_MAGIC, 8, 16);
pub const BC_ACQUIRE_DONE: u32 = iow(BC_MAGIC, 9, 16);
pub const BC_REGISTER_LOOPER: u32 = io(BC_MAGIC, 11);
pub const BC_ENTER_LOOPER: u32 = io(BC_MAGIC, 12);
pub const BC_EXIT_LOOPER: u32 = io(BC_MAGIC, 13);
pub const BC_REQUEST_DEATH_NOTIFICATION: u32 = iow(BC_MAGIC, 14, 8);
pub const BC_CLEAR_DEATH_NOTIFICATION: u32 = iow(BC_MAGIC, 15, 8);
pub const BC_DEAD_BINDER_DONE: u32 = iow(BC_MAGIC, 16, 8);
pub const BC_TRANSACTION_SG: u32 = iow(BC_MAGIC, 17, 64);
pub const BC_REPLY_SG: u32 = iow(BC_MAGIC, 18, 64);

pub const BR_ERROR: u32 = ior(BR_MAGIC, 0, 4);
pub const BR_OK: u32 = io(BR_MAGIC, 1);
pub const BR_TRANSACTION: u32 = ior(BR_MAGIC, 2, 56);
pub const BR_TRANSACTION_SG: u32 = ior(BR_MAGIC, 2, 64);
pub const BR_REPLY: u32 = ior(BR_MAGIC, 3, 56);
pub const BR_REPLY_SG: u32 = ior(BR_MAGIC, 3, 64);
pub const BR_ACQUIRE_RESULT: u32 = ior(BR_MAGIC, 4, 4);
pub const BR_DEAD_REPLY: u32 = io(BR_MAGIC, 5);
pub const BR_TRANSACTION_COMPLETE: u32 = io(BR_MAGIC, 6);
pub const BR_INCREFS: u32 = io(BR_MAGIC, 8);
pub const BR_ACQUIRE: u32 = io(BR_MAGIC, 9);
pub const BR_RELEASE: u32 = io(BR_MAGIC, 10);
pub const BR_DECREFS: u32 = io(BR_MAGIC, 11);
pub const BR_NOOP: u32 = io(BR_MAGIC, 12);
pub const BR_SPAWN_LOOPER: u32 = io(BR_MAGIC, 13);
pub const BR_FAILED_REPLY: u32 = io(BR_MAGIC, 14);

pub const DRM_MODE_GETRESOURCES: u32 = iowr(DRM_MAGIC, 0xA0, 64);
pub const DRM_MODE_GETCRTC: u32 = iowr(DRM_MAGIC, 0xA1, 104);
pub const DRM_MODE_SETCRTC: u32 = iowr(DRM_MAGIC, 0xA2, 104);
pub const DRM_MODE_GETCONNECTOR: u32 = iowr(DRM_MAGIC, 0xA7, 80);
pub const DRM_MODE_GETENCODER: u32 = iowr(DRM_MAGIC, 0xA6, 24);
pub const DRM_MODE_RMFB: u32 = iowr(DRM_MAGIC, 0xAF, 4);
pub const DRM_MODE_PAGE_FLIP: u32 = iowr(DRM_MAGIC, 0xB0, 24);
pub const DRM_MODE_CREATE_DUMB: u32 = iowr(DRM_MAGIC, 0xB2, 32);
pub const DRM_MODE_MAP_DUMB: u32 = iowr(DRM_MAGIC, 0xB3, 16);
pub const DRM_MODE_DESTROY_DUMB: u32 = iowr(DRM_MAGIC, 0xB4, 4);
pub const DRM_MODE_ADDFB2: u32 = iowr(DRM_MAGIC, 0xB8, 104);

#[cfg(test)]
mod tests {
    use super::*;

    const fn check_dir(code: u32, expected_dir: u32) -> u32 {
        (code >> 30) & expected_dir
    }

    #[test]
    fn binder_ioctls_match_reference_values() {
        assert_eq!(BINDER_WRITE_READ, 0xC0306201);
        assert_eq!(BINDER_SET_MAX_THREADS, 0x40046205);
        assert_eq!(BINDER_VERSION, 0xC0046207);
        assert_eq!(BINDER_SET_CONTEXT_MGR, 0x40086206);
        assert_eq!(BINDER_THREAD_EXIT, 0x40046208);
    }

    #[test]
    fn binder_command_codes() {
        assert_eq!(BC_TRANSACTION, 0x40386300);
        assert_eq!(BC_TRANSACTION_SG, 0x40406311);
        assert_eq!(BC_REPLY, 0x40386301);
        assert_eq!(BC_FREE_BUFFER, 0x40086303);
        assert_eq!(BC_ENTER_LOOPER, 0x0000630C);
        assert_eq!(BC_EXIT_LOOPER, 0x0000630D);
        assert_eq!(BC_ACQUIRE, 0x00006305);
        assert_eq!(BC_RELEASE, 0x00006306);
    }

    #[test]
    fn binder_return_codes() {
        assert_eq!(BR_ERROR, 0x80047200);
        assert_eq!(BR_OK, 0x00007201);
        assert_eq!(BR_TRANSACTION, 0x80387202);
        assert_eq!(BR_TRANSACTION_SG, 0x80407202);
        assert_eq!(BR_REPLY, 0x80387203);
        assert_eq!(BR_REPLY_SG, 0x80407203);
        assert_eq!(BR_DEAD_REPLY, 0x00007205);
        assert_eq!(BR_TRANSACTION_COMPLETE, 0x00007206);
        assert_eq!(BR_NOOP, 0x0000720C);
        assert_eq!(BR_FAILED_REPLY, 0x0000720E);
        assert_eq!(BR_SPAWN_LOOPER, 0x0000720D);
    }

    #[test]
    fn dir_bits() {
        assert_eq!(check_dir(BINDER_WRITE_READ, 3), 3);
        assert_eq!(check_dir(BC_TRANSACTION, 1), 1);
        assert_eq!(check_dir(BR_OK, 0), 0);
    }

    #[test]
    fn drm_ioctls() {
        assert_eq!(DRM_MODE_GETRESOURCES, 0xC04064A0);
        assert_eq!(DRM_MODE_GETCRTC, 0xC06864A1);
        assert_eq!(DRM_MODE_SETCRTC, 0xC06864A2);
        assert_eq!(DRM_MODE_GETCONNECTOR, 0xC05064A7);
        assert_eq!(DRM_MODE_GETENCODER, 0xC01864A6);
        assert_eq!(DRM_MODE_PAGE_FLIP, 0xC01864B0);
        assert_eq!(DRM_MODE_CREATE_DUMB, 0xC02064B2);
        assert_eq!(DRM_MODE_MAP_DUMB, 0xC01064B3);
        assert_eq!(DRM_MODE_DESTROY_DUMB, 0xC00464B4);
    }
}
