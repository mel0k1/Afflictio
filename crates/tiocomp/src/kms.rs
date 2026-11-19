use std::fs;
use std::io;
use std::os::unix::io::{AsRawFd, RawFd};

pub const DRM_MODE_CONNECTED: u32 = 1;
pub const DRM_MODE_TYPE_PREFERRED: u32 = 1 << 4;
pub const DRM_FORMAT_XRGB8888: u32 = 0x34325258;

#[repr(C)]
pub struct ModeCardRes {
    pub fb_id_ptr: u64,
    pub crtc_id_ptr: u64,
    pub connector_id_ptr: u64,
    pub encoder_id_ptr: u64,
    pub count_fbs: u32,
    pub count_crtcs: u32,
    pub count_connectors: u32,
    pub count_encoders: u32,
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

#[repr(C)]
pub struct ModeGetConnector {
    pub encoders_ptr: u64,
    pub modes_ptr: u64,
    pub props_ptr: u64,
    pub prop_values_ptr: u64,
    pub count_modes: u32,
    pub count_props: u32,
    pub count_encoders: u32,
    pub encoder_id: u32,
    pub connector_id: u32,
    pub connector_type: u32,
    pub connector_type_id: u32,
    pub connection: u32,
    pub mm_width: u32,
    pub mm_height: u32,
    pub subpixel: u32,
    pub pad: u32,
}

#[repr(C)]
pub struct ModeGetEncoder {
    pub encoder_id: u32,
    pub encoder_type: u32,
    pub crtc_id: u32,
    pub possible_crtcs: u32,
    pub possible_clones: u32,
    pub name_length: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeInfo {
    pub clock: u32,
    pub hdisplay: u16,
    pub hsync_start: u16,
    pub hsync_end: u16,
    pub htotal: u16,
    pub hskew: u16,
    pub vdisplay: u16,
    pub vsync_start: u16,
    pub vsync_end: u16,
    pub vtotal: u16,
    pub vscan: u16,
    pub vrefresh: u32,
    pub flags: u32,
    pub type_: u32,
    pub name: [u8; 32],
}

impl ModeInfo {
    pub fn resolution(&self) -> (u32, u32) {
        (self.hdisplay as u32, self.vdisplay as u32)
    }

    pub fn mode_name(&self) -> String {
        let len = self.name.iter().position(|c| *c == 0).unwrap_or(32);
        String::from_utf8_lossy(&self.name[..len]).to_string()
    }
}

#[repr(C)]
pub struct ModeCrtc {
    pub set_connectors_ptr: u64,
    pub count_connectors: u32,
    pub crtc_id: u32,
    pub fb_id: u32,
    pub x: u32,
    pub y: u32,
    pub gamma_size: u32,
    pub mode_valid: u32,
    pub mode: ModeInfo,
}

#[repr(C)]
pub struct ModeFbCmd2 {
    pub fb_id: u32,
    pub width: u32,
    pub height: u32,
    pub pixel_format: u32,
    pub flags: u32,
    pub handles: [u32; 4],
    pub pitches: [u32; 4],
    pub offsets: [u32; 4],
    pub modifier: [u64; 4],
}

#[repr(C)]
pub struct ModeCrtcPageFlip {
    pub crtc_id: u32,
    pub fb_id: u32,
    pub flags: u32,
    pub reserved: u32,
    pub user_data: u64,
}

#[repr(C)]
pub struct ModeCreateDumb {
    pub height: u32,
    pub width: u32,
    pub bpp: u32,
    pub flags: u32,
    pub handle: u32,
    pub pitch: u32,
    pub size: u64,
}

#[repr(C)]
pub struct ModeMapDumb {
    pub handle: u32,
    pub pad: u32,
    pub offset: u64,
}

#[repr(C)]
pub struct ModeDestroyDumb {
    pub handle: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connector {
    pub id: u32,
    pub type_: u32,
    pub type_id: u32,
    pub connection: u32,
    pub name: String,
    pub modes: Vec<ModeInfo>,
}

#[derive(Debug)]
pub struct DumbBuffer {
    pub handle: u32,
    pub pitch: u32,
    pub size: u64,
    pub fb_id: u32,
    pub mapping: *mut libc::c_void,
    pub byte_len: usize,
}

unsafe impl Send for DumbBuffer {}
unsafe impl Sync for DumbBuffer {}

pub struct KmsDevice {
    file: fs::File,
}

impl KmsDevice {
    pub fn open(path: &str) -> io::Result<Self> {
        let file = fs::OpenOptions::new().read(true).write(true).open(path)?;
        Ok(Self { file })
    }

