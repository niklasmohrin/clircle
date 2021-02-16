use crate::{Clircle, Stdio};

use winapi::shared::ntdef::NULL;
use winapi::um::{
    fileapi::{GetFileInformationByHandle, GetFileType, BY_HANDLE_FILE_INFORMATION},
    handleapi::INVALID_HANDLE_VALUE,
    processenv::GetStdHandle,
    winbase::{FILE_TYPE_DISK, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
};

use std::convert::TryFrom;
use std::fs::File;
use std::mem::MaybeUninit;
use std::os::windows::io::{FromRawHandle, IntoRawHandle, RawHandle};
use std::{cmp, hash, io, mem, ops};

/// Re-export of winapi
pub use winapi;

/// Implementation of `Clircle` for Windows.
#[derive(Debug)]
pub struct WindowsIdentifier {
    volume_serial: u32,
    file_index: u64,
    handle: RawHandle,
    owns_handle: bool,
}

impl WindowsIdentifier {
    unsafe fn try_from_raw_handle(handle: RawHandle, owns_handle: bool) -> Result<Self, io::Error> {
        if handle == INVALID_HANDLE_VALUE || handle == NULL {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Tried to convert handle to WindowsIdentifier that was invalid or null.",
            ));
        }
        // SAFETY: This function can be called with any valid handle.
        // https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfiletype
        if GetFileType(handle) != FILE_TYPE_DISK {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Tried to convert handle to WindowsIdentifier that was not a file handle.",
            ));
        }
        let mut fi = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        // SAFETY: This function is safe to call, if the handle is valid and a handle to a file.
        // https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
        let success = GetFileInformationByHandle(handle, fi.as_mut_ptr());
        if success == 0 {
            Err(io::Error::last_os_error())
        } else {
            // SAFETY: If the return value of GetFileInformationByHandle is non-zero, the struct
            // has successfully been initialized (see link above).
            let fi = fi.assume_init();

            Ok(Self {
                volume_serial: fi.dwVolumeSerialNumber,
                file_index: u64::from(fi.nFileIndexHigh) << 32 | u64::from(fi.nFileIndexLow),
                handle,
                owns_handle,
            })
        }
    }

    unsafe fn take_handle(&mut self) -> Option<RawHandle> {
        if self.owns_handle {
            self.owns_handle = false;
            Some(mem::replace(&mut self.handle, INVALID_HANDLE_VALUE))
        } else {
            None
        }
    }
}

impl Clircle for WindowsIdentifier {
    #[must_use]
    fn into_inner(mut self) -> Option<File> {
        Some(unsafe { File::from_raw_handle(self.take_handle()?) })
    }
}

impl TryFrom<Stdio> for WindowsIdentifier {
    type Error = io::Error;

    fn try_from(stdio: Stdio) -> Result<Self, Self::Error> {
        let std_handle_id = match stdio {
            Stdio::Stdin => STD_INPUT_HANDLE,
            Stdio::Stdout => STD_OUTPUT_HANDLE,
            Stdio::Stderr => STD_ERROR_HANDLE,
        };

        // SAFETY: This method can safely be called with one of the above constants.
        // https://docs.microsoft.com/en-us/windows/console/getstdhandle
        let handle = unsafe { GetStdHandle(std_handle_id) };
        if handle == INVALID_HANDLE_VALUE || handle == NULL {
            return Err(io::Error::last_os_error());
        }

        unsafe { Self::try_from_raw_handle(handle, false) }
    }
}
impl TryFrom<File> for WindowsIdentifier {
    type Error = io::Error;

    fn try_from(file: File) -> Result<Self, Self::Error> {
        unsafe { Self::try_from_raw_handle(file.into_raw_handle(), true) }
    }
}

impl ops::Drop for WindowsIdentifier {
    fn drop(&mut self) {
        unsafe {
            if let Some(handle) = self.take_handle() {
                drop(File::from_raw_handle(handle));
            }
        }
    }
}

impl cmp::PartialEq for WindowsIdentifier {
    #[must_use]
    fn eq(&self, other: &Self) -> bool {
        self.volume_serial == other.volume_serial && self.file_index == other.file_index
    }
}
impl Eq for WindowsIdentifier {}

impl hash::Hash for WindowsIdentifier {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.volume_serial.hash(state);
        self.file_index.hash(state);
    }
}
