use windows::{
    core::Result,
    Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
};

pub fn get_memory_status() -> Result<MEMORYSTATUSEX> {
    let mut memory_status = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe { GlobalMemoryStatusEx(&mut memory_status)? }
    Ok(memory_status)
}
