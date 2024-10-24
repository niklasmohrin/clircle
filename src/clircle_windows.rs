use crate::{Clircle, Stdio};

use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        GetFileInformationByHandle, GetFileType, BY_HANDLE_FILE_INFORMATION, FILE_TYPE_DISK,
    },
    System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
};

use std::convert::TryFrom;
use std::fs::File;
use std::mem::MaybeUninit;
use std::os::windows::io::{FromRawHandle, IntoRawHandle};
use std::{cmp, hash, io, mem, ops};

/// Implementation of `Clircle` for Windows.
#[derive(Debug)]
pub(crate) struct Identifier {
    volume_serial: u32,
    file_index: u64,
    handle: HANDLE,
    owns_handle: bool,
}

impl Identifier {
    unsafe fn try_from_raw_handle(handle: HANDLE, owns_handle: bool) -> Result<Self, io::Error> {
        if handle.is_invalid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Tried to convert handle to Identifier that was invalid or null.",
            ));
        }
        // SAFETY: This function can be called with any valid handle.
        // https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfiletype
        if GetFileType(handle) != FILE_TYPE_DISK {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Tried to convert handle to Identifier that was not a file handle.",
            ));
        }
        let mut fi = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        // SAFETY: This function is safe to call, if the handle is valid and a handle to a file.
        // https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
        GetFileInformationByHandle(handle, fi.as_mut_ptr())?;

        // SAFETY: GetFileInformationByHandle returned successfully.
        let fi = fi.assume_init();

        Ok(Self {
            volume_serial: fi.dwVolumeSerialNumber,
            file_index: u64::from(fi.nFileIndexHigh) << 32 | u64::from(fi.nFileIndexLow),
            handle,
            owns_handle,
        })
    }

    unsafe fn take_handle(&mut self) -> Option<HANDLE> {
        if self.owns_handle {
            self.owns_handle = false;
            Some(mem::replace(&mut self.handle, INVALID_HANDLE_VALUE))
        } else {
            None
        }
    }
}

impl Clircle for Identifier {
    #[must_use]
    fn into_inner(mut self) -> Option<File> {
        Some(unsafe { File::from_raw_handle(self.take_handle()?.0 as _) })
    }
}

impl TryFrom<Stdio> for Identifier {
    type Error = io::Error;

    fn try_from(stdio: Stdio) -> Result<Self, Self::Error> {
        let std_handle_id = match stdio {
            Stdio::Stdin => STD_INPUT_HANDLE,
            Stdio::Stdout => STD_OUTPUT_HANDLE,
            Stdio::Stderr => STD_ERROR_HANDLE,
        };

        // SAFETY: This method can safely be called with one of the above constants.
        // https://docs.microsoft.com/en-us/windows/console/getstdhandle
        let handle = unsafe { GetStdHandle(std_handle_id) }?;
        if handle.is_invalid() {
            return Err(io::Error::last_os_error());
        }

        unsafe { Self::try_from_raw_handle(handle, false) }
    }
}
impl TryFrom<File> for Identifier {
    type Error = io::Error;

    fn try_from(file: File) -> Result<Self, Self::Error> {
        unsafe { Self::try_from_raw_handle(HANDLE(file.into_raw_handle() as _), true) }
    }
}

impl ops::Drop for Identifier {
    fn drop(&mut self) {
        unsafe {
            if let Some(handle) = self.take_handle() {
                let _ = CloseHandle(handle);
            }
        }
    }
}

impl cmp::PartialEq for Identifier {
    #[must_use]
    fn eq(&self, other: &Self) -> bool {
        self.volume_serial == other.volume_serial && self.file_index == other.file_index
    }
}
impl Eq for Identifier {}

impl hash::Hash for Identifier {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.volume_serial.hash(state);
        self.file_index.hash(state);
    }
}
