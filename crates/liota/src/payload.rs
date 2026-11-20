use crate::b64;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Seek, Write};
use std::path::Path;

pub const MAGIC: &str = "AFFUPD1";
pub const HASH_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub version: String,
    pub size: u64,
    pub sha256: String,
}

fn read_line(reader: &mut impl BufRead) -> io::Result<String> {
    let mut line = Vec::new();
    let n = reader.read_until(b'\n', &mut line)?;
    if n == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "payload truncated",
        ));
    }
    while line.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
        line.pop();
    }
    String::from_utf8(line)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "payload line is not utf8"))
}

pub fn load_signing_key(path: &str) -> io::Result<SigningKey> {
    let raw = fs::read_to_string(path)?;
    let bytes = b64::decode(raw.trim())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad key encoding"))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "signing key must be 32 bytes"))?;
    Ok(SigningKey::from_bytes(&bytes))
}

pub fn load_verifying_key(path: &str) -> io::Result<VerifyingKey> {
    let raw = fs::read_to_string(path)?;
    let bytes = b64::decode(raw.trim())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad key encoding"))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "public key must be 32 bytes"))?;
    VerifyingKey::from_bytes(&bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid public key"))
}

pub fn keygen(key_path: &str, pub_path: &str) -> io::Result<()> {
    let mut seed = [0u8; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut seed)?;
    let key = SigningKey::from_bytes(&seed);
    write_key(key_path, &key.to_bytes())?;
    write_key(pub_path, &key.verifying_key().to_bytes())
}

fn write_key(path: &str, bytes: &[u8; 32]) -> io::Result<()> {
    if let Some(dir) = Path::new(path).parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, format!("{}\n", b64::encode(bytes)))
}

pub fn build(
    image_path: &str,
    version: &str,
    key_path: &str,
    out_path: &str,
) -> io::Result<Manifest> {
    let key = load_signing_key(key_path)?;
    let mut image = fs::File::open(image_path)?;
    let mut size = 0u64;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = image.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    let manifest = Manifest {
        version: version.to_string(),
        size,
        sha256: hex(&hasher.finalize()),
    };
    let manifest_line = serde_json::to_string(&manifest).map_err(io::Error::other)?;
    let sig = key.sign(manifest_line.as_bytes());
    let mut out = fs::File::create(out_path)?;
    out.write_all(MAGIC.as_bytes())?;
    out.write_all(b"\n")?;
    out.write_all(manifest_line.as_bytes())?;
    out.write_all(b"\n")?;
    out.write_all(b64::encode(&sig.to_bytes()).as_bytes())?;
    out.write_all(b"\n")?;
    image.seek(io::SeekFrom::Start(0))?;
    io::copy(&mut image, &mut out)?;
    out.sync_all()?;
    Ok(manifest)
}

pub fn verify_file(path: &str, pub_key: &VerifyingKey) -> io::Result<Manifest> {
    let mut reader = BufReader::new(fs::File::open(path)?);
    let magic = read_line(&mut reader)?;
    if magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "payload magic mismatch",
        ));
    }
    let manifest_line = read_line(&mut reader)?;
    let manifest: Manifest = serde_json::from_str(&manifest_line)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad manifest: {e}")))?;
    let sig_line = read_line(&mut reader)?;
    let sig_bytes = b64::decode(&sig_line)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad signature encoding"))?;
    let sig_bytes: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "signature must be 64 bytes"))?;
    let sig = Signature::from_bytes(&sig_bytes);
    pub_key
        .verify(manifest_line.as_bytes(), &sig)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "signature verification failed"))?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buf = vec![0u8; 1024 * 1024];
    while size < manifest.size {
        let want = (manifest.size - size).min(buf.len() as u64) as usize;
        let n = reader.read(&mut buf[..want])?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    if size != manifest.size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("image size {}: manifest says {}", size, manifest.size),
        ));
    }
    let digest = hex(&hasher.finalize());
    if digest != manifest.sha256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "image hash mismatch",
        ));
    }
    Ok(manifest)
}

pub fn verify(payload_path: &str, pub_key_path: &str) -> io::Result<Manifest> {
    let key = load_verifying_key(pub_key_path)?;
    verify_file(payload_path, &key)
}

pub fn header_len(path: &str) -> io::Result<u64> {
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut len = 0u64;
    for _ in 0..3 {
        let mut line = Vec::new();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "payload truncated",
            ));
        }
        len += n as u64;
    }
    Ok(len)
}

