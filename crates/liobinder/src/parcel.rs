pub const BINDER_TYPE_HANDLE: u32 = 0x73627248;
pub const BINDER_TYPE_BINDER: u32 = 0x73627242;

pub const TF_ACCEPT_FDS: u32 = 1 << 4;
pub const TF_ONE_WAY: u32 = 1 << 1;

pub const FLAT_FLAG_BINDER: u32 = 0x40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatBinderObject {
    pub kind: u32,
    pub flags: u32,
    pub handle: u32,
    pub cookie: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ParcelWriter {
    buf: Vec<u8>,
    offsets: Vec<u32>,
}

impl ParcelWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
        self.pad_to_4();
    }

    pub fn write_i32(&mut self, value: i32) {
        self.write_u32(value as u32);
    }

    pub fn write_u64(&mut self, value: u64) {
        self.pad_to_4();
        self.buf.extend_from_slice(&value.to_le_bytes());
        self.pad_to_4();
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
        self.pad_to_4();
    }

    pub fn pad_to_4(&mut self) {
        while !self.buf.len().is_multiple_of(4) {
            self.buf.push(0);
        }
    }

    pub fn write_string16(&mut self, value: &str) {
        let utf16: Vec<u16> = value.encode_utf16().collect();
        self.write_i32(utf16.len() as i32);
        for unit in &utf16 {
            self.buf.extend_from_slice(&unit.to_le_bytes());
        }
        self.pad_to_4();
    }

    pub fn write_interface_token(&mut self, descriptor: &str) {
        self.write_string16(descriptor);
    }

    pub fn write_object(&mut self, object: FlatBinderObject) {
        let mut raw = Vec::with_capacity(24);
        raw.extend_from_slice(&object.kind.to_le_bytes());
        raw.extend_from_slice(&object.flags.to_le_bytes());
        raw.extend_from_slice(&object.handle.to_le_bytes());
        raw.extend_from_slice(&0u32.to_le_bytes());
        raw.extend_from_slice(&object.cookie.to_le_bytes());
        self.offsets.push(self.buf.len() as u32);
        self.buf.extend_from_slice(&raw);
        self.pad_to_4();
    }

    pub fn data(&self) -> &[u8] {
        &self.buf
    }

    pub fn offsets(&self) -> &[u32] {
        &self.offsets
    }

    pub fn into_parts(self) -> (Vec<u8>, Vec<u32>) {
        (self.buf, self.offsets)
    }
}

#[derive(Debug)]
pub struct ParcelReader<'a> {
    buf: &'a [u8],
    offsets: &'a [u32],
    pos: usize,
}

impl<'a> ParcelReader<'a> {
    pub fn new(buf: &'a [u8], offsets: &'a [u32]) -> Self {
        Self {
            buf,
            offsets,
            pos: 0,
        }
    }

    pub fn read_u32(&mut self) -> Result<u32, ParcelError> {
        if self.pos + 4 > self.buf.len() {
            return Err(ParcelError::Truncated);
        }
        let bytes: [u8; 4] = self.buf[self.pos..self.pos + 4]
            .try_into()
            .map_err(|_| ParcelError::Truncated)?;
        self.pos += 4;
        Ok(u32::from_le_bytes(bytes))
    }

    pub fn read_u64(&mut self) -> Result<u64, ParcelError> {
        while !self.pos.is_multiple_of(8) {
            self.pos += 1;
        }
        if self.pos + 8 > self.buf.len() {
            return Err(ParcelError::Truncated);
        }
        let bytes: [u8; 8] = self.buf[self.pos..self.pos + 8]
            .try_into()
            .map_err(|_| ParcelError::Truncated)?;
        self.pos += 8;
        Ok(u64::from_le_bytes(bytes))
    }

    pub fn read_string16(&mut self) -> Result<String, ParcelError> {
        let len = self.read_u32()?;
        let byte_len = (len as usize).checked_mul(2).ok_or(ParcelError::TooLarge)?;
        if self.pos + byte_len > self.buf.len() {
            return Err(ParcelError::Truncated);
        }
        let units: Vec<u16> = self.buf[self.pos..self.pos + byte_len]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        self.pos += byte_len;
        while !self.pos.is_multiple_of(4) {
            self.pos += 1;
        }
        String::from_utf16(&units).map_err(|_| ParcelError::InvalidUtf16)
    }

    pub fn read_object_at(&self, index: usize) -> Result<FlatBinderObject, ParcelError> {
        let Some(&offset) = self.offsets.get(index) else {
            return Err(ParcelError::NoObject);
        };
        let start = offset as usize;
        if start + 24 > self.buf.len() {
            return Err(ParcelError::Truncated);
        }
        let kind = u32::from_le_bytes(
            self.buf[start..start + 4]
                .try_into()
                .map_err(|_| ParcelError::Truncated)?,
        );
        let flags = u32::from_le_bytes(
            self.buf[start + 4..start + 8]
                .try_into()
                .map_err(|_| ParcelError::Truncated)?,
        );
        let handle = u32::from_le_bytes(
            self.buf[start + 8..start + 12]
                .try_into()
                .map_err(|_| ParcelError::Truncated)?,
        );
        let cookie = u64::from_le_bytes(
            self.buf[start + 16..start + 24]
                .try_into()
                .map_err(|_| ParcelError::Truncated)?,
        );
        Ok(FlatBinderObject {
            kind,
            flags,
            handle,
            cookie,
        })
    }

