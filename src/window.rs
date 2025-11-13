use windows::{
    core::{w, Result, PCWSTR},
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::UpdateWindow,
        UI::{
            Controls::{InitCommonControlsEx, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX},
            WindowsAndMessaging::*,
        },
    },
};

use crate::resources::{to_pcwstr, IDC_TASKMANAGER};

type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

pub unsafe fn register_class(instance: &HINSTANCE, name: &PCWSTR, wndproc: WndProc) -> Result<()> {
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpszClassName: *name,
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        lpfnWndProc: Some(wndproc),
        hInstance: *instance,
        lpszMenuName: to_pcwstr(IDC_TASKMANAGER),
        ..Default::default()
    };

    let atom = RegisterClassExW(&wc);
    debug_assert!(atom != 0);
    Ok(())
}

pub unsafe fn create_window(instance: &HINSTANCE, name: &PCWSTR) -> Result<()> {
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        *name,
        w!("taskmanager--"),
        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        800,
        600,
        None,
        None,
        Some(*instance),
        None,
    )?;
    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = UpdateWindow(hwnd);
    Ok(())
}

pub fn init_common_controls() {
    let common_controls = INITCOMMONCONTROLSEX {
        dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_STANDARD_CLASSES,
    };
    let _ = unsafe { InitCommonControlsEx(&common_controls) };
}
