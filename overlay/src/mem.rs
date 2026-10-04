use std::ffi::c_void;
use std::mem;

use anyhow::{bail, Result};

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
    MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

pub struct GameProcess {
    handle: HANDLE,
    pid: u32,
}

impl GameProcess {
    pub fn open(name: &str) -> Result<Self> {
        let pid = find_pid(name)?;
        let handle = unsafe {
            OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, false, pid)?
        };
        Ok(Self { handle, pid })
    }

    pub fn read<T: Copy + Default>(&self, addr: u64) -> Option<T> {
        let mut val = T::default();
        let mut read = 0usize;
        unsafe {
            ReadProcessMemory(
                self.handle,
                addr as *const c_void,
                &mut val as *mut T as *mut c_void,
                mem::size_of::<T>(),
                Some(&mut read),
            )
            .ok()?;
        }
        Some(val)
    }

    pub fn read_bytes(&self, addr: u64, len: usize) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let mut read = 0usize;
        unsafe {
            ReadProcessMemory(
                self.handle,
                addr as *const c_void,
                buf.as_mut_ptr() as *mut c_void,
                len,
                Some(&mut read),
            )
            .ok()?;
        }
        Some(buf)
    }

    pub fn read_string(&self, addr: u64, max: usize) -> String {
        let mut buf = vec![0u8; max];
        let mut read = 0usize;
        unsafe {
            if ReadProcessMemory(
                self.handle,
                addr as *const c_void,
                buf.as_mut_ptr() as *mut c_void,
                max,
                Some(&mut read),
            )
            .is_err()
            {
                return String::new();
            }
        }
        let end = buf.iter().position(|&b| b == 0).unwrap_or(read);
        String::from_utf8_lossy(&buf[..end]).to_string()
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn module_base(&self, module_name: &str) -> Result<u64> {
        find_module(self.pid, module_name)
    }
}

impl Drop for GameProcess {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

fn find_pid(name: &str) -> Result<u32> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
        let mut entry = PROCESSENTRY32W {
            dwSize: mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let exe = wchar_to_string(&entry.szExeFile);
                if exe.eq_ignore_ascii_case(name) {
                    let _ = CloseHandle(snap);
                    return Ok(entry.th32ProcessID);
                }
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
        bail!("Process '{}' not found. Is CS2 running?", name)
    }
}

fn find_module(pid: u32, name: &str) -> Result<u64> {
    unsafe {
        let snap =
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)?;
        let mut entry = MODULEENTRY32W {
            dwSize: mem::size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        if Module32FirstW(snap, &mut entry).is_ok() {
            loop {
                let mod_name = wchar_to_string(&entry.szModule);
                if mod_name.eq_ignore_ascii_case(name) {
                    let _ = CloseHandle(snap);
                    return Ok(entry.modBaseAddr as u64);
                }
                if Module32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
        bail!("Module '{}' not found", name)
    }
}

fn wchar_to_string(s: &[u16]) -> String {
    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..end])
}
