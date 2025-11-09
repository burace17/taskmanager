use crate::window::WindowHandle;
use human_bytes::human_bytes;
use std::ffi::c_void;
use windows::{
    core::{w, Result},
    Win32::{
        Foundation::{HMODULE, LPARAM, RECT, WPARAM},
        UI::{
            Controls::{SBARS_SIZEGRIP, SB_SETPARTS, SB_SETTEXTW, STATUSCLASSNAMEW},
            WindowsAndMessaging::{
                CreateWindowExW, GetClientRect, SendMessageW, HMENU, WINDOW_EX_STYLE, WINDOW_STYLE,
                WS_CHILD, WS_VISIBLE,
            },
        },
    },
};

const STATUS_BAR_NUM_PARTS: usize = 2;
const STATUS_BAR_PART_PROCESS_COUNT: usize = 0;
const STATUS_BAR_PART_PROCESS_MEMORY: usize = 1;


pub fn create_control(instance: &HMODULE, parent: WindowHandle) -> Result<WindowHandle> {
    let window_style = WINDOW_STYLE(SBARS_SIZEGRIP) | WS_CHILD | WS_VISIBLE;
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            STATUSCLASSNAMEW,
            w!(""),
            window_style,
            0,
            0,
            0,
            0,
            parent.0,
            HMENU(crate::resources::ID_STATUS_BAR as *mut c_void),
            *instance,
            None,
        )?
    };

    let mut rect = RECT::default();
    let _ = unsafe { GetClientRect(parent.0, &mut rect) };

    let mut parts = [0; STATUS_BAR_NUM_PARTS as usize];
    let n_width = rect.right / (STATUS_BAR_NUM_PARTS as i32);
    let mut right_edge = n_width;
    for i in 0..STATUS_BAR_NUM_PARTS {
        parts[i] = right_edge;
        right_edge += n_width;
    }

    unsafe {
        SendMessageW(
            hwnd,
            SB_SETPARTS,
            WPARAM(STATUS_BAR_NUM_PARTS),
            LPARAM(&raw mut parts as isize),
        );
        Ok(WindowHandle::new(hwnd))
    }
}

fn build_memory_status_string() -> Result<String> {
    let mem_status = crate::system::get_memory_status()?;
    let mem_in_use = mem_status.ullTotalPhys - mem_status.ullAvailPhys;
    Ok(format!(
        "Memory Usage: {}/{} ({}%)",
        human_bytes(mem_in_use as f64),
        human_bytes(mem_status.ullTotalPhys as f64),
        mem_status.dwMemoryLoad
    ))
}

pub fn update(main_window: WindowHandle) {
    let app_state = unsafe { crate::state::get(main_window) };
    let app_state = app_state.borrow();
    let status_bar = app_state.status_bar;
    set_text(
        status_bar,
        STATUS_BAR_PART_PROCESS_COUNT,
        &format!("Processes: {}", app_state.processes.len()),
    );

    let mem_status_string = build_memory_status_string().unwrap_or_default();
    set_text(status_bar, STATUS_BAR_PART_PROCESS_MEMORY, &mem_status_string);
}

fn set_text(status_bar: WindowHandle, part: usize, text: &str) {
    let wide_text = widestring::U16CString::from_str(text).unwrap();
    unsafe {
        SendMessageW(
            status_bar.0,
            SB_SETTEXTW,
            WPARAM(part),
            LPARAM(wide_text.as_ptr() as isize),
        );
    }
}
