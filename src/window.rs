use windows::{
    core::{w, Result, PCWSTR},
    Win32::{
        Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::UpdateWindow,
        UI::WindowsAndMessaging::{
                CreateWindowExW, LoadCursorW,
                RegisterClassExW, ShowWindow, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, SW_SHOW, WINDOW_EX_STYLE, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
                WS_VISIBLE,
            },
    },
};

use crate::resources::{to_pcwstr, IDC_TASKMANAGER};

// Container for a valid window handle
// Initialize with new()
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowHandle(pub HWND);

impl WindowHandle {
    pub unsafe fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }
}

type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

pub unsafe fn register_class(instance: &HMODULE, name: &PCWSTR, wndproc: WndProc) -> Result<()> {
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

pub unsafe fn create_window(instance: &HMODULE, name: &PCWSTR) -> Result<()> {
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