use std::{
    cmp::{min, Ordering},
    collections::HashMap,
    ffi::c_void,
    mem::transmute,
};

use crate::{
    process::{self, Process},
    resources::{to_pcwstr, IDM_TASK_CONTEXT_MENU},
    state::{self, SortKey, SortState},
};
use human_bytes::human_bytes;
use widestring::U16CString;
use windows::{
    core::{w, Result, PWSTR},
    Win32::{
        Foundation::{HMODULE, HWND, INVALID_HANDLE_VALUE, LPARAM, LRESULT, RECT, TRUE, WPARAM},
        Globalization::{
            CompareStringEx, COMPARE_STRING_FLAGS, CSTR_EQUAL, CSTR_GREATER_THAN, CSTR_LESS_THAN,
            LOCALE_NAME_SYSTEM_DEFAULT,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::{
                HDF_SORTDOWN, HDF_SORTUP, HDF_STRING, HDITEMW, HDI_FORMAT, HDM_GETITEM,
                HDM_SETITEM, HEADER_CONTROL_FORMAT_FLAGS, LIST_VIEW_ITEM_FLAGS, LVCFMT_LEFT,
                LVCFMT_RIGHT, LVCF_FMT, LVCF_SUBITEM, LVCF_TEXT, LVCF_WIDTH, LVCOLUMNW,
                LVCOLUMNW_FORMAT, LVIF_TEXT, LVM_GETHEADER, LVM_GETNEXTITEM, LVM_INSERTCOLUMN,
                LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETITEMCOUNT, LVNI_SELECTED,
                LVSICF_NOINVALIDATEALL, LVSICF_NOSCROLL, LVS_AUTOARRANGE, LVS_EX_DOUBLEBUFFER,
                LVS_EX_FULLROWSELECT, LVS_OWNERDATA, LVS_REPORT, NMLISTVIEW, NMLVDISPINFOW,
                WC_LISTVIEWW,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DestroyMenu, GetClientRect, GetSubMenu, GetWindowRect, LoadMenuW,
                MoveWindow, SendMessageW, TrackPopupMenu, HMENU, TPM_LEFTALIGN, TPM_RIGHTBUTTON,
                WM_SIZE, WS_BORDER, WS_CHILD, WS_EX_CLIENTEDGE, WS_TABSTOP, WS_VISIBLE,
            },
        },
    },
};

use crate::window::WindowHandle;

const INDEX_NAME: i32 = 0;
const INDEX_PID: i32 = 1;
const INDEX_CPU: i32 = 2;
const INDEX_MEMORY: i32 = 3;
const NUM_TASK_LIST_COLUMNS: usize = 4;

fn column_index_to_sort_key(sort_column_index: i32) -> SortKey {
    match sort_column_index {
        INDEX_NAME => SortKey::Name,
        INDEX_PID => SortKey::Pid,
        INDEX_CPU => SortKey::Cpu,
        INDEX_MEMORY => SortKey::Memory,
        _ => unreachable!(),
    }
}

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
        HMENU(crate::resources::ID_TASK_LIST as *mut c_void),
        *instance,
        None,
    )?;

    let extended_lv_style = LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER;
    SendMessageW(
        hwnd,
        LVM_SETEXTENDEDLISTVIEWSTYLE,
        WPARAM(extended_lv_style as usize),
        LPARAM(extended_lv_style as isize),
    );

    let handle = WindowHandle::new(hwnd);

    add_column(handle, "Name", INDEX_NAME, 400, LVCFMT_LEFT);
    add_column(handle, "PID", INDEX_PID, 50, LVCFMT_LEFT);
    add_column(handle, "CPU", INDEX_CPU, 50, LVCFMT_RIGHT);
    add_column(handle, "Memory", INDEX_MEMORY, 90, LVCFMT_RIGHT);

    Ok(handle)
}

