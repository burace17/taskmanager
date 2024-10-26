use std::{cell::RefCell, collections::HashMap, mem::transmute, rc::Rc};

use resources::ID_UPDATE_TIMER;
use state::TaskManagerState;
use window::WindowHandle;
use windows::{
    core::{w, Result},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
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
                DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
                GetWindowLongPtrW, KillTimer, LoadAcceleratorsW, PostQuitMessage, SendMessageW, SetTimer, SetWindowLongPtrW,
                TranslateAcceleratorW, TranslateMessage,
                GWLP_USERDATA, MSG, WM_COMMAND, WM_CREATE,
                WM_DESTROY, WM_NOTIFY, WM_SIZE, WM_TIMER,
            },
        },
    },
};

use crate::resources::{to_pcwstr, IDC_TASKMANAGER};

mod process;
mod resources;
mod run_dialog;
mod state;
mod task_list;
mod window;

fn main() -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;

        let window_class = w!("window");
        window::register_class(&instance, &window_class, wndproc)?;
        window::create_window(&instance, &window_class)?;

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

fn refresh_process_list(main_window: WindowHandle) {
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

    app_state.processes = new_processes;
    app_state.pid_map = new_pid_map;

    let listview_behavior = LVSICF_NOINVALIDATEALL | LVSICF_NOSCROLL;
    unsafe {
        SendMessageW(
            app_state.task_list.0,
            LVM_SETITEMCOUNT,
            WPARAM(app_state.processes.len()),
            LPARAM(listview_behavior as isize),
        )
    };
}

fn on_wm_create(hwnd: WindowHandle) -> LRESULT {
    unsafe {
        let instance = GetModuleHandleW(None).expect("shouldn't fail");
        let task_list_hwnd = task_list::create_control(&instance, hwnd).expect("shouldn't fail");

        let mut system_info = SYSTEM_INFO::default();
        GetSystemInfo(&mut system_info);

        state::initialize(hwnd, task_list_hwnd, system_info.dwNumberOfProcessors);

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
    let app_state = unsafe { state::get(hwnd) };
    task_list::resize_to_parent(app_state.borrow().task_list, hwnd);
    LRESULT(0)
}
