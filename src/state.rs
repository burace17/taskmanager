use std::{cell::RefCell, collections::HashMap, rc::Rc};

use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA,
};

use crate::{process::Process, WindowHandle};

#[derive(Clone, Copy)]
pub enum SortState {
    SortUp(SortKey),
    SortDown(SortKey),
}

#[derive(Clone, Copy)]
pub enum SortKey {
    Name,
    Pid,
    Cpu,
    Memory,
}

pub struct TaskManagerState {
    pub task_list: WindowHandle,
    pub processes: Vec<Process>,
    pub sort_state: SortState,
    pub pid_map: HashMap<u32, Process>,
    pub num_cpus: u32,
}

// safety: SetWindowLongPtr needs to have been called to store the state prior to this
pub unsafe fn get(hwnd: WindowHandle) -> Rc<RefCell<TaskManagerState>> {
    let app_state_ptr =
        GetWindowLongPtrW(hwnd.0, GWLP_USERDATA) as *const RefCell<TaskManagerState>;
    let app_state = Rc::from_raw(app_state_ptr);
    Rc::increment_strong_count(app_state_ptr);
    app_state
}

// safety: GWLP_USERDATA for hwnd must be unset
pub unsafe fn initialize(hwnd: WindowHandle, task_list_hwnd: WindowHandle, num_cpus: u32) {
    let app_state = Rc::new(RefCell::new(TaskManagerState {
        task_list: task_list_hwnd,
        processes: Vec::new(),
        sort_state: SortState::SortUp(SortKey::Name),
        pid_map: HashMap::new(),
        num_cpus,
    }));
    let app_state_ptr = Rc::<RefCell<TaskManagerState>>::into_raw(app_state);
    SetWindowLongPtrW(hwnd.0, GWLP_USERDATA, app_state_ptr as isize);
}
