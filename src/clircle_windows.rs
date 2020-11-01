use crate::Stdio;
use std::convert::TryFrom;
use std::io;
use std::iter;
use std::mem::MaybeUninit;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::null_mut;
use winapi::shared::ntdef::NULL;
use winapi::um::fileapi::{
    CreateFileW, GetFileInformationByHandle, GetFileType, BY_HANDLE_FILE_INFORMATION, OPEN_EXISTING,
};
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::processenv::GetStdHandle;
use winapi::um::winbase::{FILE_TYPE_DISK, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
use winapi::um::winnt::{FILE_ATTRIBUTE_NORMAL, FILE_READ_ATTRIBUTES, FILE_SHARE_READ, HANDLE};

/// Re-export of winapi
pub use winapi;

/// Implementation of `Clircle` for Windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowsIdentifier {
    volume_serial: u32,
    file_index: u64,
}

impl TryFrom<HANDLE> for WindowsIdentifier {
    type Error = io::Error;

    fn try_from(handle: HANDLE) -> Result<Self, Self::Error> {
        if handle == INVALID_HANDLE_VALUE || handle == NULL {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Tried to convert handle to WindowsIdentifier that was invalid or null.",
            ));
        }
        // SAFETY: This function can be called with any valid handle.
        // https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfiletype
        if unsafe { GetFileType(handle) } != FILE_TYPE_DISK {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Tried to convert handle to WindowsIdentifier that was not a file handle.",
            ));
        }
        let mut fi = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        // SAFETY: This function is safe to call, if the handle is valid and a handle to a file.
        // https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
        let success = unsafe { GetFileInformationByHandle(handle, fi.as_mut_ptr()) };
        if success == 0 {
            Err(io::Error::last_os_error())
        } else {
            // SAFETY: If the return value of GetFileInformationByHandle is non-zero, the struct
            // has successfully been initialized (see link above).
            let fi = unsafe { fi.assume_init() };

            Ok(WindowsIdentifier {
                volume_serial: fi.dwVolumeSerialNumber,
                file_index: u64::from(fi.nFileIndexHigh) << 32 | u64::from(fi.nFileIndexLow),
            })
        }
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

        Self::try_from(handle)
    }
}

impl TryFrom<&'_ Path> for WindowsIdentifier {
    type Error = io::Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        // Convert to C-style UTF-16
        let path: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(iter::once(0))
            .collect();

        // SAFETY: Arguments are specified according to documentation and failure is caught below.
        // https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                FILE_READ_ATTRIBUTES,
                // Other processes can still read the file, but cannot write to it
                FILE_SHARE_READ,
                // No extra security attributes needed
                null_mut(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                // No meaning in this mode
                null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE || handle == NULL {
            return Err(io::Error::last_os_error());
        }

        let ret = WindowsIdentifier::try_from(handle);
        // SAFETY: The handle is valid by the above comparison.
        // https://docs.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-closehandle
        unsafe { CloseHandle(handle) };
        ret
    }
}
