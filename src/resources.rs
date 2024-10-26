use windows::core::PCWSTR;

pub fn to_pcwstr(id: u16) -> PCWSTR {
    PCWSTR(id as _)
}
// Keep up to date with resources.rc
pub const IDM_EXIT: u16 = 100;
pub const IDC_TASKMANAGER: u16 = 101;
pub const IDM_NEW_TASK: u16 = 104;