    pub fn scan() -> Option<String> {
        let Ok(dir) = fs::read_dir("/dev/dri") else {
            eprintln!("tiocompd: /dev/dri not found");
            return None;
        };
        let mut entries: Vec<_> = dir
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with("card"))
            .collect();
        entries.sort();
        for name in entries {
            let path = format!("/dev/dri/{name}");
            let Ok(device) = Self::open(&path) else {
                eprintln!("tiocompd: {path} open failed");
                continue;
            };
            if device.connected_connector().is_some() {
                return Some(path);
            }
        }
        None
    }

    pub fn resources(&self) -> io::Result<(IdLists, Bounds)> {
        let mut res = ModeCardRes {
            fb_id_ptr: 0,
            crtc_id_ptr: 0,
            connector_id_ptr: 0,
            encoder_id_ptr: 0,
            count_fbs: 0,
            count_crtcs: 0,
            count_connectors: 0,
            count_encoders: 0,
            min_width: 0,
            max_width: 0,
            min_height: 0,
            max_height: 0,
        };
        self_ioctl_get(
            self.file.as_raw_fd(),
            crate::ioc::DRM_MODE_GETRESOURCES,
            &mut res as *mut ModeCardRes as *mut libc::c_void,
        )?;
        let lists = fetch_id_lists(self.file.as_raw_fd(), &res)?;
        Ok((
            lists,
            (res.min_width, res.max_width, res.min_height, res.max_height),
        ))
    }

    pub fn connected_connector(&self) -> Option<Connector> {
        let Ok(((fbs, crtcs, connector_ids, encoders), _)) = self.resources() else {
            return None;
        };
        let _ = (fbs, crtcs, encoders);
        for id in connector_ids {
            let connector = match self.connector(id) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if connector.connection == DRM_MODE_CONNECTED && !connector.modes.is_empty() {
                return Some(connector);
            }
        }
        None
    }

    pub fn connector(&self, connector_id: u32) -> io::Result<Connector> {
        let mut probe_slot = empty_mode();
        let mut get = ModeGetConnector {
            encoders_ptr: 0,
            modes_ptr: &mut probe_slot as *mut ModeInfo as u64,
            props_ptr: 0,
            prop_values_ptr: 0,
            count_modes: 1,
            count_props: 0,
            count_encoders: 0,
            encoder_id: 0,
            connector_id,
            connector_type: 0,
            connector_type_id: 0,
            connection: 0,
            mm_width: 0,
            mm_height: 0,
            subpixel: 0,
            pad: 0,
        };
        self_ioctl_get(
            self.file.as_raw_fd(),
            crate::ioc::DRM_MODE_GETCONNECTOR,
            &mut get as *mut ModeGetConnector as *mut libc::c_void,
        )?;
        let mut modes = vec![empty_mode(); get.count_modes as usize];
        let mut encoders = vec![0u32; get.count_encoders as usize];
        let mut props = vec![0u32; get.count_props as usize];
        let mut prop_values = vec![0u64; get.count_props as usize];
        get.modes_ptr = modes.as_mut_ptr() as u64;
        get.encoders_ptr = encoders.as_mut_ptr() as u64;
        get.props_ptr = props.as_mut_ptr() as u64;
        get.prop_values_ptr = prop_values.as_mut_ptr() as u64;
        if let Err(e) = self_ioctl_get(
            self.file.as_raw_fd(),
            crate::ioc::DRM_MODE_GETCONNECTOR,
            &mut get as *mut ModeGetConnector as *mut libc::c_void,
        ) {
            eprintln!(
                "tiocompd: connector {connector_id} second call failed: {e} (modes {} props {} encoders {})",
                get.count_modes, get.count_props, get.count_encoders
            );
            return Err(e);
        }
        modes.truncate(get.count_modes as usize);
        Ok(Connector {
            id: get.connector_id,
            type_: get.connector_type,
            type_id: get.connector_type_id,
            connection: get.connection,
            name: connector_name(get.connector_type, get.connector_type_id),
            modes,
        })
    }

