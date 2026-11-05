//! Single instance process lock for Windows desktop applications and daemons.
//!
//! Uses a system-wide named Mutex to guarantee that only one instance of the
//! background daemon or cat overlay can run concurrently per Windows user session.
//! If an existing instance is already active, subsequent invocations terminate cleanly
//! with an informational log instead of spawning duplicate processes or corrupting state.

#[cfg(windows)]
pub struct SingleInstanceGuard {
    handle: windows::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl SingleInstanceGuard {
    /// Acquire an exclusive single-instance lock using a named mutex.
    /// Returns `Some(SingleInstanceGuard)` if this is the first instance,
    /// or `None` if an instance is already running.
    pub fn acquire(name: &str) -> Option<Self> {
        use windows::core::HSTRING;
        use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
        use windows::Win32::System::Threading::CreateMutexW;

        // Scope to current Windows user session
        let mutex_name = format!("Local\\{}", name);
        let h_string = HSTRING::from(mutex_name.as_str());

        unsafe {
            let handle = match CreateMutexW(None, true, &h_string) {
                Ok(h) => h,
                Err(_) => return None,
            };
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return None;
            }
            Some(Self { handle })
        }
    }
}

#[cfg(windows)]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(not(windows))]
pub struct SingleInstanceGuard;

#[cfg(not(windows))]
impl SingleInstanceGuard {
    pub fn acquire(_name: &str) -> Option<Self> {
        Some(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_instance_acquisition_and_conflict() {
        let lock_name = "OSMOO_TEST_SINGLE_INSTANCE_UNIQUE_MUTEX";
        let first = SingleInstanceGuard::acquire(lock_name);
        assert!(first.is_some(), "First acquisition must succeed");

        #[cfg(windows)]
        {
            let second = SingleInstanceGuard::acquire(lock_name);
            assert!(
                second.is_none(),
                "Second acquisition of same mutex must return None (conflict blocked)"
            );
        }

        drop(first);

        let third = SingleInstanceGuard::acquire(lock_name);
        assert!(third.is_some(), "Re-acquisition after drop must succeed");
    }
}
