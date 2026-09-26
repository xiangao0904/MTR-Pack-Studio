use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const FILE_MAGIC: &[u8; 8] = b"MTRPACK\0";
const RECORD_MAGIC: &[u8; 4] = b"MOBJ";
const CONTAINER_VERSION: u16 = 1;
const SCHEMA_VERSION: u32 = 1;
const HEADER_SIZE: usize = 256;
const SLOT_SIZE: usize = 96;
const SLOT_A_OFFSET: u64 = 32;
const SLOT_B_OFFSET: u64 = 128;
const RECORD_HEADER_SIZE: usize = 96;
const RECORD_VERSION: u16 = 1;
const KIND_BLOB: u16 = 1;
const KIND_INDEX: u16 = 2;
const CODEC_RAW: u8 = 0;
const CODEC_ZSTD: u8 = 1;
const MAX_INDEX_SIZE: u64 = 64 * 1024 * 1024;
const MAX_OBJECT_SIZE: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContentEntry {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub file: String,
    pub updated_at: u128,
    #[serde(default)]
    pub resources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobLocation {
    pub offset: u64,
    pub raw_len: u64,
    pub stored_len: u64,
    pub codec: u8,
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndex {
    pub schema_version: u32,
    pub generation: u64,
    pub previous_index_offset: u64,
    pub project_id: Uuid,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub cover_hash: Option<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    pub content: Vec<ContentEntry>,
    #[serde(default)]
    pub assets: BTreeMap<String, String>,
    #[serde(default)]
    pub texture_names: BTreeMap<String, String>,
    #[serde(default)]
    pub asset_thumbnail_hashes: BTreeMap<String, String>,
    pub blobs: BTreeMap<String, BlobLocation>,
}

#[derive(Debug, Clone, Copy)]
struct Checkpoint {
    generation: u64,
    index_offset: u64,
    stored_len: u64,
    raw_len: u64,
    index_hash: [u8; 32],
}

#[derive(Debug)]
struct RecordHeader {
    kind: u16,
    codec: u8,
    raw_len: u64,
    stored_len: u64,
    raw_hash: [u8; 32],
    stored_hash: [u8; 32],
}

#[derive(Debug)]
pub struct Container {
    path: PathBuf,
    file: File,
    project_id: Uuid,
    active_slot: usize,
    active_index_offset: u64,
    pub index: ProjectIndex,
    pub recovered: bool,
}

impl Container {
    pub fn create(path: &Path, name: &str) -> Result<Self, String> {
        let path = with_extension(path);
        let parent = path
            .parent()
            .ok_or_else(|| "Choose a valid project file location.".to_string())?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        cleanup_temporary_files(&path);
        let temporary = temporary_path(&path, "create");
        let project_id = Uuid::new_v4();
        let mut file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        write_empty_header(&mut file, project_id)?;
        let index = ProjectIndex {
            schema_version: SCHEMA_VERSION,
            generation: 0,
            previous_index_offset: 0,
            project_id,
            name: name.to_string(),
            description: String::new(),
            target: String::new(),
            cover_hash: None,
            namespace: crate::domain::slugify(name, "mtr_pack"),
            content: Vec::new(),
            assets: BTreeMap::new(),
            texture_names: BTreeMap::new(),
            asset_thumbnail_hashes: BTreeMap::new(),
            blobs: BTreeMap::new(),
        };
        let mut container = Self {
            path: temporary.clone(),
            file,
            project_id,
            active_slot: 1,
            active_index_offset: 0,
            index,
            recovered: false,
        };
        container.commit()?;
        container.file.sync_all().map_err(|e| e.to_string())?;
        drop(container.file);
        atomic_replace(&temporary, &path)?;
        Self::open(&path)
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        let path = path
            .canonicalize()
            .map_err(|e| format!("Unable to open project file: {e}"))?;
        if !is_project_path(&path) || !path.is_file() {
            return Err("Choose an MTR Pack Studio .mtrpack file.".into());
        }
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let (project_id, raw_slots) = read_header(&mut file)?;
        let mut valid = Vec::new();
        for (slot, checkpoint) in raw_slots.iter().enumerate() {
            if let Some(checkpoint) = checkpoint {
                if let Ok(index) = read_index(&mut file, checkpoint) {
                    if index.project_id == project_id {
                        valid.push((slot, *checkpoint, index));
                    }
                }
            }
        }
        let (active_slot, checkpoint, index) = valid
            .into_iter()
            .max_by_key(|(_, checkpoint, _)| checkpoint.generation)
            .ok_or_else(|| "This project has no valid recovery checkpoint.".to_string())?;
        if index.schema_version > SCHEMA_VERSION {
            return Err("This project was created by a newer version of MTR Pack Studio.".into());
        }
        let highest_declared = raw_slots
            .iter()
            .flatten()
            .map(|slot| slot.generation)
            .max()
            .unwrap_or(0);
        let selected_end = checkpoint.index_offset + aligned_record_size(checkpoint.stored_len);
        let recovered = highest_declared > checkpoint.generation
            || file.metadata().map_err(|e| e.to_string())?.len() > selected_end;
        Ok(Self {
            path,
            file,
            project_id,
            active_slot,
            active_index_offset: checkpoint.index_offset,
            index,
            recovered,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn flush(&mut self) -> Result<(), String> {
        self.file.sync_all().map_err(|e| e.to_string())
    }

    pub fn put_blob(&mut self, bytes: &[u8], media_type: &str) -> Result<String, String> {
        if bytes.len() as u64 > MAX_OBJECT_SIZE {
            return Err("Resource is too large for this project format.".into());
        }
        let hash = blake3::hash(bytes);
        let key = hash.to_hex().to_string();
        if self.index.blobs.contains_key(&key) {
            return Ok(key);
        }
        let compress = should_compress(media_type);
        let (codec, stored) = encode_payload(bytes, compress)?;
        let offset = write_record(&mut self.file, KIND_BLOB, codec, bytes, &stored)?;
        self.index.blobs.insert(
            key.clone(),
            BlobLocation {
                offset,
                raw_len: bytes.len() as u64,
                stored_len: stored.len() as u64,
                codec,
                media_type: media_type.into(),
            },
        );
        Ok(key)
    }

    pub fn read_blob(&mut self, hash: &str) -> Result<Vec<u8>, String> {
        let location = self
            .index
            .blobs
            .get(hash)
            .cloned()
            .ok_or_else(|| "Project resource is missing from the index.".to_string())?;
        let (header, bytes) = read_record(&mut self.file, location.offset)?;
        if header.kind != KIND_BLOB
            || blake3::Hash::from_hex(hash)
                .map_err(|_| "Invalid resource hash.".to_string())?
                .as_bytes()
                != &header.raw_hash
        {
            return Err("Project resource index is invalid.".into());
        }
        Ok(bytes)
    }

    pub fn commit(&mut self) -> Result<(), String> {
        self.index.schema_version = SCHEMA_VERSION;
        self.index.generation = self
            .index
            .generation
            .checked_add(1)
            .ok_or_else(|| "Project generation overflow.".to_string())?;
        self.index.previous_index_offset = self.active_index_offset;
        let raw = rmp_serde::to_vec_named(&self.index).map_err(|e| e.to_string())?;
        if raw.len() as u64 > MAX_INDEX_SIZE {
            return Err("Project index is too large.".into());
        }
        let stored = zstd::bulk::compress(&raw, 3).map_err(|e| e.to_string())?;
        self.file
            .seek(SeekFrom::End(0))
            .map_err(|e| e.to_string())?;
        let offset = write_record(&mut self.file, KIND_INDEX, CODEC_ZSTD, &raw, &stored)?;
        self.file.sync_data().map_err(|e| e.to_string())?;
        let checkpoint = Checkpoint {
            generation: self.index.generation,
            index_offset: offset,
            stored_len: stored.len() as u64,
            raw_len: raw.len() as u64,
            index_hash: *blake3::hash(&raw).as_bytes(),
        };
        let next_slot = 1 - self.active_slot;
        write_checkpoint(&mut self.file, next_slot, &checkpoint)?;
        self.file.sync_all().map_err(|e| e.to_string())?;
        self.active_slot = next_slot;
        self.active_index_offset = offset;
        self.recovered = false;
        Ok(())
    }

    pub fn stale_bytes(&self) -> Result<u64, String> {
        let file_size = self.file.metadata().map_err(|e| e.to_string())?.len();
        let live_blobs: u64 = self
            .index
            .blobs
            .values()
            .map(|blob| aligned_record_size(blob.stored_len))
            .sum();
        let live = HEADER_SIZE as u64
            + live_blobs
            + aligned_record_size(estimated_index_record_size(&self.index)?);
        Ok(file_size.saturating_sub(live))
    }

    pub fn should_compact(&self) -> Result<bool, String> {
        let size = self.file.metadata().map_err(|e| e.to_string())?.len();
        let stale = self.stale_bytes()?;
        Ok(stale > 64 * 1024 * 1024 && stale.saturating_mul(100) > size.saturating_mul(30))
    }

    pub fn compact(&mut self) -> Result<(), String> {
        let temporary = temporary_path(&self.path, "compact");
        let mut output = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        write_empty_header(&mut output, self.project_id)?;
        let mut fresh_index = self.index.clone();
        fresh_index.generation = 0;
        fresh_index.previous_index_offset = 0;
        fresh_index.blobs.clear();
        for (hash, location) in self.index.blobs.clone() {
            let bytes = self.read_blob(&hash)?;
            let (codec, stored) = encode_payload(&bytes, should_compress(&location.media_type))?;
            let offset = write_record(&mut output, KIND_BLOB, codec, &bytes, &stored)?;
            fresh_index.blobs.insert(
                hash,
                BlobLocation {
                    offset,
                    raw_len: bytes.len() as u64,
                    stored_len: stored.len() as u64,
                    codec,
                    media_type: location.media_type,
                },
            );
        }
        fresh_index.generation = 1;
        let raw = rmp_serde::to_vec_named(&fresh_index).map_err(|e| e.to_string())?;
        let stored = zstd::bulk::compress(&raw, 3).map_err(|e| e.to_string())?;
        let index_offset = write_record(&mut output, KIND_INDEX, CODEC_ZSTD, &raw, &stored)?;
        output.sync_data().map_err(|e| e.to_string())?;
        write_checkpoint(
            &mut output,
            0,
            &Checkpoint {
                generation: 1,
                index_offset,
                stored_len: stored.len() as u64,
                raw_len: raw.len() as u64,
                index_hash: *blake3::hash(&raw).as_bytes(),
            },
        )?;
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output);
        let handle_path = temporary_path(&self.path, "handle");
        let placeholder = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&handle_path)
            .map_err(|e| e.to_string())?;
        let original = std::mem::replace(&mut self.file, placeholder);
        drop(original);
        if let Err(error) = atomic_replace(&temporary, &self.path) {
            let reopened = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.path)
                .map_err(|reopen_error| {
                    format!("{error}; failed to reopen original project: {reopen_error}")
                })?;
            let placeholder = std::mem::replace(&mut self.file, reopened);
            drop(placeholder);
            let _ = fs::remove_file(&handle_path);
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        let reopened = Self::open(&self.path)?;
        *self = reopened;
        let _ = fs::remove_file(handle_path);
        Ok(())
    }
}

fn default_namespace() -> String { "mtr_pack".into() }

pub fn is_project_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("mtrpack"))
}

pub fn has_project_magic(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic).is_ok() && &magic == FILE_MAGIC
}