    pub fn encoder_crtc(&self, encoder_id: u32) -> io::Result<u32> {
        let mut get = ModeGetEncoder {
            encoder_id,
            encoder_type: 0,
            crtc_id: 0,
            possible_crtcs: 0,
            possible_clones: 0,
            name_length: 0,
        };
        self_ioctl_get(
            self.file.as_raw_fd(),
            crate::ioc::DRM_MODE_GETENCODER,
            &mut get as *mut ModeGetEncoder as *mut libc::c_void,
        )?;
        Ok(get.crtc_id)
    }

    pub fn current_crtc(&self, crtc_id: u32) -> io::Result<ModeCrtc> {
        let mut crtc = ModeCrtc {
            set_connectors_ptr: 0,
            count_connectors: 0,
            crtc_id,
            fb_id: 0,
            x: 0,
            y: 0,
            gamma_size: 0,
            mode_valid: 0,
            mode: empty_mode(),
        };
        self_ioctl_get(
            self.file.as_raw_fd(),
            crate::ioc::DRM_MODE_GETCRTC,
            &mut crtc as *mut ModeCrtc as *mut libc::c_void,
        )?;
        Ok(crtc)
    }

    pub fn create_dumb(&self, width: u32, height: u32) -> io::Result<ModeCreateDumb> {
        let mut dumb = ModeCreateDumb {
            height,
            width,
            bpp: 32,
            flags: 0,
            handle: 0,
            pitch: 0,
            size: 0,
        };
        self_ioctl_get(
            self.file.as_raw_fd(),
            crate::ioc::DRM_MODE_CREATE_DUMB,
            &mut dumb as *mut ModeCreateDumb as *mut libc::c_void,
        )?;
        Ok(dumb)
    }

    pub fn add_fb2(&self, width: u32, height: u32, handle: u32, pitch: u32) -> io::Result<u32> {
        let mut fb = ModeFbCmd2 {
            fb_id: 0,
            width,
            height,
            pixel_format: DRM_FORMAT_XRGB8888,
            flags: 0,
            handles: [handle, 0, 0, 0],
            pitches: [pitch, 0, 0, 0],
            offsets: [0, 0, 0, 0],
            modifier: [0, 0, 0, 0],
        };
        self_ioctl_get(
            self.file.as_raw_fd(),
            crate::ioc::DRM_MODE_ADDFB2,
            &mut fb as *mut ModeFbCmd2 as *mut libc::c_void,
        )?;
        Ok(fb.fb_id)
    }

    pub fn map_dumb(&self, handle: u32, size: u64) -> io::Result<DumbBuffer> {
        let mut map = ModeMapDumb {
            handle,
            pad: 0,
            offset: 0,
        };
        self_ioctl_get(
            self.file.as_raw_fd(),
            crate::ioc::DRM_MODE_MAP_DUMB,
            &mut map as *mut ModeMapDumb as *mut libc::c_void,
        )?;
        let byte_len = size as usize;
        let mapping = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                byte_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                self.file.as_raw_fd(),
                map.offset as libc::off_t,
            )
        };
        if mapping == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(DumbBuffer {
            handle,
            pitch: 0,
            size,
            fb_id: 0,
            mapping,
            byte_len,
        })
    }

    pub fn destroy_dumb(&self, handle: u32) {
        let mut destroy = ModeDestroyDumb { handle };
        let _ = self_ioctl_get(
            self.file.as_raw_fd(),
            crate::ioc::DRM_MODE_DESTROY_DUMB,
            &mut destroy as *mut ModeDestroyDumb as *mut libc::c_void,
        );
    }

