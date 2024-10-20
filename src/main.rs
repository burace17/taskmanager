use std::{ffi::c_void, ptr::addr_of_mut};

use widestring::U16CString;
use windows::{
    core::{w, Result, PCWSTR, PWSTR},
    Win32::{
        Foundation::{HMODULE, HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM},
        Graphics::Gdi::UpdateWindow,
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::{
                LVCFMT_LEFT, LVCF_FMT, LVCF_SUBITEM, LVCF_TEXT, LVCF_WIDTH, LVCOLUMNW, LVIF_TEXT, LVITEMW, LVM_DELETEALLITEMS, LVM_INSERTCOLUMN, LVM_INSERTITEM, LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETITEM, LVS_AUTOARRANGE, LVS_EX_FULLROWSELECT, LVS_REPORT, WC_LISTVIEWW
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect,
                GetMessageW, LoadAcceleratorsW, LoadCursorW, MoveWindow, PostQuitMessage,
                RegisterClassExW, SendMessageW, ShowWindow, TranslateAcceleratorW,
                TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, HMENU, IDC_ARROW, MSG,
                SW_SHOW, WINDOW_EX_STYLE, WM_COMMAND, WM_CREATE, WM_DESTROY, WNDCLASSEXW,
                WS_BORDER, WS_CHILD, WS_EX_CLIENTEDGE, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE,
            },
        },
    },
};

use crate::resources::{to_pcwstr, IDC_TASKMANAGER};

mod process;
mod resources;
// Container for a valid window handle
// Initialize with new()
struct WindowHandle(HWND);

impl WindowHandle {
    pub unsafe fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }
}

const ID_LISTVIEW: i32 = 2000;

unsafe fn register_class(instance: &HMODULE, name: &PCWSTR) -> Result<()> {
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpszClassName: *name,
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        lpfnWndProc: Some(wndproc),
        hInstance: (*instance).into(),
        lpszMenuName: to_pcwstr(IDC_TASKMANAGER),
        ..Default::default()
    };

    let atom = RegisterClassExW(&wc);
    debug_assert!(atom != 0);
    Ok(())
}

unsafe fn create_window(instance: &HMODULE, name: &PCWSTR) -> Result<()> {
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        *name,
        w!("Sample window"),
        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        None,
        None,
        *instance,
        None,
    )?;
    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = UpdateWindow(hwnd);
    Ok(())
}

unsafe fn create_listview(instance: &HMODULE, parent: &WindowHandle) -> Result<WindowHandle> {
    let style = WS_TABSTOP | WS_CHILD | WS_BORDER | WS_VISIBLE;
    let lv_style = LVS_AUTOARRANGE | LVS_REPORT; // | LVS_OWNERDATA;
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
        HMENU(ID_LISTVIEW as *mut c_void),
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
    resize_listview(&handle, &parent);
    Ok(handle)
}

fn resize_listview(listview: &WindowHandle, parent: &WindowHandle) {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetClientRect(parent.0, addr_of_mut!(rect));
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

fn listview_add_column(listview: &WindowHandle, title: &str) {
    let mut test = widestring::U16CString::from_str(title).unwrap();
    let header = PWSTR::from_raw(test.as_mut_ptr());
    let mut column = LVCOLUMNW {
        mask: LVCF_FMT | LVCF_WIDTH | LVCF_TEXT | LVCF_SUBITEM,
        fmt: LVCFMT_LEFT,
        cx: 120,
        pszText: header,
        ..Default::default()
    };
    unsafe {
        SendMessageW(
            listview.0,
            LVM_INSERTCOLUMN,
            WPARAM(0),
            LPARAM(addr_of_mut!(column) as isize),
        )
    };
}

fn init_listview(listview: &WindowHandle) {
    listview_add_column(listview, "PID");
    listview_add_column(listview, "Name");
}

fn listview_add_process(listview: &WindowHandle, process_name: &mut U16CString, pid: u32) {
    let item_text = PWSTR::from_raw(process_name.as_mut_ptr());
    let mut item = LVITEMW {
        mask: LVIF_TEXT,
        pszText: item_text,
        ..Default::default()
    };
    unsafe {
        SendMessageW(
            listview.0,
            LVM_INSERTITEM,
            WPARAM(0),
            LPARAM(addr_of_mut!(item) as isize),
        )
    };

    let pid_s = pid.to_string();
    let mut item_str2 = U16CString::from_str(pid_s).unwrap();
    let item_text2 = PWSTR::from_raw(item_str2.as_mut_ptr());
    item.pszText = item_text2;
    item.iSubItem = 1;
    unsafe {
        SendMessageW(
            listview.0,
            LVM_SETITEM,
            WPARAM(0),
            LPARAM(addr_of_mut!(item) as isize),
        )
    };
}

fn listview_clear(listview: &WindowHandle) {
    unsafe { SendMessageW(listview.0, LVM_DELETEALLITEMS, WPARAM(0), LPARAM(0))};
}

fn refresh_process_list(listview: &WindowHandle) {
    listview_clear(listview);
    let mut processes = process::get_processes().unwrap();
    for process in processes.iter_mut() {
        listview_add_process(listview, &mut process.image_name, process.pid);
    }
}

fn main() -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;

        let window_class = w!("window");
        register_class(&instance, &window_class)?;
        create_window(&instance, &window_class)?;

        let accel = LoadAcceleratorsW(instance, to_pcwstr(IDC_TASKMANAGER))?;
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).into() {
            if TranslateAcceleratorW(message.hwnd, accel, &message) == 0 {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    Ok(())
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let window_handle = WindowHandle::new(hwnd);
        match msg {
            WM_CREATE => {
                let instance = GetModuleHandleW(None).expect("shouldn't fail");
                let list_hwnd = create_listview(&instance, &window_handle).expect("shouldn't fail");
                init_listview(&list_hwnd);
                refresh_process_list(&list_hwnd);
                LRESULT(0)
            }
            WM_COMMAND => handle_wm_command(window_handle, msg, wparam, lparam),
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn handle_wm_command(hwnd: WindowHandle, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let id = (wparam.0 & 0xffff) as u16;
    match id {
        resources::IDM_EXIT => {
            unsafe {
                DestroyWindow(hwnd.0).unwrap();
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd.0, msg, wparam, lparam) },
    }
}