pub fn with_extension(path: &Path) -> PathBuf {
    if is_project_path(path) {
        path.to_path_buf()
    } else {
        path.with_extension("mtrpack")
    }
}

fn write_empty_header(file: &mut File, project_id: Uuid) -> Result<(), String> {
    let mut header = [0u8; HEADER_SIZE];
    header[0..8].copy_from_slice(FILE_MAGIC);
    header[8..10].copy_from_slice(&CONTAINER_VERSION.to_le_bytes());
    header[10..12].copy_from_slice(&(HEADER_SIZE as u16).to_le_bytes());
    header[16..32].copy_from_slice(project_id.as_bytes());
    file.write_all(&header).map_err(|e| e.to_string())?;
    Ok(())
}

fn read_header(file: &mut File) -> Result<(Uuid, [Option<Checkpoint>; 2]), String> {
    let mut header = [0u8; HEADER_SIZE];
    file.seek(SeekFrom::Start(0))
        .and_then(|_| file.read_exact(&mut header))
        .map_err(|_| "This is not a complete MTR Pack Studio project file.".to_string())?;
    if &header[0..8] != FILE_MAGIC {
        return Err("This is not an MTR Pack Studio project file.".into());
    }
    if u16_at(&header, 8) != CONTAINER_VERSION {
        return Err("This project uses an unsupported container version.".into());
    }
    if u16_at(&header, 10) as usize != HEADER_SIZE {
        return Err("Invalid project header size.".into());
    }
    let project_id = Uuid::from_slice(&header[16..32]).map_err(|e| e.to_string())?;
    Ok((
        project_id,
        [
            parse_checkpoint(&header[32..128]),
            parse_checkpoint(&header[128..224]),
        ],
    ))
}

