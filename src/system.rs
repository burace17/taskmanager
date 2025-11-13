use windows::{
    core::{w, Error, Result, PCWSTR},
    Win32::{
        Foundation::{ERROR_SUCCESS, WIN32_ERROR},
        System::{
            Performance::{
                PdhAddCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue,
                PdhOpenQueryW, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
            },
            SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        },
    },
};

pub fn get_memory_status() -> Result<MEMORYSTATUSEX> {
    let mut memory_status = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe { GlobalMemoryStatusEx(&mut memory_status)? }
    Ok(memory_status)
}

pub fn start_query_data_collection() -> Result<(PDH_HQUERY, PDH_HCOUNTER)> {
    let mut query = PDH_HQUERY::default();
    let status = unsafe { WIN32_ERROR(PdhOpenQueryW(PCWSTR(std::ptr::null()), 0, &mut query)) };
    if status != ERROR_SUCCESS {
        let err = Error::from_thread();
        eprintln!("failed to open pdhquery: {}", err);
        return Err(err);
    }

    let mut counter = PDH_HCOUNTER::default();
    let status = unsafe {
        WIN32_ERROR(PdhAddCounterW(
            query,
            w!("\\Processor(_Total)\\% Processor Time"),
            0,
            &mut counter,
        ))
    };
    if status != ERROR_SUCCESS {
        let err = Error::from_thread();
        eprintln!("failed to add counter: {}", err);
        return Err(err);
    }

    collect_query_data(query)?;
    Ok((query, counter))
}

pub fn collect_query_data(query: PDH_HQUERY) -> Result<()> {
    let status = unsafe { WIN32_ERROR(PdhCollectQueryData(query)) };
    if status != ERROR_SUCCESS {
        let err = Error::from_thread();
        eprintln!("failed to collect query data: {}", err);
        return Err(err);
    }
    Ok(())
}

pub fn get_cpu_usage(counter: PDH_HCOUNTER) -> Result<f64> {
    let mut value = PDH_FMT_COUNTERVALUE::default();
    let status = unsafe {
        WIN32_ERROR(PdhGetFormattedCounterValue(
            counter,
            PDH_FMT_DOUBLE,
            None,
            &mut value,
        ))
    };
    if status != ERROR_SUCCESS {
        let err = Error::from_thread();
        eprintln!("failed to get formatted counter value: {}", err);
        return Err(err);
    }

    unsafe { Ok(value.Anonymous.doubleValue) }
}

pub fn end_query_data_collection(query: PDH_HQUERY) {
    if !query.is_invalid() {
        unsafe { PdhCloseQuery(query) };
    }
}
