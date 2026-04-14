use std::{collections::BTreeMap, io::{self, Read, Seek, SeekFrom}};
use symphonia::core::io::MediaSource;
use crossbeam_channel::Sender;

use crate::player_engine::PlayerStatus;
use crate::Url2AudioError;

const CHUNK_SIZE: usize = 65536;
const MAX_CHUNK_DISTANCE: usize = 32;

/// Wrapper which impl `Read`, `Seek`, `Send`, `Sync` and `MediaSource`
/// for reader returned by `ureq` request.
pub struct UrlSourceBuf {
    chunks: BTreeMap<usize, Vec<u8>>,
    url: String,
    reader: Box<dyn Read + Sync + Send>,
    pos: usize,
    len: Option<u64>,
    tx: Option<crossbeam_channel::Sender<PlayerStatus>>
}

impl UrlSourceBuf {
    pub fn new(url: &str, tx: Option<Sender<PlayerStatus>>) -> Result<Self, Url2AudioError> {
        let r = ureq::get(url).call()?;
        let len = r.header("content-length")
            .and_then(|s| s.parse::<u64>().ok());
        let reader = r.into_reader();
        Ok(UrlSourceBuf {
            chunks: Default::default(),
            reader: Box::new(reader),
            url: url.to_string(),
            pos: 0,
            tx,
            len,
        })
    }

    fn get_chunk_key(&self, p: usize) -> usize {
        p / CHUNK_SIZE
    }

    fn evict_distant_chunks(&mut self) {
        let current_key = self.get_chunk_key(self.pos);
        let min = current_key.saturating_sub(MAX_CHUNK_DISTANCE);
        let max = current_key + MAX_CHUNK_DISTANCE;
        self.chunks.retain(|&k, _| k >= min && k <= max);
    }

    fn has_chunk(&self, key: usize) -> bool {
        self.chunks.contains_key(&key)
    }

    fn insert_chunk(&mut self, chunk_key: usize, chunk: Vec<u8>) {
        if let Some(tx) = self.tx.as_ref() {
            self.len = self.len.or_else(|| self.byte_len());
            if let Some(l) = self.len {
                let start = chunk_key as f32 * CHUNK_SIZE as f32 / l as f32;
                let end = start + CHUNK_SIZE as f32 / l as f32;
                let _ = tx.try_send(PlayerStatus::ChunkAdded(start, end));
            }
        }
        self.chunks.insert(chunk_key, chunk);
    }

    /// Reopen the HTTP reader from the chunk boundary of the current `pos`,
    /// using an open-ended Range so subsequent sequential reads keep working.
    /// Pre-fetches and caches the first chunk if not already present.
    fn reposition_reader(&mut self) -> io::Result<()> {
        let chunk_key = self.get_chunk_key(self.pos);
        let chunk_begin = chunk_key * CHUNK_SIZE;
        let res = ureq::get(&self.url)
            .set("Range", &format!("bytes={}-", chunk_begin))
            .call()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        self.reader = Box::new(res.into_reader());
        if !self.has_chunk(chunk_key) {
            let chunk = Self::read_chunk_from_reader(&mut self.reader)?;
            self.insert_chunk(chunk_key, chunk);
        }
        Ok(())
    }

    /// Read up to CHUNK_SIZE bytes from `reader` into a heap-allocated Vec.
    /// Tolerates short reads (e.g. at EOF) — returned Vec is always CHUNK_SIZE,
    /// zero-padded if fewer bytes are available.
    fn read_chunk_from_reader(reader: &mut Box<dyn Read + Sync + Send>) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut bytes_read = 0;
        while bytes_read < CHUNK_SIZE {
            match reader.read(&mut buf[bytes_read..]) {
                Ok(0) => break, // EOF — zero-pad the rest
                Ok(n) => bytes_read += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(buf)
    }
}

unsafe impl Send for UrlSourceBuf {}
unsafe impl Sync for UrlSourceBuf {}

impl MediaSource for UrlSourceBuf {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.len
    }
}


impl Read for UrlSourceBuf {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let chunk_key = self.pos / CHUNK_SIZE;
        let offset = self.pos % CHUNK_SIZE;

        if !self.has_chunk(chunk_key) {
            let chunk = Self::read_chunk_from_reader(&mut self.reader)?;
            self.insert_chunk(chunk_key, chunk);
        }

        let chunk = self.chunks.get(&chunk_key).unwrap();
        let bytes_to_read = buf.len().min(CHUNK_SIZE - offset);
        buf[..bytes_to_read].copy_from_slice(&chunk[offset..offset + bytes_to_read]);
        self.pos += bytes_to_read;
        self.evict_distant_chunks();
        Ok(bytes_to_read)
    }
}

impl Seek for UrlSourceBuf {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(p) => {
                self.pos = p as usize;
                self.reposition_reader()?;
                Ok(p)
            },
            SeekFrom::End(p) => {
                let total_len = self.byte_len()
                    .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no content-length available"))?;
                let new_pos = (total_len as i64 + p).max(0) as usize;
                self.pos = new_pos;
                self.reposition_reader()?;
                Ok(new_pos as u64)
            },
            SeekFrom::Current(p) => {
                let new_pos = (self.pos as i64 + p).max(0) as usize;
                self.pos = new_pos;
                self.reposition_reader()?;
                Ok(new_pos as u64)
            },
        }
    }
}


#[test]
fn ureq_range() {
    let url = "https://podcast.daskoimladja.com/media/2024-05-27-PONEDELJAK_27.05.2024.mp3";
    let r = ureq::get(url).set("Range", "bytes=0-20").call();
    let mut r = r.unwrap().into_reader();
    let mut buf: [u8; 20] = [0; 20];
    let r = r.read_exact(&mut buf);
    println!("r1: {:#?},,, {:#?}", r, buf);
}

#[test]
fn ureq_range2() {
    let url = "https://podcast.daskoimladja.com/media/2024-05-27-PONEDELJAK_27.05.2024.mp3";
    let r = ureq::get(url).set("Range", "bytes=10-").call();
    let mut r = r.unwrap().into_reader();
    let mut buf: [u8; 10] = [0; 10];
    let r = r.read_exact(&mut buf);
    println!("r2: {:#?},,, {:#?}", r, buf);
}

#[test]
fn ureq_content_length() {
    let url = "https://podcast.daskoimladja.com/media/2024-05-27-PONEDELJAK_27.05.2024.mp3";
    let r = ureq::get(url).call().unwrap();
    let headers = r.headers_names();
    let cl = r.header("content-length");
    println!("r3: {:#?},,, {:#?}", headers, cl);
}
