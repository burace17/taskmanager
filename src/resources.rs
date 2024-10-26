use windows::core::PCWSTR;

pub fn to_pcwstr(id: u16) -> PCWSTR {
    PCWSTR(id as _)
}
// Keep up to date with resources.rc
pub const IDM_EXIT: u16 = 100;
pub const IDC_TASKMANAGER: u16 = 101;
pub const IDM_NEW_TASK: u16 = 104;
pub const IDM_END_TASK: u16 = 105;
pub const IDM_TASK_CONTEXT_MENU: u16 = 106;

pub const ID_TASK_LIST: i32 = 2000;
pub const ID_UPDATE_TIMER: u32 = 2001;