    pub fn first_object(&self) -> Result<FlatBinderObject, ParcelError> {
        self.read_object_at(0)
    }

    pub fn object_count(&self) -> usize {
        self.offsets.len()
    }

    pub fn position(&self) -> usize {
        self.pos
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParcelError {
    Truncated,
    TooLarge,
    InvalidUtf16,
    NoObject,
}

impl std::fmt::Display for ParcelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParcelError::Truncated => write!(f, "parcel data truncated"),
            ParcelError::TooLarge => write!(f, "parcel field too large"),
            ParcelError::InvalidUtf16 => write!(f, "string16 is not valid utf16"),
            ParcelError::NoObject => write!(f, "parcel contains no object"),
        }
    }
}

pub fn handle_object(handle: u32) -> FlatBinderObject {
    FlatBinderObject {
        kind: BINDER_TYPE_HANDLE,
        flags: FLAT_FLAG_BINDER,
        handle,
        cookie: 0,
    }
}

pub fn binder_object(cookie: u64) -> FlatBinderObject {
    FlatBinderObject {
        kind: BINDER_TYPE_BINDER,
        flags: FLAT_FLAG_BINDER | 0x04 | 0x08,
        handle: 0,
        cookie,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_type_values() {
        assert_eq!(BINDER_TYPE_HANDLE, 0x73627248);
        assert_eq!(BINDER_TYPE_BINDER, 0x73627242);
        assert_eq!(TF_ACCEPT_FDS, 0x10);
        assert_eq!(TF_ONE_WAY, 0x2);
    }

    #[test]
    fn u32_and_padding() {
        let mut w = ParcelWriter::new();
        w.write_u32(0x11223344);
        w.write_u32(0x55667788);
        assert_eq!(w.data(), &[0x44, 0x33, 0x22, 0x11, 0x88, 0x77, 0x66, 0x55]);
    }

    #[test]
    fn u64_is_aligned() {
        let mut w = ParcelWriter::new();
        w.write_u32(7);
        w.write_u64(0x0102030405060708);
        assert_eq!(w.data().len(), 12);
        assert_eq!(w.data()[4], 0x08);
        assert_eq!(w.data()[11], 0x01);
    }

    #[test]
    fn string16_roundtrip() {
        let mut w = ParcelWriter::new();
        w.write_string16("hello");
        let raw = w.data();
        assert_eq!(raw[0..4], 5i32.to_le_bytes());
        assert_eq!(raw[4], b'h');
        assert_eq!(raw[5], 0);
        let mut r = ParcelReader::new(raw, &[]);
        assert_eq!(r.read_string16().unwrap(), "hello");
    }

    #[test]
    fn string16_pads_to_four() {
        let mut w = ParcelWriter::new();
        w.write_string16("ab");
        let raw = w.data();
        assert_eq!(raw.len(), 8);
        assert_eq!(raw[4], b'a');
        assert_eq!(raw[5], 0);
        assert_eq!(raw[6], b'b');
        assert_eq!(raw[7], 0);
    }

    #[test]
    fn interface_token_matches_descriptor() {
        let mut w = ParcelWriter::new();
        w.write_interface_token("android.os.IServiceManager");
        let raw = w.data();
        let mut r = ParcelReader::new(raw, &[]);
        assert_eq!(r.read_string16().unwrap(), "android.os.IServiceManager");
    }

    #[test]
    fn object_offsets_track_position() {
        let mut w = ParcelWriter::new();
        w.write_i32(1);
        w.write_object(handle_object(0x42));
        w.write_i32(2);
        let (raw, offsets) = w.into_parts();
        assert_eq!(offsets, [4]);
        assert_eq!(
            raw[offsets[0] as usize + 8..offsets[0] as usize + 12],
            0x42u32.to_le_bytes()
        );
        let r = ParcelReader::new(&raw, &offsets);
        let object = r.first_object().unwrap();
        assert_eq!(object.kind, BINDER_TYPE_HANDLE);
        assert_eq!(object.handle, 0x42);
        assert_eq!(r.object_count(), 1);
    }

    #[test]
    fn binder_object_carries_cookie() {
        let mut w = ParcelWriter::new();
        w.write_object(binder_object(0xABCDEF));
        let (raw, offsets) = w.into_parts();
        let r = ParcelReader::new(&raw, &offsets);
        let object = r.first_object().unwrap();
        assert_eq!(object.kind, BINDER_TYPE_BINDER);
        assert_eq!(object.cookie, 0xABCDEF);
    }

    #[test]
    fn truncated_reads_fail() {
        let mut w = ParcelWriter::new();
        w.write_u32(1);
        let raw = w.data();
        let mut r = ParcelReader::new(raw, &[]);
        assert_eq!(r.read_u32().unwrap(), 1);
        assert_eq!(r.read_u32(), Err(ParcelError::Truncated));
        assert_eq!(r.first_object(), Err(ParcelError::NoObject));
    }

    #[test]
    fn utf16_supplementary_plane_roundtrip() {
        let mut w = ParcelWriter::new();
        w.write_string16("a\u{1F600}b");
        let raw = w.data();
        let mut r = ParcelReader::new(raw, &[]);
        assert_eq!(r.read_string16().unwrap(), "a\u{1F600}b");
    }
}