fn lexical_str_cmp(a: &U16CString, b: &U16CString) -> Ordering {
    let result = unsafe {
        CompareStringEx(
            LOCALE_NAME_SYSTEM_DEFAULT,
            COMPARE_STRING_FLAGS(0),
            a.as_slice(),
            b.as_slice(),
            None,
            None,
            LPARAM(0),
        )
    };
    match result {
        CSTR_LESS_THAN => Ordering::Less,
        CSTR_EQUAL => Ordering::Equal,
        CSTR_GREATER_THAN => Ordering::Greater,
        _ => unreachable!(),
    }
}

fn sort_process_list(processes: &mut [Process], sort_key: SortKey) {
    match sort_key {
        SortKey::Name => processes.sort_by(|a, b| lexical_str_cmp(&a.image_name, &b.image_name)),
        SortKey::Pid => processes.sort_by_key(|k| k.pid),
        SortKey::Cpu => processes.sort_by_key(|k| k.cpu_usage),
        SortKey::Memory => processes.sort_by_key(|k| k.private_working_set),
    }
}

// invalidate_all = true does full refresh of list rather than just the items in view
pub fn refresh_process_list(main_window: WindowHandle, invalidate_all: bool) {
    let app_state = unsafe { state::get(main_window) };
    let mut app_state = app_state.borrow_mut();
    let mut new_processes = process::get_processes().unwrap();
    let mut new_pid_map = HashMap::new();
    for process in new_processes.iter_mut() {
        new_pid_map.insert(process.pid, process.clone());
        if let Some(old_process) = app_state.pid_map.get(&process.pid) {
            process.cpu_usage = process::get_cpu_usage(old_process, process, app_state.num_cpus);
        }
    }

    match app_state.sort_state {
        SortState::SortUp(sort_key) => sort_process_list(&mut new_processes, sort_key),
        SortState::SortDown(sort_key) => {
            sort_process_list(&mut new_processes, sort_key);
            new_processes.reverse();
        }
    }

    app_state.processes = new_processes;
    app_state.pid_map = new_pid_map;

    let flags = if invalidate_all {
        LVSICF_NOSCROLL
    } else {
        LVSICF_NOSCROLL | LVSICF_NOINVALIDATEALL
    };

    unsafe {
        SendMessageW(
            app_state.task_list.0,
            LVM_SETITEMCOUNT,
            WPARAM(app_state.processes.len()),
            LPARAM(flags as isize),
        );
    };
}

pub fn resize_to_parent(listview: WindowHandle, parent: WindowHandle, status_bar: WindowHandle) {
    unsafe {
        // Let status bar size itself first
        SendMessageW(status_bar.0, WM_SIZE, WPARAM(0), LPARAM(0));

        // Get client area of parent window
        let mut client_rect = RECT::default();
        let _ = GetClientRect(parent.0, &mut client_rect);

        // Get status bar rect to determine its height
        let mut status_rect = RECT::default();
        let _ = GetWindowRect(status_bar.0, &mut status_rect);
        let status_height = status_rect.bottom - status_rect.top;

        // Size listview to fill client area minus status bar height
        let _ = MoveWindow(
            listview.0,
            0,
            0,
            client_rect.right,
            client_rect.bottom - status_height,
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

    let app_state = state::get(hwnd);
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
            let ws_s = human_bytes(process.private_working_set as f64);
            copy_string_to_buffer(&ws_s, lpdi.item.pszText, lpdi.item.cchTextMax);
        }
        _ => unreachable!(),
    }
}

pub unsafe fn on_column_click(hwnd: WindowHandle, lparam: LPARAM) {
    let lpdi = transmute::<LPARAM, *const NMLISTVIEW>(lparam);
    let lpdi = &(*lpdi);
    toggle_sort_order(hwnd, lpdi.iSubItem);
    refresh_process_list(hwnd, true);
}

pub fn on_show_contextmenu(hwnd: WindowHandle, x: i32, y: i32) {
    unsafe {
        let app_state = state::get(hwnd);
        let selected_item = get_selected_task(app_state.borrow().task_list);
        if selected_item == -1 {
            return;
        }

        let instance = GetModuleHandleW(None).expect("shouldn't fail");
        let menu_load =
            LoadMenuW(instance, to_pcwstr(IDM_TASK_CONTEXT_MENU)).expect("shouldn't fail");
        let menu = GetSubMenu(menu_load, 0);
        // FIXME: should call GetSystemMetrics to find the correct context menu alignment
        let _ = TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_RIGHTBUTTON, x, y, 0, hwnd.0, None);
        DestroyMenu(menu_load).expect("shouldn't fail");
    }
}