pub fn install_image(
    payload: &str,
    pub_key_path: &str,
    partition: &str,
) -> io::Result<(Manifest, u64)> {
    let manifest = verify(payload, pub_key_path)?;
    let offset = header_len(payload)?;
    let mut source = fs::File::open(payload)?;
    source.seek(io::SeekFrom::Start(offset))?;
    let mut target = fs::OpenOptions::new().write(true).open(partition)?;
    let mut buf = vec![0u8; 1024 * 1024];
    let mut total = 0u64;
    while total < manifest.size {
        let n = source.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let take = n.min((manifest.size - total) as usize);
        target.write_all(&buf[..take])?;
        total += take as u64;
    }
    if total != manifest.size {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!(
                "payload image shorter than manifest: {} of {}",
                total, manifest.size
            ),
        ));
    }
    target.sync_all()?;
    Ok((manifest, total))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::id;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{tag}-{}", id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fixed_key() -> SigningKey {
        let mut seed = [0u8; 32];
        for (i, b) in seed.iter_mut().enumerate() {
            *b = (i * 11 + 5) as u8;
        }
        SigningKey::from_bytes(&seed)
    }

    fn write_key_files(dir: &Path, key: &SigningKey) -> (String, String) {
        let key_path = dir.join("update.key");
        let pub_path = dir.join("update.pub");
        fs::write(&key_path, format!("{}\n", b64::encode(&key.to_bytes()))).unwrap();
        fs::write(
            &pub_path,
            format!("{}\n", b64::encode(&key.verifying_key().to_bytes())),
        )
        .unwrap();
        (
            key_path.to_string_lossy().to_string(),
            pub_path.to_string_lossy().to_string(),
        )
    }

    fn flip_byte(path: &Path, offset: u64) -> std::path::PathBuf {
        let flipped = path.with_extension("flipped");
        let mut data = fs::read(path).unwrap();
        data[offset as usize] ^= 0x01;
        fs::write(&flipped, data).unwrap();
        flipped
    }

    #[test]
    fn sign_then_verify_roundtrip() {
        let dir = temp_dir("liota-sign1");
        let key = fixed_key();
        let (key_path, pub_path) = write_key_files(&dir, &key);
        let image = dir.join("image.bin");
        let body: Vec<u8> = (0..200_000u32).map(|i| (i * 13) as u8).collect();
        fs::write(&image, &body).unwrap();
        let payload = dir.join("update.bin");
        let manifest = build(
            image.to_str().unwrap(),
            "0.8.0",
            &key_path,
            payload.to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.size, 200_000);
        assert_eq!(manifest.sha256.len(), HASH_LEN);
        let checked = verify(payload.to_str().unwrap(), &pub_path).unwrap();
        assert_eq!(checked, manifest);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_rejects_flipped_image_byte() {
        let dir = temp_dir("liota-sign2");
        let key = fixed_key();
        let (key_path, pub_path) = write_key_files(&dir, &key);
        let image = dir.join("image.bin");
        fs::write(&image, vec![9u8; 50_000]).unwrap();
        let payload = dir.join("update.bin");
        build(
            image.to_str().unwrap(),
            "0.8.0",
            &key_path,
            payload.to_str().unwrap(),
        )
        .unwrap();
        let flipped = flip_byte(&payload, 200);
        assert!(verify(flipped.to_str().unwrap(), &pub_path).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_rejects_flipped_manifest_and_sig() {
        let dir = temp_dir("liota-sign3");
        let key = fixed_key();
        let (key_path, pub_path) = write_key_files(&dir, &key);
        let image = dir.join("image.bin");
        fs::write(&image, vec![1u8; 10_000]).unwrap();
        let payload = dir.join("update.bin");
        build(
            image.to_str().unwrap(),
            "0.8.0",
            &key_path,
            payload.to_str().unwrap(),
        )
        .unwrap();
        let manifest_flipped = flip_byte(&payload, 9);
        assert!(verify(manifest_flipped.to_str().unwrap(), &pub_path).is_err());
        let payload2 = dir.join("update2.bin");
        build(
            image.to_str().unwrap(),
            "0.8.0",
            &key_path,
            payload2.to_str().unwrap(),
        )
        .unwrap();
        let sig_flipped = flip_byte(&payload2, 90);
        assert!(verify(sig_flipped.to_str().unwrap(), &pub_path).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_rejects_bad_magic_and_wrong_key() {
        let dir = temp_dir("liota-sign4");
        let key = fixed_key();
        let (key_path, pub_path) = write_key_files(&dir, &key);
        let image = dir.join("image.bin");
        fs::write(&image, vec![2u8; 1_000]).unwrap();
        let payload = dir.join("update.bin");
        build(
            image.to_str().unwrap(),
            "0.8.0",
            &key_path,
            payload.to_str().unwrap(),
        )
        .unwrap();
        let mut text = fs::read_to_string(&payload).unwrap();
        text = text.replacen(MAGIC, "XXXXXXX", 1);
        fs::write(dir.join("bad-magic.bin"), text).unwrap();
        assert!(verify(dir.join("bad-magic.bin").to_str().unwrap(), &pub_path).is_err());
        let mut other_seed = [0u8; 32];
        for (i, b) in other_seed.iter_mut().enumerate() {
            *b = (i * 17 + 23) as u8;
        }
        let other = SigningKey::from_bytes(&other_seed);
        let (_, other_pub) = write_key_files(&dir, &other);
        assert!(verify(payload.to_str().unwrap(), &other_pub).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_rejects_truncated_image() {
        let dir = temp_dir("liota-sign5");
        let key = fixed_key();
        let (key_path, pub_path) = write_key_files(&dir, &key);
        let image = dir.join("image.bin");
        fs::write(&image, vec![3u8; 30_000]).unwrap();
        let payload = dir.join("update.bin");
        build(
            image.to_str().unwrap(),
            "0.8.0",
            &key_path,
            payload.to_str().unwrap(),
        )
        .unwrap();
        let full = fs::read(&payload).unwrap();
        fs::write(dir.join("trunc.bin"), &full[..full.len() - 1000]).unwrap();
        assert!(verify(dir.join("trunc.bin").to_str().unwrap(), &pub_path).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn keygen_produces_working_pair() {
        let dir = temp_dir("liota-sign6");
        let key_path = dir.join("nested/update.key");
        let pub_path = dir.join("nested/update.pub");
        keygen(key_path.to_str().unwrap(), pub_path.to_str().unwrap()).unwrap();
        let signing = load_signing_key(key_path.to_str().unwrap()).unwrap();
        let verifying = load_verifying_key(pub_path.to_str().unwrap()).unwrap();
        let msg = b"manifest";
        let sig = signing.sign(msg);
        assert!(verifying.verify(msg, &sig).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn image_bytes_survive_header() {
        let dir = temp_dir("liota-sign7");
        let key = fixed_key();
        let (key_path, _) = write_key_files(&dir, &key);
        let image = dir.join("image.bin");
        let body: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();
        fs::write(&image, &body).unwrap();
        let payload = dir.join("update.bin");
        build(
            image.to_str().unwrap(),
            "9.9.9",
            &key_path,
            payload.to_str().unwrap(),
        )
        .unwrap();
        let data = fs::read(&payload).unwrap();
        let header_len = data.len() - body.len();
        assert_eq!(&data[header_len..], &body[..]);
        assert_eq!(&data[..8], b"AFFUPD1\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_ignores_trailing_sector_pad() {
        let dir = temp_dir("liota-sign9");
        let key = fixed_key();
        let (key_path, pub_path) = write_key_files(&dir, &key);
        let image = dir.join("image.bin");
        fs::write(&image, vec![4u8; 100_000]).unwrap();
        let payload = dir.join("update.bin");
        build(
            image.to_str().unwrap(),
            "0.8.0",
            &key_path,
            payload.to_str().unwrap(),
        )
        .unwrap();
        let mut data = fs::read(&payload).unwrap();
        while !data.len().is_multiple_of(512) {
            data.push(0);
        }
        fs::write(&payload, &data).unwrap();
        assert!(verify(payload.to_str().unwrap(), &pub_path).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_image_writes_image_only() {
        let dir = temp_dir("liota-sign10");
        let key = fixed_key();
        let (key_path, pub_path) = write_key_files(&dir, &key);
        let body: Vec<u8> = (0..80_000u32).map(|i| (i * 3) as u8).collect();
        let image = dir.join("image.bin");
        fs::write(&image, &body).unwrap();
        let payload = dir.join("update.bin");
        build(
            image.to_str().unwrap(),
            "0.8.0",
            &key_path,
            payload.to_str().unwrap(),
        )
        .unwrap();
        let partition = dir.join("slot.img");
        fs::write(&partition, vec![0u8; 200_000]).unwrap();
        let (manifest, written) = install_image(
            payload.to_str().unwrap(),
            &pub_path,
            partition.to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.size, 80_000);
        assert_eq!(written, 80_000);
        let data = fs::read(&partition).unwrap();
        assert_eq!(&data[..80_000], &body[..]);
        assert_eq!(&data[80_000..], &vec![0u8; 120_000][..]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_image_rejects_tampered_payload() {
        let dir = temp_dir("liota-sign11");
        let key = fixed_key();
        let (key_path, pub_path) = write_key_files(&dir, &key);
        let image = dir.join("image.bin");
        fs::write(&image, vec![5u8; 20_000]).unwrap();
        let payload = dir.join("update.bin");
        build(
            image.to_str().unwrap(),
            "0.8.0",
            &key_path,
            payload.to_str().unwrap(),
        )
        .unwrap();
        let flipped = flip_byte(&payload, 300);
        let partition = dir.join("slot.img");
        fs::write(&partition, vec![0u8; 50_000]).unwrap();
        assert!(install_image(
            flipped.to_str().unwrap(),
            &pub_path,
            partition.to_str().unwrap()
        )
        .is_err());
        assert_eq!(fs::read(&partition).unwrap(), vec![0u8; 50_000]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn key_files_use_base64_of_32_bytes() {
        let dir = temp_dir("liota-sign8");
        let (key_path, pub_path) = write_key_files(&dir, &fixed_key());
        let key_raw = fs::read_to_string(&key_path).unwrap();
        let pub_raw = fs::read_to_string(&pub_path).unwrap();
        assert_eq!(b64::decode(key_raw.trim()).unwrap().len(), 32);
        assert_eq!(b64::decode(pub_raw.trim()).unwrap().len(), 32);
        let _ = fs::remove_dir_all(&dir);
    }
}
