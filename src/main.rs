use std::{
    cell::RefCell, cmp::min, collections::HashMap, ffi::c_void, mem::transmute, ptr::addr_of_mut,
    rc::Rc,
};

use process::Process;
use widestring::U16CString;
use windows::{
    core::{w, Result, PCWSTR, PWSTR},
    Win32::{
        Foundation::{HMODULE, HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM},
        Graphics::Gdi::UpdateWindow,
        System::{
            LibraryLoader::GetModuleHandleW,
            SystemInformation::{GetSystemInfo, SYSTEM_INFO},
        },
        UI::{
            Controls::{
                LIST_VIEW_ITEM_FLAGS, LVCFMT_LEFT, LVCFMT_RIGHT, LVCF_FMT, LVCF_SUBITEM, LVCF_TEXT,
                LVCF_WIDTH, LVCOLUMNW, LVCOLUMNW_FORMAT, LVIF_TEXT, LVM_INSERTCOLUMN,
                LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETITEMCOUNT, LVN_COLUMNCLICK, LVN_GETDISPINFO,
                LVSICF_NOINVALIDATEALL, LVSICF_NOSCROLL, LVS_AUTOARRANGE, LVS_EX_FULLROWSELECT,
                LVS_OWNERDATA, LVS_REPORT, NMHDR, NMLISTVIEW, NMLVDISPINFOW, WC_LISTVIEWW,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect,
                GetMessageW, GetWindowLongPtrW, KillTimer, LoadAcceleratorsW, LoadCursorW,
                MoveWindow, PostQuitMessage, RegisterClassExW, SendMessageW, SetTimer,
                SetWindowLongPtrW, ShowWindow, TranslateAcceleratorW, TranslateMessage, CS_HREDRAW,
                CS_VREDRAW, CW_USEDEFAULT, GWLP_USERDATA, HMENU, IDC_ARROW, MSG, SW_SHOW,
                WINDOW_EX_STYLE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_NOTIFY, WM_SIZE, WM_TIMER,
                WNDCLASSEXW, WS_BORDER, WS_CHILD, WS_EX_CLIENTEDGE, WS_OVERLAPPEDWINDOW,
                WS_TABSTOP, WS_VISIBLE,
            },
        },
    },
};

use crate::resources::{to_pcwstr, IDC_TASKMANAGER};

mod process;
mod resources;
mod run_dialog;

const ID_LISTVIEW: i32 = 2000;
const ID_UPDATE_TIMER: u32 = 2001;

const INDEX_NAME: i32 = 0;
const INDEX_PID: i32 = 1;
const INDEX_CPU: i32 = 2;
const INDEX_MEMORY: i32 = 3;

// Container for a valid window handle
// Initialize with new()
struct WindowHandle(HWND);

impl WindowHandle {
    pub unsafe fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }
}

struct TaskManagerState {
    listview: WindowHandle,
    processes: Vec<Process>,
    pid_map: HashMap<u32, Process>,
    num_cpus: u32,
}

// safety: SetWindowLongPtr needs to have been called to store the state prior to this
unsafe fn get_task_manager_state(hwnd: &WindowHandle) -> Rc<RefCell<TaskManagerState>> {
    let app_state_ptr =
        GetWindowLongPtrW(hwnd.0, GWLP_USERDATA) as *const RefCell<TaskManagerState>;
    let app_state = Rc::from_raw(app_state_ptr);
    Rc::increment_strong_count(app_state_ptr);
    app_state
}

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
        w!("taskmgr--"),
        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        800,
        600,
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
    resize_listview(&handle, parent);
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

fn listview_add_column(
    listview: &WindowHandle,
    title: &str,
    order: i32,
    width: i32,
    fmt: LVCOLUMNW_FORMAT,
) {
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
            LPARAM(addr_of_mut!(column) as isize),
        )
    };
}

