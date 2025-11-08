#![windows_subsystem = "windows"]

use std::{cell::RefCell, mem::transmute, rc::Rc};

use resources::{FALSE, IDD_ABOUTBOX, ID_UPDATE_TIMER, TRUE};
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
            Controls::{LVN_COLUMNCLICK, LVN_GETDISPINFO, NMHDR},
            WindowsAndMessaging::{
                DefWindowProcW, DestroyWindow, DialogBoxParamW, DispatchMessageW, EndDialog,
                GetMessageW, GetWindowLongPtrW, KillTimer, LoadAcceleratorsW, PostQuitMessage,
                SetTimer, SetWindowLongPtrW, TranslateAcceleratorW, TranslateMessage,
                GWLP_USERDATA, MSG, WM_COMMAND, WM_CONTEXTMENU, WM_CREATE, WM_DESTROY,
                WM_INITDIALOG, WM_NOTIFY, WM_SIZE, WM_TIMER,
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
        window::init_common_controls();
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
        WM_NOTIFY => on_wm_notify(hwnd, wparam, lparam),
        WM_SIZE => on_wm_size(hwnd),
        WM_CONTEXTMENU => on_wm_contextmenu(hwnd, lparam),
        _ => DefWindowProcW(hwnd.0, msg, wparam, lparam),
    }
}

const IDOK: usize = windows::Win32::UI::WindowsAndMessaging::IDOK.0 as usize;
const IDCANCEL: usize = windows::Win32::UI::WindowsAndMessaging::IDCANCEL.0 as usize;
unsafe extern "system" fn aboutdlgproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    _lparam: LPARAM,
) -> isize {
    match msg {
        WM_COMMAND if wparam.0 == IDOK || wparam.0 == IDCANCEL => {
            let _ = EndDialog(hwnd, IDOK as isize);
            TRUE
        }
        WM_INITDIALOG | WM_COMMAND => TRUE,
        _ => FALSE,
    }
}

fn on_wm_create(hwnd: WindowHandle) -> LRESULT {
    unsafe {
        let instance = GetModuleHandleW(None).expect("shouldn't fail");
        let task_list_hwnd = task_list::create_control(&instance, hwnd).expect("shouldn't fail");

        let mut system_info = SYSTEM_INFO::default();
        GetSystemInfo(&mut system_info);

        state::initialize(hwnd, task_list_hwnd, system_info.dwNumberOfProcessors);

        task_list::refresh_process_list(hwnd, false);
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
        debug_assert_eq!(Rc::strong_count(&app_state), 1);
        SetWindowLongPtrW(hwnd.0, GWLP_USERDATA, 0);
        drop(app_state);
        PostQuitMessage(0);
        LRESULT(0)
    }
}

fn on_wm_timer(hwnd: WindowHandle) -> LRESULT {
    task_list::refresh_process_list(hwnd, true);
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
        resources::IDM_ABOUT => {
            let instance = GetModuleHandleW(None).expect("shouldn't fail");
            let _ = DialogBoxParamW(
                instance,
                to_pcwstr(IDD_ABOUTBOX),
                hwnd.0,
                Some(aboutdlgproc),
                LPARAM(0),
            );
            LRESULT(0)
        }
        resources::IDM_END_TASK => task_list::on_end_task_clicked(hwnd),
        _ => DefWindowProcW(hwnd.0, msg, wparam, lparam),
    }
}

unsafe fn on_wm_notify(hwnd: WindowHandle, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let lpnmh = transmute::<LPARAM, *const NMHDR>(lparam);
    let code = (*lpnmh).code;
    match code {
        LVN_GETDISPINFO => task_list::on_get_display_info(hwnd, lparam),
        LVN_COLUMNCLICK => task_list::on_column_click(hwnd, lparam),
        _ => {
            return DefWindowProcW(hwnd.0, WM_NOTIFY, wparam, lparam);
        }
    }
    LRESULT(0)
}

fn on_wm_size(hwnd: WindowHandle) -> LRESULT {
    // safety: WM_CREATE will ensure the state has been stored in the window first
    let app_state = unsafe { state::get(hwnd) };
    task_list::resize_to_parent(app_state.borrow().task_list, hwnd);
    LRESULT(0)
}

unsafe fn on_wm_contextmenu(hwnd: WindowHandle, lparam: LPARAM) -> LRESULT {
    let lparam = lparam.0 as i32;
    let x = lparam & 0xFFFF;
    let y = (lparam >> 16) & 0xFFFF;
    task_list::on_show_contextmenu(hwnd, x, y);
    LRESULT(0)
}