fn parse_checkpoint(bytes: &[u8]) -> Option<Checkpoint> {
    if bytes.iter().all(|byte| *byte == 0) {
        return None;
    }
    let expected = blake3::hash(&bytes[..64]);
    if expected.as_bytes() != &bytes[64..96] {
        return None;
    }
    Some(Checkpoint {
        generation: u64_at(bytes, 0),
        index_offset: u64_at(bytes, 8),
        stored_len: u64_at(bytes, 16),
        raw_len: u64_at(bytes, 24),
        index_hash: bytes[32..64].try_into().ok()?,
    })
}

fn write_checkpoint(file: &mut File, slot: usize, checkpoint: &Checkpoint) -> Result<(), String> {
    let mut bytes = [0u8; SLOT_SIZE];
    bytes[0..8].copy_from_slice(&checkpoint.generation.to_le_bytes());
    bytes[8..16].copy_from_slice(&checkpoint.index_offset.to_le_bytes());
    bytes[16..24].copy_from_slice(&checkpoint.stored_len.to_le_bytes());
    bytes[24..32].copy_from_slice(&checkpoint.raw_len.to_le_bytes());
    bytes[32..64].copy_from_slice(&checkpoint.index_hash);
    let checksum = blake3::hash(&bytes[..64]);
    bytes[64..96].copy_from_slice(checksum.as_bytes());
    file.seek(SeekFrom::Start(if slot == 0 {
        SLOT_A_OFFSET
    } else {
        SLOT_B_OFFSET
    }))
    .and_then(|_| file.write_all(&bytes))
    .map_err(|e| e.to_string())
}