fn init_listview(listview: &WindowHandle) {
    listview_add_column(listview, "Name", INDEX_NAME, 400, LVCFMT_LEFT);
    listview_add_column(listview, "PID", INDEX_PID, 50, LVCFMT_LEFT);
    listview_add_column(listview, "CPU", INDEX_CPU, 50, LVCFMT_RIGHT);
    listview_add_column(listview, "Memory", INDEX_MEMORY, 90, LVCFMT_RIGHT);
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

unsafe fn listview_get_display_info(hwnd: &WindowHandle, lparam: LPARAM) {
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

unsafe fn listview_column_click(_hwnd: &WindowHandle, lparam: LPARAM) {
    let lpdi = transmute::<LPARAM, *const NMLISTVIEW>(lparam);
    let lpdi = &(*lpdi);
    println!("column click: {}", lpdi.iSubItem);
}

unsafe fn handle_wm_notify(hwnd: &WindowHandle, lparam: LPARAM) {
    let lpnmh = transmute::<LPARAM, *const NMHDR>(lparam);
    //let listview_handle = GetDlgItem(hwnd.0, ID_LISTVIEW);
    let code = (*lpnmh).code;
    match code {
        LVN_GETDISPINFO => listview_get_display_info(hwnd, lparam),
        LVN_COLUMNCLICK => listview_column_click(hwnd, lparam),
        _ => {}
    }
}

fn refresh_process_list(main_window: &WindowHandle) {
    let app_state = unsafe { get_task_manager_state(main_window) };
    let mut app_state = app_state.borrow_mut();
    let mut new_processes = process::get_processes().unwrap();
    let mut new_pid_map = HashMap::new();
    for process in new_processes.iter_mut() {
        new_pid_map.insert(process.pid, process.clone());
        if let Some(old_process) = app_state.pid_map.get(&process.pid) {
            process.cpu_usage = process::get_cpu_usage(old_process, process, app_state.num_cpus);
        }
    }

    app_state.processes = new_processes;
    app_state.pid_map = new_pid_map;

    let listview_behavior = LVSICF_NOINVALIDATEALL | LVSICF_NOSCROLL;
    unsafe {
        SendMessageW(
            app_state.listview.0,
            LVM_SETITEMCOUNT,
            WPARAM(app_state.processes.len()),
            LPARAM(listview_behavior as isize),
        )
    };
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

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let window_handle = WindowHandle::new(hwnd);
    match msg {
        WM_CREATE => {
            let instance = GetModuleHandleW(None).expect("shouldn't fail");
            let list_hwnd = create_listview(&instance, &window_handle).expect("shouldn't fail");
            init_listview(&list_hwnd);

            let mut system_info = SYSTEM_INFO::default();
            GetSystemInfo(addr_of_mut!(system_info));

            let app_state = Rc::new(RefCell::new(TaskManagerState {
                listview: list_hwnd,
                processes: Vec::new(),
                pid_map: HashMap::new(),
                num_cpus: system_info.dwNumberOfProcessors,
            }));
            let app_state_ptr = Rc::<RefCell<TaskManagerState>>::into_raw(app_state);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, app_state_ptr as isize);

            refresh_process_list(&window_handle);
            SetTimer(hwnd, ID_UPDATE_TIMER as usize, 500, None);
            LRESULT(0)
        }
        WM_COMMAND => handle_wm_command(window_handle, msg, wparam, lparam),
        WM_DESTROY => {
            let _ = KillTimer(hwnd, ID_UPDATE_TIMER as usize);
            let app_state = Rc::from_raw(
                GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<TaskManagerState>
            );
            println!("app ref count = {} ", Rc::strong_count(&app_state));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(app_state);
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_TIMER => {
            refresh_process_list(&window_handle);
            LRESULT(0)
        }
        WM_NOTIFY => {
            handle_wm_notify(&window_handle, lparam);
            LRESULT(0)
        }
        WM_SIZE => {
            let app_state = get_task_manager_state(&window_handle);
            resize_listview(&app_state.borrow().listview, &window_handle);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn handle_wm_command(hwnd: WindowHandle, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let id = (wparam.0 & 0xffff) as u16;
    match id {
        resources::IDM_NEW_TASK => {
            if let Err(err) = run_dialog::show(&hwnd) {
                println!("run_file error: {}", err);
            }
            LRESULT(0)
        }
        resources::IDM_EXIT => {
            DestroyWindow(hwnd.0).unwrap();
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd.0, msg, wparam, lparam),
    }
}
