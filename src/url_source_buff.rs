use std::{collections::BTreeMap, io::{self, Read, Seek, SeekFrom}};
use symphonia::core::io::MediaSource;
use crossbeam_channel::Sender;

use crate::player_engine::PlayerStatus;

const CHUNK_SIZE: usize = 65536;

/// Wrapper which impl `Read`, `Seek`, `Send`, `Sync` and `MediaSource`
/// for reader returned by `ureq` request.
pub struct UrlSourceBuf {
    chunks: BTreeMap<usize, [u8; CHUNK_SIZE]>,
    url: String,
    reader: Box<dyn Read + Sync + Send>,
    pos: usize,
    len: Option<u64>,
    tx: Option<crossbeam_channel::Sender<PlayerStatus>>
}

impl UrlSourceBuf {
    pub fn new(url: &str, tx: Option<Sender<PlayerStatus>>) -> Self {
        let r = ureq::get(url).call();
        let r = r.unwrap().into_reader();
        UrlSourceBuf {
            chunks: Default::default(),
            reader: Box::new(r),
            url: url.to_string(),
            pos: 0,
            tx,
            len: None
        }
    }

    fn get_chunk_key(&self, p: usize) -> usize {
        let key = p / CHUNK_SIZE;
        key
    }

    fn has_chunk(&self, key: usize) -> bool {
        self.chunks.get(&key).is_some()
    }

    fn insert_chunk(&mut self, chunk_key: usize, chunk: &[u8;CHUNK_SIZE]) {
        self.chunks.insert(chunk_key, *chunk);
        match self.tx.as_ref() {
            Some(tx) => {
                self.len = if self.len.is_none() {
                    self.byte_len()
                } else {
                    self.len
                };

                match self.len {
                    Some(l) => {
                        let start = chunk_key as f32 * CHUNK_SIZE as f32 / l as f32;
                        let end = start + CHUNK_SIZE as f32 / l as f32;

                        let _ = tx.try_send(PlayerStatus::ChunkAdded(start, end));
                    },
                    None => {},
                }
            },
            None => {},
        }
    }

    fn get_bytes_from_chunk(&mut self, chunk_key: usize, offset: usize, num_of_bytes: usize) -> Result<Vec<u8>, io::Error> {
        // let mut arr = vec![0; num_of_bytes];

        if !self.has_chunk(chunk_key) {
            let mut b = [0u8;CHUNK_SIZE];
            self.reader.read_exact(&mut b)?;
            self.insert_chunk(chunk_key, &b);
        }

        let chunk = self.chunks.get(&chunk_key).unwrap();
        let bytes_to_read = if num_of_bytes > CHUNK_SIZE - offset { CHUNK_SIZE - offset } else { num_of_bytes };
        let v1 = chunk[offset..offset + bytes_to_read].to_vec();

        let v2 = if bytes_to_read < num_of_bytes {
            let d = num_of_bytes - bytes_to_read;
            let additional = self.get_bytes_from_chunk(chunk_key + 1, 0, d)?;
            additional
        } else {
            Default::default()
        };

        let s = [v1, v2].concat();
        Ok(s)
    }
}

unsafe impl Send for UrlSourceBuf {}
unsafe impl Sync for UrlSourceBuf {}

impl MediaSource for UrlSourceBuf {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        if let Ok(r) = ureq::get(&self.url).call() {
            let cl = r.header("content-length");
            match cl {
                Some(len_str) => {
                    let len: Result<u64, _> = len_str.to_string().parse();
                    match len {
                        Ok(l) => Some(l),
                        Err(_) => None,
                    }
                },
                None => None,
            }
        } else {
            None
        }
    }
}


impl Read for UrlSourceBuf {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        let chunk_key = self.pos / CHUNK_SIZE;
        let offset = self.pos - CHUNK_SIZE * chunk_key;
        let offset = offset as usize;

