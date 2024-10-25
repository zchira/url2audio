use std::{collections::BTreeMap, io::{Read, Seek, SeekFrom}};
use lazy_static::lazy_static;
use symphonia::core::io::MediaSource;

lazy_static! {
    static ref CHUNK_SIZE: u64 = 65536;
}
/// Wrapper which impl `Read`, `Seek`, `Send`, `Sync` and `MediaSource`
/// for reader returned by `ureq` request.
pub struct UrlSourceBuf {
    chunks: BTreeMap<usize, [u8; 65536]>,
    url: String,
    reader: Box<dyn Read + Sync + Send>,
    pos: usize
}

impl UrlSourceBuf {
    pub fn new(url: &str) -> Self {
        let r = ureq::get(url).call();
        let r = r.unwrap().into_reader();
        UrlSourceBuf {
            chunks: Default::default(),
            reader: Box::new(r),
            url: url.to_string(),
            pos: 0
        }
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
        let chunk_key = self.pos / 65536;
        let offset = self.pos - 65536 * chunk_key;
        let offset = offset as usize;

        match self.chunks.get(&(chunk_key as usize)) {
            Some(chunk) => {
                let bytes_to_read = if buf.len() > 65536 - offset { 65536 - offset } else { buf.len() };
                let s = &chunk[offset..offset + bytes_to_read]; //offset..offset+bytes_to_read];
                buf.copy_from_slice(s);
                self.pos = self.pos + bytes_to_read;
                Ok(bytes_to_read)
            },
            None => {
                let chunk_begin = chunk_key * 65536;
                let res = ureq::get(&self.url).set("Range", &format!("bytes={}-", chunk_begin)).call();
                let mut r = res.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?.into_reader();
                let mut b = [0u8;65536];
                r.read_exact(&mut b);
                self.chunks.insert(chunk_key, b);

                let bytes_to_read = if buf.len() > 65536 - offset { 65536 - offset } else { buf.len() };
                let s = &b[offset..offset + bytes_to_read]; //offset..offset+bytes_to_read];
                buf.copy_from_slice(s);
                self.pos = self.pos + bytes_to_read;
                Ok(bytes_to_read)
            }
        }
    }
}

impl Seek for UrlSourceBuf {
    fn seek(&mut self, pos: SeekFrom) -> std::result::Result<u64, std::io::Error> {
        match pos {
            SeekFrom::Start(p) => {
                let res = ureq::get(&self.url).set("Range", &format!("bytes={}-", p)).call();
                let mut r = res.unwrap().into_reader();
                let mut b = [0u8;100];
                r.read_exact(&mut b);
                self.reader = Box::new(r);
                Ok(u64::from_ne_bytes(p.to_ne_bytes()))
            },
            SeekFrom::End(p) => {
                let res = ureq::get(&self.url).set("Range", &format!("bytes=-{}", p)).call();
                let r = res.unwrap().into_reader();
                self.reader = Box::new(r);
                Ok(u64::from_ne_bytes(p.to_ne_bytes()))
            },
            SeekFrom::Current(p) => {
                let res = ureq::get(&self.url).set("Range", &format!("bytes={}-", p)).call();
                let r = res.unwrap().into_reader();
                self.reader = Box::new(r);
                Ok(u64::from_ne_bytes(p.to_ne_bytes()))
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
