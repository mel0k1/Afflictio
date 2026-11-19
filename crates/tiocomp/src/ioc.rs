const fn ioc(dir: u32, kind: u8, nr: u8, size: u32) -> u32 {
    (dir << 30) | ((size & ((1 << 14) - 1)) << 16) | ((kind as u32) << 8) | (nr as u32)
}

const _IOC_WRITE: u32 = 1;
const _IOC_READ: u32 = 2;
const DRM_MAGIC: u8 = b'd';

const fn iowr(kind: u8, nr: u8, size: u32) -> u32 {
    ioc(_IOC_READ | _IOC_WRITE, kind, nr, size)
}

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

    #[test]
    fn drm_ioctl_values() {
        assert_eq!(DRM_MODE_GETRESOURCES, 0xC04064A0);
        assert_eq!(DRM_MODE_GETCRTC, 0xC06864A1);
        assert_eq!(DRM_MODE_SETCRTC, 0xC06864A2);
        assert_eq!(DRM_MODE_GETCONNECTOR, 0xC05064A7);
        assert_eq!(DRM_MODE_GETENCODER, 0xC01864A6);
        assert_eq!(DRM_MODE_PAGE_FLIP, 0xC01864B0);
        assert_eq!(DRM_MODE_CREATE_DUMB, 0xC02064B2);
        assert_eq!(DRM_MODE_MAP_DUMB, 0xC01064B3);
        assert_eq!(DRM_MODE_DESTROY_DUMB, 0xC00464B4);
        assert_eq!(DRM_MODE_ADDFB2, 0xC06864B8);
    }
}
