use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};
#[derive(Clone)]
pub struct RollingLog(Arc<Mutex<Writer>>);
struct Writer {
    path: PathBuf,
    file: File,
    size: u64,
    limit: u64,
}
impl RollingLog {
    pub fn new(path: PathBuf, limit: u64) -> io::Result<Self> {
        if limit == 0 {
            return Err(io::Error::other("log limit must be positive"));
        }
        if let Some(p) = path.parent() {
            fs::create_dir_all(p)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let size = file.metadata()?.len();
        Ok(Self(Arc::new(Mutex::new(Writer {
            path,
            file,
            size,
            limit,
        }))))
    }
}
impl Write for RollingLog {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut w = self
            .0
            .lock()
            .map_err(|_| io::Error::other("logger lock poisoned"))?;
        if w.size > 0 && w.size + buf.len() as u64 > w.limit {
            w.file.flush()?;
            let p1 = w.path.with_extension("1.txt");
            let p2 = w.path.with_extension("2.txt");
            if p2.exists() {
                fs::remove_file(&p2)?;
            }
            if p1.exists() {
                fs::rename(&p1, &p2)?;
            }
            fs::rename(&w.path, &p1)?;
            w.file = OpenOptions::new().create(true).append(true).open(&w.path)?;
            w.size = 0;
        }
        w.file.write_all(buf)?;
        w.size += buf.len() as u64;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("logger lock poisoned"))?
            .file
            .flush()
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RollingLog {
    type Writer = EventWriter;
    fn make_writer(&'a self) -> EventWriter {
        EventWriter {
            log: self.clone(),
            buffer: Vec::new(),
        }
    }
}
pub struct EventWriter {
    log: RollingLog,
    buffer: Vec<u8>,
}
impl Write for EventWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if !self.buffer.is_empty() {
            self.log.write_all(&self.buffer)?;
            self.buffer.clear();
        }
        self.log.flush()
    }
}
impl Drop for EventWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}