pub fn on_end_task_clicked(hwnd: WindowHandle) -> LRESULT {
    unsafe {
        let app_state = state::get(hwnd);
        let app_state = app_state.borrow();
        let selected_item = get_selected_task(app_state.task_list);
        if selected_item >= 0 {
            let process = &app_state.processes[selected_item as usize];
            if let Err(e) = process::kill_process(process.pid) {
                println!("failed to kill process {}: {}", process.pid, e);
            }
        }

        LRESULT(0)
    }
}

fn add_column(task_list: WindowHandle, title: &str, order: i32, width: i32, fmt: LVCOLUMNW_FORMAT) {
    let mut title = widestring::U16CString::from_str(title).unwrap();
    let header = PWSTR::from_raw(title.as_mut_ptr());
    let mut column = LVCOLUMNW {
        mask: LVCF_FMT | LVCF_WIDTH | LVCF_TEXT | LVCF_SUBITEM,
        fmt,
        cx: width,
        pszText: header,
        ..Default::default()
    };
    unsafe {
        SendMessageW(
            task_list.0,
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

fn get_selected_task(list_hwnd: WindowHandle) -> isize {
    let result = unsafe {
        SendMessageW(
            list_hwnd.0,
            LVM_GETNEXTITEM,
            WPARAM(-1_isize as usize),
            LPARAM(LVNI_SELECTED as isize),
        )
    };
    result.0
}

const HEADER_SORT_UP_FORMAT: i32 = HDF_SORTUP.0 | HDF_STRING.0;
const HEADER_SORT_DOWN_FORMAT: i32 = HDF_SORTDOWN.0 | HDF_STRING.0;
const HEADER_NO_SORT_FORMAT: i32 = HDF_STRING.0;

unsafe fn toggle_sort_order(hwnd: WindowHandle, sort_column_index: i32) {
    let binding = state::get(hwnd);
    let mut app_state = binding.borrow_mut();
    let header = SendMessageW(app_state.task_list.0, LVM_GETHEADER, WPARAM(0), LPARAM(0));
    if header.0 == INVALID_HANDLE_VALUE.0 as isize {
        println!("LVM_GETHEADER failed");
        return;
    }

    let header = WindowHandle::new(HWND(header.0 as _));

    for column_index in 0..NUM_TASK_LIST_COLUMNS {
        let mut column = HDITEMW {
            mask: HDI_FORMAT,
            ..Default::default()
        };
        let result = SendMessageW(
            header.0,
            HDM_GETITEM,
            WPARAM(column_index),
            LPARAM(&raw mut column as isize),
        );
        if result.0 == 0 {
            println!("HDM_GETITEM failed for column: {}", column_index);
            continue;
        }

        if column_index == sort_column_index as usize {
            if column.fmt.0 & HDF_SORTUP.0 != 0 {
                column.fmt = HEADER_CONTROL_FORMAT_FLAGS(HEADER_SORT_DOWN_FORMAT);
                app_state.sort_state =
                    SortState::SortDown(column_index_to_sort_key(sort_column_index));
            } else {
                column.fmt = HEADER_CONTROL_FORMAT_FLAGS(HEADER_SORT_UP_FORMAT);
                app_state.sort_state =
                    SortState::SortUp(column_index_to_sort_key(sort_column_index));
            }
        } else {
            column.fmt = HEADER_CONTROL_FORMAT_FLAGS(HEADER_NO_SORT_FORMAT);
        }

        let result = SendMessageW(
            header.0,
            HDM_SETITEM,
            WPARAM(column_index),
            LPARAM(&raw mut column as isize),
        );
        if result.0 == 0 {
            println!("HDM_SETITEM failed for column: {}", column_index);
        }
    }
}
