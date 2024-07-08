use std::io::{Read, Seek, SeekFrom};
use symphonia::core::io::MediaSource;

/// Wrapper which impl `Read`, `Seek`, `Send`, `Sync` and `MediaSource`
/// for reader returned by `ureq` request.
pub struct UrlSource {
    url: String,
    reader: Box<dyn Read + Sync + Send>
}

impl UrlSource {
    pub fn new(url: &str) -> Self {
        let r = ureq::get(url).call();
        let r = r.unwrap().into_reader();
        UrlSource {
            reader: Box::new(r),
            url: url.to_string()
        }
    }
}

unsafe impl Send for UrlSource {}
unsafe impl Sync for UrlSource {}

impl MediaSource for UrlSource {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}


impl Read for UrlSource {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        self.reader.read(buf)
    }
}

impl Seek for UrlSource {
    fn seek(&mut self, pos: SeekFrom) -> std::result::Result<u64, std::io::Error> {
        match pos {
            SeekFrom::Start(p) => {
                let res = ureq::get(&self.url).set("Range", &format!("bytes={}-", p)).call();
                let r = res.unwrap().into_reader();
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
