use std::{
    mem::transmute,
    ptr::{null, null_mut},
};

use windows::{
    core::{w, Result, PCSTR, PCWSTR},
    Win32::{
        Foundation::{FreeLibrary, HWND},
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
        UI::WindowsAndMessaging::HICON,
    },
};

use crate::WindowHandle;

type RunDlgFunc = unsafe extern "system" fn(
    hwndowner: HWND,
    icon: HICON,
    lpszdir: PCWSTR,
    lpsztitle: PCWSTR,
    lpszdesc: PCWSTR,
    dwflags: i32,
);

pub fn show(owner: &WindowHandle) -> Result<()> {
    unsafe {
        let shell32 = LoadLibraryW(w!("shell32.dll"))?;
        let run_dlg_proc = GetProcAddress(shell32, PCSTR(61 as *const u8));

        if let Some(run_dlg_proc) = run_dlg_proc {
            let run_dlg_proc =
                transmute::<unsafe extern "system" fn() -> isize, RunDlgFunc>(run_dlg_proc);
            run_dlg_proc(
                owner.0,
                HICON(null_mut()),
                PCWSTR(null()),
                w!("Create new task"),
                PCWSTR(null()),
                0,
            );
        }
        FreeLibrary(shell32)?;
        Ok(())
    }
}
