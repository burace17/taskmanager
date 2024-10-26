use std::{cmp::min, ffi::c_void, mem::transmute};

use crate::get_task_manager_state;
use widestring::U16CString;
use windows::{
    core::{w, Result, PWSTR},
    Win32::{
        Foundation::{HMODULE, LPARAM, RECT, TRUE, WPARAM},
        UI::{
            Controls::{
                LIST_VIEW_ITEM_FLAGS, LVCFMT_LEFT, LVCFMT_RIGHT, LVCF_FMT, LVCF_SUBITEM, LVCF_TEXT,
                LVCF_WIDTH, LVCOLUMNW, LVCOLUMNW_FORMAT, LVIF_TEXT, LVM_INSERTCOLUMN,
                LVM_SETEXTENDEDLISTVIEWSTYLE, LVS_AUTOARRANGE, LVS_EX_FULLROWSELECT, LVS_OWNERDATA,
                LVS_REPORT, NMLISTVIEW, NMLVDISPINFOW, WC_LISTVIEWW,
            },
            WindowsAndMessaging::{
                CreateWindowExW, GetClientRect, MoveWindow, SendMessageW, HMENU, WS_BORDER,
                WS_CHILD, WS_EX_CLIENTEDGE, WS_TABSTOP, WS_VISIBLE,
            },
        },
    },
};

use crate::WindowHandle;

const INDEX_NAME: i32 = 0;
const INDEX_PID: i32 = 1;
const INDEX_CPU: i32 = 2;
const INDEX_MEMORY: i32 = 3;

pub unsafe fn create_control(instance: &HMODULE, parent: WindowHandle) -> Result<WindowHandle> {
    let style = WS_TABSTOP | WS_CHILD | WS_BORDER | WS_VISIBLE;
    let lv_style = LVS_AUTOARRANGE | LVS_REPORT | LVS_OWNERDATA;
    let window_style = style | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(lv_style);
    let hwnd = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        WC_LISTVIEWW,
        w!(""),
        window_style,
        0,
        0,
        0,
        0,
        parent.0,
        HMENU(crate::resources::ID_LISTVIEW as *mut c_void),
        *instance,
        None,
    )?;

    SendMessageW(
        hwnd,
        LVM_SETEXTENDEDLISTVIEWSTYLE,
        WPARAM(LVS_EX_FULLROWSELECT as usize),
        LPARAM(LVS_EX_FULLROWSELECT as isize),
    );

    let handle = WindowHandle::new(hwnd);
    resize_to_parent(handle, parent);

    add_column(handle, "Name", INDEX_NAME, 400, LVCFMT_LEFT);
    add_column(handle, "PID", INDEX_PID, 50, LVCFMT_LEFT);
    add_column(handle, "CPU", INDEX_CPU, 50, LVCFMT_RIGHT);
    add_column(handle, "Memory", INDEX_MEMORY, 90, LVCFMT_RIGHT);

    Ok(handle)
}

pub fn resize_to_parent(listview: WindowHandle, parent: WindowHandle) {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetClientRect(parent.0, &mut rect);
    };
    unsafe {
        let _ = MoveWindow(
            listview.0,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            TRUE,
        );
    };
}

pub unsafe fn on_get_display_info(hwnd: WindowHandle, lparam: LPARAM) {
    let lpdi = transmute::<LPARAM, *const NMLVDISPINFOW>(lparam);
    let lpdi = &(*lpdi);
    if (lpdi.item.mask & LVIF_TEXT) == LIST_VIEW_ITEM_FLAGS(0) {
        return;
    }

    let app_state = get_task_manager_state(hwnd);
    let processes = &app_state.borrow().processes;
    let process = &processes[lpdi.item.iItem as usize];

    match lpdi.item.iSubItem {
        INDEX_NAME => {
            copy_wstring_to_buffer(&process.image_name, lpdi.item.pszText, lpdi.item.cchTextMax);
        }
        INDEX_PID => {
            let pid_s = process.pid.to_string();
            copy_string_to_buffer(&pid_s, lpdi.item.pszText, lpdi.item.cchTextMax);
        }
        INDEX_CPU => {
            let cpu_s = process.cpu_usage.to_string();
            copy_string_to_buffer(&cpu_s, lpdi.item.pszText, lpdi.item.cchTextMax);
        }
        INDEX_MEMORY => {
            let mut ws_s = (process.private_working_set / 1024).to_string();
            ws_s.push_str(" K");
            copy_string_to_buffer(&ws_s, lpdi.item.pszText, lpdi.item.cchTextMax);
        }
        _ => unreachable!(),
    }
}

pub unsafe fn on_column_click(_hwnd: WindowHandle, lparam: LPARAM) {
    let lpdi = transmute::<LPARAM, *const NMLISTVIEW>(lparam);
    let lpdi = &(*lpdi);
    println!("column click: {}", lpdi.iSubItem);
}

fn add_column(listview: WindowHandle, title: &str, order: i32, width: i32, fmt: LVCOLUMNW_FORMAT) {
    let mut test = widestring::U16CString::from_str(title).unwrap();
    let header = PWSTR::from_raw(test.as_mut_ptr());
    let mut column = LVCOLUMNW {
        mask: LVCF_FMT | LVCF_WIDTH | LVCF_TEXT | LVCF_SUBITEM,
        fmt,
        cx: width,
        pszText: header,
        ..Default::default()
    };
    unsafe {
        SendMessageW(
            listview.0,
            LVM_INSERTCOLUMN,
            WPARAM(order as usize),
            LPARAM(&raw mut column as isize),
        )
    };
}

unsafe fn copy_string_to_buffer(s: &str, buffer: PWSTR, buffer_size: i32) {
    let wstr = U16CString::from_str(s).unwrap();
    copy_wstring_to_buffer(&wstr, buffer, buffer_size);
}

unsafe fn copy_wstring_to_buffer(wstr: &U16CString, buffer: PWSTR, buffer_size: i32) {
    let wstr_size_bytes = wstr.as_slice().len() + 1;
    let len = min(buffer_size as usize, wstr_size_bytes);
    std::ptr::copy_nonoverlapping(wstr.as_ptr(), buffer.as_ptr(), len);
}
