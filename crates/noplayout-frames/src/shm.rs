//! Named shared memory for the paint ring (E-042 own-ring path): POSIX `shm_open` + `mmap` on
//! Linux/macOS, a pagefile-backed `CreateFileMappingW` section on Windows.
//!
//! Lifetime: on Windows the section dies with its last handle, so a `kill -9`'d writer cannot
//! leak it. POSIX objects outlive their processes until `shm_unlink`, so the engine records
//! every name it hands out in a registry file and unlinks them when an incarnation retires and
//! at engine start ([`Registry`]). memmap2 is not used: it cannot map a Windows pagefile
//! section, and on Unix it would only wrap the same `mmap` call.

use std::io;
use std::path::{Path, PathBuf};

/// A mapped shared-memory region. `ptr` is page-aligned and valid for `len` bytes.
#[derive(Debug)]
pub struct Shm {
    ptr: *mut u8,
    len: usize,
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

// SAFETY: the region is plain shared memory; synchronization is the ring's job (atomics).
unsafe impl Send for Shm {}
unsafe impl Sync for Shm {}

impl Shm {
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Platform object name for a ring name (`[A-Za-z0-9._-]`, ≤ 30 chars for macOS PSHMNAMLEN).
pub fn os_name(name: &str) -> io::Result<String> {
    if name.is_empty()
        || name.len() > 30
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("bad shared-memory name {name:?}"),
        ));
    }
    Ok(if cfg!(windows) {
        format!("Local\\{name}")
    } else {
        format!("/{name}")
    })
}

#[cfg(unix)]
mod imp {
    use super::*;
    use std::ffi::CString;

    fn cname(name: &str) -> io::Result<CString> {
        CString::new(os_name(name)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
    }

    fn map(fd: libc::c_int, len: usize) -> io::Result<Shm> {
        // SAFETY: fd is a valid shm descriptor sized to `len`; MAP_SHARED of the whole object.
        let p = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        let err = io::Error::last_os_error();
        // SAFETY: the mapping keeps the object alive; the descriptor is no longer needed.
        unsafe { libc::close(fd) };
        if p == libc::MAP_FAILED {
            return Err(err);
        }
        Ok(Shm { ptr: p.cast(), len })
    }

    pub fn create(name: &str, len: usize) -> io::Result<Shm> {
        let n = cname(name)?;
        // SAFETY: valid C string; O_EXCL so two writers never share an object by accident.
        let fd = unsafe {
            libc::shm_open(
                n.as_ptr(),
                libc::O_CREAT | libc::O_EXCL | libc::O_RDWR,
                0o600 as libc::c_uint,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fd valid.
        if unsafe { libc::ftruncate(fd, len as libc::off_t) } != 0 {
            let e = io::Error::last_os_error();
            unsafe {
                libc::close(fd);
                libc::shm_unlink(n.as_ptr());
            }
            return Err(e);
        }
        map(fd, len)
    }

    pub fn open(name: &str) -> io::Result<Shm> {
        let n = cname(name)?;
        // SAFETY: valid C string.
        let fd = unsafe { libc::shm_open(n.as_ptr(), libc::O_RDWR, 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: zeroed stat is a valid out-parameter.
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd, &mut st) } != 0 {
            let e = io::Error::last_os_error();
            unsafe { libc::close(fd) };
            return Err(e);
        }
        map(fd, st.st_size as usize)
    }

    pub fn unlink(name: &str) -> io::Result<()> {
        let n = cname(name)?;
        // SAFETY: valid C string.
        if unsafe { libc::shm_unlink(n.as_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    impl Drop for Shm {
        fn drop(&mut self) {
            // SAFETY: ptr/len came from a successful mmap.
            unsafe { libc::munmap(self.ptr.cast(), self.len) };
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_ALL_ACCESS, MEMORY_BASIC_INFORMATION, MapViewOfFile,
        OpenFileMappingW, PAGE_READWRITE, UnmapViewOfFile, VirtualQuery,
    };

    fn wide(name: &str) -> io::Result<Vec<u16>> {
        Ok(os_name(name)?.encode_utf16().chain([0]).collect())
    }

    fn view(handle: windows_sys::Win32::Foundation::HANDLE, len: usize) -> io::Result<Shm> {
        // SAFETY: valid section handle; map the whole section.
        let v = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, len) };
        if v.Value.is_null() {
            let e = io::Error::last_os_error();
            unsafe { CloseHandle(handle) };
            return Err(e);
        }
        let mut len = len;
        if len == 0 {
            // SAFETY: zeroed info is a valid out-parameter.
            let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
            unsafe { VirtualQuery(v.Value, &mut info, std::mem::size_of_val(&info)) };
            len = info.RegionSize;
        }
        Ok(Shm {
            ptr: v.Value.cast(),
            len,
            handle,
        })
    }

    pub fn create(name: &str, len: usize) -> io::Result<Shm> {
        let n = wide(name)?;
        let size = len as u64;
        // SAFETY: pagefile-backed section (INVALID_HANDLE_VALUE), default security.
        let h = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                std::ptr::null(),
                PAGE_READWRITE,
                (size >> 32) as u32,
                size as u32,
                n.as_ptr(),
            )
        };
        if h.is_null() {
            return Err(io::Error::last_os_error());
        }
        // ERROR_ALREADY_EXISTS = 183: never share an object by accident.
        if unsafe { GetLastError() } == 183 {
            unsafe { CloseHandle(h) };
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        view(h, len)
    }

    pub fn open(name: &str) -> io::Result<Shm> {
        let n = wide(name)?;
        // SAFETY: valid wide string.
        let h = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, n.as_ptr()) };
        if h.is_null() {
            return Err(io::Error::last_os_error());
        }
        view(h, 0)
    }

    /// Sections vanish with their last handle; nothing to unlink.
    pub fn unlink(_name: &str) -> io::Result<()> {
        Ok(())
    }

    impl Drop for Shm {
        fn drop(&mut self) {
            use windows_sys::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS;
            // SAFETY: ptr came from MapViewOfFile, handle from Create/OpenFileMappingW.
            unsafe {
                UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.ptr.cast(),
                });
                CloseHandle(self.handle);
            }
        }
    }
}

pub use imp::{create, open, unlink};

/// Names of every ring the engine created, so a crashed engine's objects are unlinked at the
/// next start (POSIX objects outlive their processes). One name per line.
#[derive(Debug, Clone)]
pub struct Registry {
    path: PathBuf,
}

impl Registry {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Record `name` before anyone creates it.
    pub fn record(&self, name: &str) -> io::Result<()> {
        use std::io::Write;
        if let Some(d) = self.path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(f, "{name}")?;
        f.sync_data()
    }

    /// Unlink every recorded name and clear the registry. Returns how many objects existed.
    pub fn sweep(&self) -> io::Result<usize> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        let mut n = 0;
        for name in text.lines().filter(|l| !l.is_empty()) {
            if unlink(name).is_ok() && cfg!(unix) {
                n += 1;
            }
        }
        std::fs::write(&self.path, "")?;
        Ok(n)
    }
}