fn write_record(
    file: &mut File,
    kind: u16,
    codec: u8,
    raw: &[u8],
    stored: &[u8],
) -> Result<u64, String> {
    let offset = file.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    let mut header = [0u8; RECORD_HEADER_SIZE];
    header[0..4].copy_from_slice(RECORD_MAGIC);
    header[4..6].copy_from_slice(&RECORD_VERSION.to_le_bytes());
    header[6..8].copy_from_slice(&kind.to_le_bytes());
    header[8] = codec;
    header[10..12].copy_from_slice(&(RECORD_HEADER_SIZE as u16).to_le_bytes());
    header[12..20].copy_from_slice(&(raw.len() as u64).to_le_bytes());
    header[20..28].copy_from_slice(&(stored.len() as u64).to_le_bytes());
    header[28..60].copy_from_slice(blake3::hash(raw).as_bytes());
    header[60..92].copy_from_slice(blake3::hash(stored).as_bytes());
    file.write_all(&header)
        .and_then(|_| file.write_all(stored))
        .map_err(|e| e.to_string())?;
    let padding = (8 - (stored.len() % 8)) % 8;
    if padding > 0 {
        file.write_all(&[0u8; 8][..padding])
            .map_err(|e| e.to_string())?;
    }
    Ok(offset)
}

