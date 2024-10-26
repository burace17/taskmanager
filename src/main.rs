use std::{cell::RefCell, collections::HashMap, mem::transmute, rc::Rc};

use process::Process;
use resources::ID_UPDATE_TIMER;
use windows::{
    core::{w, Result, PCWSTR},
    Win32::{
        Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::UpdateWindow,
        System::{
            LibraryLoader::GetModuleHandleW,
            SystemInformation::{GetSystemInfo, SYSTEM_INFO},
        },
        UI::{
            Controls::{
                LVM_SETITEMCOUNT, LVN_COLUMNCLICK, LVN_GETDISPINFO, LVSICF_NOINVALIDATEALL,
                LVSICF_NOSCROLL, NMHDR,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
                GetWindowLongPtrW, KillTimer, LoadAcceleratorsW, LoadCursorW, PostQuitMessage,
                RegisterClassExW, SendMessageW, SetTimer, SetWindowLongPtrW, ShowWindow,
                TranslateAcceleratorW, TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT,
                GWLP_USERDATA, IDC_ARROW, MSG, SW_SHOW, WINDOW_EX_STYLE, WM_COMMAND, WM_CREATE,
                WM_DESTROY, WM_NOTIFY, WM_SIZE, WM_TIMER, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
                WS_VISIBLE,
            },
        },
    },
};

use crate::resources::{to_pcwstr, IDC_TASKMANAGER};

mod process;
mod resources;
mod run_dialog;
mod task_list;

// Container for a valid window handle
// Initialize with new()
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowHandle(HWND);

impl WindowHandle {
    pub(crate) unsafe fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }
}

pub struct TaskManagerState {
    listview: WindowHandle,
    processes: Vec<Process>,
    pid_map: HashMap<u32, Process>,
    num_cpus: u32,
}

// safety: SetWindowLongPtr needs to have been called to store the state prior to this
pub(crate) unsafe fn get_task_manager_state(hwnd: WindowHandle) -> Rc<RefCell<TaskManagerState>> {
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

fn refresh_process_list(main_window: WindowHandle) {
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
    let hwnd = WindowHandle::new(hwnd);
    match msg {
        WM_CREATE => on_wm_create(hwnd),
        WM_COMMAND => on_wm_command(hwnd, msg, wparam, lparam),
        WM_DESTROY => on_wm_destroy(hwnd),
        WM_TIMER => on_wm_timer(hwnd),
        WM_NOTIFY => on_wm_notify(hwnd, lparam),
        WM_SIZE => on_wm_size(hwnd),
        _ => DefWindowProcW(hwnd.0, msg, wparam, lparam),
    }
}

fn on_wm_create(hwnd: WindowHandle) -> LRESULT {
    unsafe {
        let instance = GetModuleHandleW(None).expect("shouldn't fail");
        let list_hwnd = task_list::create_control(&instance, hwnd).expect("shouldn't fail");

        let mut system_info = SYSTEM_INFO::default();
        GetSystemInfo(&mut system_info);

        let app_state = Rc::new(RefCell::new(TaskManagerState {
            listview: list_hwnd,
            processes: Vec::new(),
            pid_map: HashMap::new(),
            num_cpus: system_info.dwNumberOfProcessors,
        }));
        let app_state_ptr = Rc::<RefCell<TaskManagerState>>::into_raw(app_state);
        SetWindowLongPtrW(hwnd.0, GWLP_USERDATA, app_state_ptr as isize);

        refresh_process_list(hwnd);
        SetTimer(hwnd.0, ID_UPDATE_TIMER as usize, 500, None);
    }
    LRESULT(0)
}

fn on_wm_destroy(hwnd: WindowHandle) -> LRESULT {
    unsafe {
        let _ = KillTimer(hwnd.0, ID_UPDATE_TIMER as usize);
        let app_state = Rc::from_raw(
            GetWindowLongPtrW(hwnd.0, GWLP_USERDATA) as *const RefCell<TaskManagerState>
        );
        println!("app ref count = {} ", Rc::strong_count(&app_state));
        SetWindowLongPtrW(hwnd.0, GWLP_USERDATA, 0);
        drop(app_state);
        PostQuitMessage(0);
        LRESULT(0)
    }
}

fn on_wm_timer(hwnd: WindowHandle) -> LRESULT {
    refresh_process_list(hwnd);
    LRESULT(0)
}

unsafe fn on_wm_command(hwnd: WindowHandle, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
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

unsafe fn on_wm_notify(hwnd: WindowHandle, lparam: LPARAM) -> LRESULT {
    let lpnmh = transmute::<LPARAM, *const NMHDR>(lparam);
    //let listview_handle = GetDlgItem(hwnd.0, ID_LISTVIEW);
    let code = (*lpnmh).code;
    match code {
        LVN_GETDISPINFO => task_list::on_get_display_info(hwnd, lparam),
        LVN_COLUMNCLICK => task_list::on_column_click(hwnd, lparam),
        _ => {}
    }
    LRESULT(0)
}

fn on_wm_size(hwnd: WindowHandle) -> LRESULT {
    // safety: WM_CREATE will ensure the state has been stored in the window first
    let app_state = unsafe { get_task_manager_state(hwnd) };
    task_list::resize_to_parent(app_state.borrow().listview, hwnd);
    LRESULT(0)
}
