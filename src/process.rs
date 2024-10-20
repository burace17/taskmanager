use std::ptr::addr_of_mut;

use widestring::U16CString;
use windows::{
    core::{Result, PWSTR},
    Win32::{
        Foundation::{CloseHandle, FALSE},
        System::ProcessStatus::EnumProcesses,
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
};

pub struct Process {
    pub pid: u32,
    pub image_name: U16CString,
}

pub fn get_processes() -> Result<Vec<Process>> {
    let mut output_process_list = Vec::new();

    let mut process_list: [u32; 1024] = [0; 1024];
    let cb = size_of_val(&process_list) as u32;
    let mut cb_needed: u32 = 0;

    unsafe {
        EnumProcesses(process_list.as_mut_ptr(), cb, addr_of_mut!(cb_needed))?;
    }

    if cb == cb_needed {
        println!("might need a bigger array...");
    }

    //let num_processes = cb_needed / (size_of::<u32>() as u32);

    for pid in process_list.iter().filter(|handle| **handle != 0) {
        let open_result = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, *pid) };
        match open_result {
            Ok(process) => {
                let mut process_name: [u16; 1024] = [0; 1024];
                let mut process_name_size: u32 = 1024;
                let result = unsafe {
                    QueryFullProcessImageNameW(
                        process,
                        PROCESS_NAME_FORMAT(0),
                        PWSTR(process_name.as_mut_ptr()),
                        addr_of_mut!(process_name_size),
                    )
                };
                match result {
                    Ok(()) => {
                        output_process_list.push(Process {
                            pid: *pid,
                            image_name: unsafe { U16CString::from_ptr_str(process_name.as_ptr()) },
                        });
                    }
                    Err(_) => {
                        //println!("error getting process name: {}", e);
                    }
                }
                unsafe { CloseHandle(process)? };
            }
            Err(_) => {
                //println!("Failed to open pid {}: {}", pid, e);
            }
        }
    }
    Ok(output_process_list)
}