fn read_record(file: &mut File, offset: u64) -> Result<(RecordHeader, Vec<u8>), String> {
    let file_len = file.metadata().map_err(|e| e.to_string())?.len();
    if offset < HEADER_SIZE as u64
        || offset
            .checked_add(RECORD_HEADER_SIZE as u64)
            .is_none_or(|end| end > file_len)
    {
        return Err("Project record offset is invalid.".into());
    }
    let mut bytes = [0u8; RECORD_HEADER_SIZE];
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.read_exact(&mut bytes))
        .map_err(|e| e.to_string())?;
    if &bytes[0..4] != RECORD_MAGIC
        || u16_at(&bytes, 4) != RECORD_VERSION
        || u16_at(&bytes, 10) as usize != RECORD_HEADER_SIZE
    {
        return Err("Project record header is invalid.".into());
    }
    let raw_len = u64_at(&bytes, 12);
    let stored_len = u64_at(&bytes, 20);
    if raw_len > MAX_OBJECT_SIZE
        || stored_len > MAX_OBJECT_SIZE
        || offset
            .checked_add(RECORD_HEADER_SIZE as u64)
            .and_then(|value| value.checked_add(stored_len))
            .is_none_or(|end| end > file_len)
    {
        return Err("Project record length is invalid.".into());
    }
    let header = RecordHeader {
        kind: u16_at(&bytes, 6),
        codec: bytes[8],
        raw_len,
        stored_len,
        raw_hash: bytes[28..60].try_into().unwrap(),
        stored_hash: bytes[60..92].try_into().unwrap(),
    };
    let mut stored = vec![0; stored_len as usize];
    file.read_exact(&mut stored).map_err(|e| e.to_string())?;
    if blake3::hash(&stored).as_bytes() != &header.stored_hash {
        return Err("Project record payload is damaged.".into());
    }
    let raw = match header.codec {
        CODEC_RAW => stored,
        CODEC_ZSTD => zstd::bulk::decompress(&stored, raw_len as usize)
            .map_err(|_| "Project record compression data is damaged.".to_string())?,
        _ => return Err("Project record uses an unsupported codec.".into()),
    };
    if raw.len() as u64 != raw_len || blake3::hash(&raw).as_bytes() != &header.raw_hash {
        return Err("Project record content hash does not match.".into());
    }
    Ok((header, raw))
}

fn read_index(file: &mut File, checkpoint: &Checkpoint) -> Result<ProjectIndex, String> {
    if checkpoint.raw_len > MAX_INDEX_SIZE || checkpoint.stored_len > MAX_INDEX_SIZE {
        return Err("Project index is too large.".into());
    }
    let (header, raw) = read_record(file, checkpoint.index_offset)?;
    if header.kind != KIND_INDEX
        || header.raw_len != checkpoint.raw_len
        || header.stored_len != checkpoint.stored_len
        || blake3::hash(&raw).as_bytes() != &checkpoint.index_hash
    {
        return Err("Project checkpoint does not match its index.".into());
    }
    let index: ProjectIndex =
        rmp_serde::from_slice(&raw).map_err(|e| format!("Invalid project index: {e}"))?;
    if index.generation != checkpoint.generation {
        return Err("Project index generation does not match its checkpoint.".into());
    }
    Ok(index)
}

fn encode_payload(raw: &[u8], compress: bool) -> Result<(u8, Vec<u8>), String> {
    if !compress || raw.is_empty() {
        return Ok((CODEC_RAW, raw.to_vec()));
    }
    let compressed = zstd::bulk::compress(raw, 3).map_err(|e| e.to_string())?;
    if compressed.len().saturating_mul(100) <= raw.len().saturating_mul(97) {
        Ok((CODEC_ZSTD, compressed))
    } else {
        Ok((CODEC_RAW, raw.to_vec()))
    }
}

fn should_compress(media_type: &str) -> bool {
    !matches!(
        media_type.to_ascii_lowercase().as_str(),
        "image/png"
            | "image/jpeg"
            | "image/webp"
            | "image/avif"
            | "image/ktx2"
            | "application/zip"
            | "application/zstd"
    )
}

