use windows::{
    core::Result,
    Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
};

pub fn get_memory_status() -> Result<MEMORYSTATUSEX> {
    let mut memory_status = MEMORYSTATUSEX::default();
    memory_status.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    unsafe { GlobalMemoryStatusEx(&mut memory_status)? }
    Ok(memory_status)
}
