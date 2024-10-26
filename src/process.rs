use std::{mem::transmute, time::Instant};

use widestring::U16CString;
use windows::{
    core::{Result, PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, FALSE, FILETIME, HANDLE},
        System::{
            ProcessStatus::{
                EnumProcesses, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
                PROCESS_MEMORY_COUNTERS_EX2,
            },
            Threading::{
                GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        UI::Shell::PathFindFileNameW,
    },
};

#[derive(Clone)]
pub struct Process {
    pub pid: u32,
    pub image_name: U16CString,
    pub private_working_set: usize,
    cpu_time: u64,
    sample_time: Instant,
    pub cpu_usage: u64,
}

unsafe fn get_process_image_name(process: HANDLE) -> Result<U16CString> {
    let mut process_name: [u16; 1024] = [0; 1024];
    let mut process_name_size: u32 = 1024;
    QueryFullProcessImageNameW(
        process,
        PROCESS_NAME_FORMAT(0),
        PWSTR(process_name.as_mut_ptr()),
        &mut process_name_size,
    )?;
    let file_name = PathFindFileNameW(PCWSTR(process_name.as_ptr()));
    Ok(U16CString::from_ptr_str(file_name.as_ptr()))
}

unsafe fn get_process_working_set_size(process: HANDLE) -> Result<usize> {
    let mut process_memory_counters = PROCESS_MEMORY_COUNTERS_EX2::default();
    GetProcessMemoryInfo(
        process,
        transmute::<*mut PROCESS_MEMORY_COUNTERS_EX2, *mut PROCESS_MEMORY_COUNTERS>(
            &mut process_memory_counters,
        ),
        size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32,
    )?;
    Ok(process_memory_counters.PrivateWorkingSetSize)
}

fn filetime_to_u64(time: &FILETIME) -> u64 {
    ((time.dwHighDateTime as u64) << 32) | (time.dwLowDateTime as u64)
}

unsafe fn get_process_cpu_time(process: HANDLE) -> Result<u64> {
    let mut creation_time = FILETIME::default();
    let mut exit_time = FILETIME::default();
    let mut kernel_time = FILETIME::default();
    let mut user_time = FILETIME::default();

    GetProcessTimes(
        process,
        &mut creation_time,
        &mut exit_time,
        &mut kernel_time,
        &mut user_time,
    )?;

    let kernel_time_64 = filetime_to_u64(&kernel_time);
    let user_time_64 = filetime_to_u64(&user_time);
    Ok(kernel_time_64 + user_time_64)
}

fn open_process(pid: &u32) -> Option<(u32, HANDLE)> {
    let result = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, *pid) };
    if let Ok(handle) = result {
        Some((*pid, handle))
    } else {
        None
    }
}

unsafe fn query_process_information(pid: u32, process: HANDLE) -> Result<Process> {
    let image_name = get_process_image_name(process)?;
    let working_set_size = get_process_working_set_size(process)?;
    let cpu_time = get_process_cpu_time(process)?;
    Ok(Process {
        pid,
        image_name,
        private_working_set: working_set_size,
        cpu_time,
        sample_time: Instant::now(),
        cpu_usage: 0,
    })
}

pub fn get_processes() -> Result<Vec<Process>> {
    let mut output_process_list = Vec::new();

    let mut process_list: [u32; 1024] = [0; 1024];
    let cb = size_of_val(&process_list) as u32;
    let mut cb_needed: u32 = 0;

    unsafe {
        EnumProcesses(process_list.as_mut_ptr(), cb, &mut cb_needed)?;
    }

    if cb == cb_needed {
        println!("might need a bigger array...");
    }

    for (pid, process_handle) in process_list.iter().filter_map(open_process) {
        if let Ok(process) = unsafe { query_process_information(pid, process_handle) } {
            output_process_list.push(process);
        }
        unsafe { CloseHandle(process_handle)? };
    }
    Ok(output_process_list)
}

pub fn get_cpu_usage(sample1: &Process, sample2: &Process, num_cpus: u32) -> u64 {
    let p1_time_ms = sample1.cpu_time / (1000 * 10);
    let p2_time_ms = sample2.cpu_time / (1000 * 10);
    let delta = (p2_time_ms - p1_time_ms) as f64;

    let time_elapsed = sample2.sample_time.duration_since(sample1.sample_time);
    let time_elapsed_ms = time_elapsed.as_millis() as f64;

    let res = (((delta / time_elapsed_ms) / (num_cpus as f64)) * 100.0).round();
    res as u64
}
