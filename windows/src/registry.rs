//! Small helpers for values under `HKEY_CURRENT_USER`.

use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR},
        System::Registry::{
            HKEY_CURRENT_USER, REG_DWORD, REG_ROUTINE_FLAGS, REG_SZ, RRF_RT_REG_BINARY,
            RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
        },
    },
    core::HSTRING,
};

pub fn get_string(subkey: &str, name: &str) -> Option<String> {
    let data = get(subkey, name, RRF_RT_REG_SZ)?;
    let wide: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .take_while(|&unit| unit != 0)
        .collect();
    Some(String::from_utf16_lossy(&wide))
}

pub fn get_dword(subkey: &str, name: &str) -> Option<u32> {
    let data = get(subkey, name, RRF_RT_REG_DWORD)?;
    Some(u32::from_le_bytes(data.get(..4)?.try_into().ok()?))
}

pub fn get_binary(subkey: &str, name: &str) -> Option<Vec<u8>> {
    get(subkey, name, RRF_RT_REG_BINARY)
}

fn get(subkey: &str, name: &str, flags: REG_ROUTINE_FLAGS) -> Option<Vec<u8>> {
    let (subkey, name) = (HSTRING::from(subkey), HSTRING::from(name));
    let mut size = 0u32;
    // SAFETY: the first call only asks for the size; the second writes at most
    // `size` bytes into a buffer of that length.
    unsafe { RegGetValueW(HKEY_CURRENT_USER, &subkey, &name, flags, None, None, Some(&raw mut size)) }
        .ok()
        .ok()?;
    let mut data = vec![0u8; size as usize];
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &subkey,
            &name,
            flags,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&raw mut size),
        )
    }
    .ok()
    .ok()?;
    data.truncate(size as usize);
    Some(data)
}

/// Creates the key if it doesn't exist yet.
pub fn set_string(subkey: &str, name: &str, value: &str) -> windows::core::Result<()> {
    let wide: Vec<u16> = value.encode_utf16().chain([0]).collect();
    let bytes: Vec<u8> = wide.iter().flat_map(|unit| unit.to_le_bytes()).collect();
    set(subkey, name, REG_SZ.0, &bytes)
}

pub fn set_dword(subkey: &str, name: &str, value: u32) -> windows::core::Result<()> {
    set(subkey, name, REG_DWORD.0, &value.to_le_bytes())
}

fn set(subkey: &str, name: &str, kind: u32, data: &[u8]) -> windows::core::Result<()> {
    #[allow(clippy::cast_possible_truncation)]
    let len = data.len() as u32;
    // SAFETY: `data` is valid for `len` bytes for the duration of the call.
    unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(subkey),
            &HSTRING::from(name),
            kind,
            Some(data.as_ptr().cast()),
            len,
        )
    }
    .ok()
}

/// Succeeds if the value is already gone.
pub fn delete(subkey: &str, name: &str) -> windows::core::Result<()> {
    // SAFETY: plain call with valid strings.
    let status: WIN32_ERROR =
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(subkey), &HSTRING::from(name)) };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    status.ok()
}