    pub fn set_crtc(
        &self,
        crtc_id: u32,
        fb_id: u32,
        connector_id: u32,
        mode: ModeInfo,
    ) -> io::Result<()> {
        let mut connectors = [connector_id];
        let mut crtc = ModeCrtc {
            set_connectors_ptr: connectors.as_mut_ptr() as u64,
            count_connectors: 1,
            crtc_id,
            fb_id,
            x: 0,
            y: 0,
            gamma_size: 0,
            mode_valid: 1,
            mode,
        };
        self_ioctl_get(
            self.file.as_raw_fd(),
            crate::ioc::DRM_MODE_SETCRTC,
            &mut crtc as *mut ModeCrtc as *mut libc::c_void,
        )
    }

    pub fn page_flip(&self, crtc_id: u32, fb_id: u32) -> io::Result<()> {
        let mut flip = ModeCrtcPageFlip {
            crtc_id,
            fb_id,
            flags: 0,
            reserved: 0,
            user_data: 0,
        };
        self_ioctl_get(
            self.file.as_raw_fd(),
            crate::ioc::DRM_MODE_PAGE_FLIP,
            &mut flip as *mut ModeCrtcPageFlip as *mut libc::c_void,
        )
    }

    pub fn rm_fb(&self, fb_id: u32) {
        let mut id = fb_id;
        let _ = self_ioctl_get(
            self.file.as_raw_fd(),
            crate::ioc::DRM_MODE_RMFB,
            &mut id as *mut u32 as *mut libc::c_void,
        );
    }
}

fn self_ioctl_get(fd: RawFd, cmd: u32, arg: *mut libc::c_void) -> io::Result<()> {
    let rc = unsafe { libc::ioctl(fd, cmd as libc::Ioctl, arg) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub type IdLists = (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>);
pub type Bounds = (u32, u32, u32, u32);

fn fetch_id_lists(fd: RawFd, res: &ModeCardRes) -> io::Result<IdLists> {
    let mut fbs = vec![0u32; res.count_fbs as usize];
    let mut crtcs = vec![0u32; res.count_crtcs as usize];
    let mut connectors = vec![0u32; res.count_connectors as usize];
    let mut encoders = vec![0u32; res.count_encoders as usize];
    let mut res2 = ModeCardRes {
        fb_id_ptr: fbs.as_mut_ptr() as u64,
        crtc_id_ptr: crtcs.as_mut_ptr() as u64,
        connector_id_ptr: connectors.as_mut_ptr() as u64,
        encoder_id_ptr: encoders.as_mut_ptr() as u64,
        count_fbs: fbs.len() as u32,
        count_crtcs: crtcs.len() as u32,
        count_connectors: connectors.len() as u32,
        count_encoders: encoders.len() as u32,
        min_width: 0,
        max_width: 0,
        min_height: 0,
        max_height: 0,
    };
    self_ioctl_get(
        fd,
        crate::ioc::DRM_MODE_GETRESOURCES,
        &mut res2 as *mut ModeCardRes as *mut libc::c_void,
    )?;
    Ok((fbs, crtcs, connectors, encoders))
}

fn connector_name(type_: u32, type_id: u32) -> String {
    let kind = match type_ {
        4 => "DVI",
        5 => "Composite",
        6 => "SVIDEO",
        7 => "LVDS",
        8 => "Component",
        9 => "DIN",
        10 => "DP",
        11 => "HDMI",
        12 => "TV",
        13 => "eDP",
        14 => "VGA",
        15 => "Virtual",
        16 => "DSI",
        17 => "DPI",
        18 => "Writeback",
        19 => "SPI",
        _ => "Conn",
    };
    format!("{kind}-{type_id}")
}

pub fn preferred_mode(modes: &[ModeInfo]) -> ModeInfo {
    if let Some(mode) = modes
        .iter()
        .find(|m| m.type_ & DRM_MODE_TYPE_PREFERRED != 0)
    {
        *mode
    } else {
        modes[0]
    }
}

impl DumbBuffer {
    pub fn pixels(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.mapping as *mut u32, self.byte_len / 4) }
    }
}

pub fn empty_mode() -> ModeInfo {
    ModeInfo {
        clock: 0,
        hdisplay: 0,
        hsync_start: 0,
        hsync_end: 0,
        htotal: 0,
        hskew: 0,
        vdisplay: 0,
        vsync_start: 0,
        vsync_end: 0,
        vtotal: 0,
        vscan: 0,
        vrefresh: 0,
        flags: 0,
        type_: 0,
        name: [0; 32],
    }
}