        if !self.has_chunk(chunk_key) {
            let mut b = [0u8;CHUNK_SIZE];
            self.reader.read_exact(&mut b)?;
            self.insert_chunk(chunk_key, &b);
        }

        let bytes_to_read = if buf.len() > CHUNK_SIZE - offset { CHUNK_SIZE - offset } else { buf.len() };
        let v1;

        {
            let chunk = self.chunks.get(&chunk_key).unwrap();
            let s = &chunk[offset..offset + bytes_to_read]; //offset..offset+bytes_to_read];
            v1 = s.to_vec();
        }

        let v2: Vec<u8> = if bytes_to_read < buf.len() {
            let d = buf.len() - bytes_to_read;
            let additional_bytes = self.get_bytes_from_chunk(chunk_key + 1, 0, d)?;
            additional_bytes
        } else {
            Default::default()
        };

        let s = [v1, v2].concat();
        buf.copy_from_slice(&s);
        self.pos = self.pos + bytes_to_read;
        Ok(bytes_to_read)
    }
}

impl Seek for UrlSourceBuf {
    fn seek(&mut self, pos: SeekFrom) -> std::result::Result<u64, std::io::Error> {
        match pos {
            SeekFrom::Start(p) => {
                let chunk_key = self.get_chunk_key(p as usize);
                if self.has_chunk(chunk_key) {
                    self.pos = p as usize;
                    Ok(u64::from_ne_bytes(self.pos.to_ne_bytes()))
                } else {
                    let mut chunk = [0u8; CHUNK_SIZE];
                    let chunk_begin = chunk_key * CHUNK_SIZE;
                    let res = ureq::get(&self.url).set("Range", &format!("bytes={}-{}", chunk_begin, chunk_begin + CHUNK_SIZE)).call();
                    self.reader = Box::new(res.unwrap().into_reader());
                    self.reader.read_exact(&mut chunk)?;
                    self.insert_chunk(chunk_key, &chunk);
                    self.pos = p as usize;
                    Ok(u64::from_ne_bytes(self.pos.to_ne_bytes()))
                }
            },
            SeekFrom::End(p) => {
                // TODO: fix
                let new_pos = self.byte_len().unwrap() as i64 + p;
                let chunk_key = self.get_chunk_key(new_pos as usize);

                if self.has_chunk(chunk_key) {
                    self.pos = new_pos as usize;
                    Ok(u64::from_ne_bytes(self.pos.to_ne_bytes()))
                } else {
                    let mut chunk = [0u8; CHUNK_SIZE];
                    let chunk_begin = chunk_key * CHUNK_SIZE;
                    let res = ureq::get(&self.url).set("Range", &format!("bytes=-{}", chunk_begin)).call();
                    self.reader = Box::new(res.unwrap().into_reader());

                    self.reader.read_exact(&mut chunk)?;
                    self.insert_chunk(chunk_key, &chunk);
                    self.pos = new_pos as usize;
                    Ok(u64::from_ne_bytes(self.pos.to_ne_bytes()))
                }
            },
            SeekFrom::Current(p) => {
                let new_pos: i64 = self.pos as i64 + p;
                let chunk_key = self.get_chunk_key(new_pos as usize);
                if self.has_chunk(chunk_key) {
                    self.pos = p as usize;
                    Ok(u64::from_ne_bytes(self.pos.to_ne_bytes()))
                } else {
                    let mut chunk = [0u8; CHUNK_SIZE];
                    let chunk_begin = chunk_key * CHUNK_SIZE;
                    let res = ureq::get(&self.url).set("Range", &format!("bytes={}-{}", chunk_begin, chunk_begin + CHUNK_SIZE)).call();
                    self.reader = Box::new(res.unwrap().into_reader());
                    self.reader.read_exact(&mut chunk)?;
                    self.insert_chunk(chunk_key, &chunk);
                    self.pos = p as usize;
                    Ok(u64::from_ne_bytes(self.pos.to_ne_bytes()))
                }
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