fn aligned_record_size(stored_len: u64) -> u64 {
    RECORD_HEADER_SIZE as u64 + stored_len.div_ceil(8) * 8
}
fn estimated_index_record_size(index: &ProjectIndex) -> Result<u64, String> {
    Ok(zstd::bulk::compress(
        &rmp_serde::to_vec_named(index).map_err(|e| e.to_string())?,
        3,
    )
    .map_err(|e| e.to_string())?
    .len() as u64)
}
fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}
fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn temporary_path(path: &Path, purpose: &str) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project.mtrpack");
    path.with_file_name(format!(".{file_name}.{purpose}.{}.tmp", Uuid::new_v4()))
}

fn cleanup_temporary_files(path: &Path) {
    let Some(parent) = path.parent() else { return };
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return;
    };
    let prefix = format!(".{name}.");
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let candidate = entry.file_name();
            let candidate = candidate.to_string_lossy();
            if candidate.starts_with(&prefix) && candidate.ends_with(".tmp") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

#[cfg(windows)]
pub(crate) fn atomic_replace(source: &Path, destination: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let ok = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub(crate) fn atomic_replace(source: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(source, destination).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("mtrpack-{name}-{nonce}.mtrpack"))
    }

    #[test]
    fn creates_fixed_header_and_reopens_index() {
        let path = test_path("header");
        let container = Container::create(&path, "Urban Rail").unwrap();
        assert_eq!(container.index.name, "Urban Rail");
        assert_eq!(container.index.generation, 1);
        drop(container);
        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], FILE_MAGIC);
        assert_eq!(u16_at(&bytes, 8), 1);
        assert_eq!(u16_at(&bytes, 10), 256);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn deduplicates_equal_resources() {
        let path = test_path("dedupe");
        let mut container = Container::create(&path, "Test").unwrap();
        let first = container
            .put_blob(b"same model data", "model/gltf+json")
            .unwrap();
        let size = container.file.metadata().unwrap().len();
        let second = container
            .put_blob(b"same model data", "model/gltf+json")
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(container.file.metadata().unwrap().len(), size);
        assert_eq!(container.read_blob(&first).unwrap(), b"same model data");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn falls_back_when_newest_checkpoint_is_torn() {
        let path = test_path("recover");
        let mut container = Container::create(&path, "Test").unwrap();
        container.index.description = "second generation".into();
        container.commit().unwrap();
        let active_offset = if container.active_slot == 0 {
            SLOT_A_OFFSET
        } else {
            SLOT_B_OFFSET
        };
        container
            .file
            .seek(SeekFrom::Start(active_offset + 70))
            .unwrap();
        container.file.write_all(&[0xAA]).unwrap();
        container.file.sync_all().unwrap();
        drop(container);
        let recovered = Container::open(&path).unwrap();
        assert!(recovered.recovered);
        assert_eq!(recovered.index.generation, 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_unknown_container_version_and_oversized_record() {
        let path = test_path("invalid");
        let container = Container::create(&path, "Test").unwrap();
        drop(container);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.seek(SeekFrom::Start(8)).unwrap();
        file.write_all(&99u16.to_le_bytes()).unwrap();
        drop(file);
        assert!(Container::open(&path)
            .unwrap_err()
            .contains("unsupported container version"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn compaction_keeps_live_content() {
        let path = test_path("compact");
        let mut container = Container::create(&path, "Test").unwrap();
        let hash = container
            .put_blob(&vec![b'a'; 16_384], "application/json")
            .unwrap();
        container.commit().unwrap();
        for index in 0..8 {
            container.index.description = format!("revision {index}");
            container.commit().unwrap();
        }
        let before = container.file.metadata().unwrap().len();
        container.compact().unwrap();
        let after = container.file.metadata().unwrap().len();
        assert!(after < before);
        assert_eq!(container.read_blob(&hash).unwrap(), vec![b'a'; 16_384]);
        assert_eq!(container.index.description, "revision 7");
        fs::remove_file(path).unwrap();
    }
}
