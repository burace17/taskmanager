use std::rc::Rc;

use im::HashMap;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA,
};

use crate::{process::Process, HWND};

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

#[derive(Clone)]
pub struct TaskManagerState {
    pub task_list: HWND,
    pub status_bar: HWND,
    pub num_cpus: u32,
    pub sort_state: SortState,

    pub processes: Vec<Rc<Process>>,
    pub pid_map: HashMap<u32, Rc<Process>>,
}

// safety: SetWindowLongPtr needs to have been called to store the state prior to this
pub unsafe fn get(hwnd: HWND) -> TaskManagerState {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const TaskManagerState;
    (*state_ptr).clone()
}

// Update state atomically with a transformation function
unsafe fn update<F>(hwnd: HWND, f: F)
where
    F: FnOnce(&TaskManagerState) -> TaskManagerState,
{
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TaskManagerState;
    let old_state = &*state_ptr;
    let new_state = f(old_state);
    *state_ptr = new_state;
}

pub unsafe fn update_processes(
    hwnd: HWND,
    new_processes: Vec<Rc<Process>>,
    new_pid_map: HashMap<u32, Rc<Process>>,
) {
    update(hwnd, |old| TaskManagerState {
        task_list: old.task_list,
        status_bar: old.status_bar,
        num_cpus: old.num_cpus,
        sort_state: old.sort_state,
        processes: new_processes,
        pid_map: new_pid_map,
    });
}

pub unsafe fn set_sort_state(hwnd: HWND, new_sort: SortState) {
    update(hwnd, |old| TaskManagerState {
        task_list: old.task_list,
        status_bar: old.status_bar,
        num_cpus: old.num_cpus,
        sort_state: new_sort,
        processes: old.processes.clone(),
        pid_map: old.pid_map.clone(),
    });
}

pub unsafe fn initialize(
    hwnd: HWND,
    task_list_hwnd: HWND,
    status_bar_hwnd: HWND,
    num_cpus: u32,
) {
    let state = TaskManagerState {
        task_list: task_list_hwnd,
        status_bar: status_bar_hwnd,
        num_cpus,
        sort_state: SortState::SortUp(SortKey::Name),
        processes: Vec::new(),
        pid_map: HashMap::new(),
    };

    let state_box = Box::new(state);
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state_box) as isize);
}

pub unsafe fn destroy(hwnd: HWND) {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TaskManagerState;
    if !state_ptr.is_null() {
        drop(Box::from_raw(state_ptr));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
    }
}
