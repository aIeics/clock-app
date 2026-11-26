//! One Clock at a time. A second launch (say, from the Start menu while the
//! first one sits in the tray) asks the running app to show its window, then
//! exits instead of adding a second tray icon.
//!
//! A named mutex marks the running app; the installer checks the same name
//! (`AppMutex` in `installer/clock.iss`) so it can ask to close Clock before
//! updating or uninstalling. A named auto-reset event carries the "show your
//! window" request. The listener thread sleeps in the kernel until then, so
//! nothing polls.

/// Also used by the installer; keep both in sync.
pub const MUTEX_NAME: &str = "friends-clock-app";
#[cfg_attr(not(windows), allow(dead_code))]
const EVENT_NAME: &str = "friends-clock-app.show";

pub enum Launch {
    /// No other Clock is running: start normally.
    First(Running),
    /// Another Clock is running and was asked to show its window.
    AlreadyRunning,
}

pub use imp::{Running, claim};

#[cfg(windows)]
mod imp {
    use std::ptr::null;

    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Threading::{
        CreateEventW, CreateMutexW, INFINITE, SetEvent, WaitForSingleObject,
    };

    use super::{EVENT_NAME, Launch, MUTEX_NAME};

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    /// The running app's "show your window" event. Its handle stays open for
    /// the life of the process, like the mutex's.
    pub struct Running {
        /// The event `HANDLE` as an address, so it can move to the listener
        /// thread. Zero if the event could not be created.
        event: usize,
    }

    impl Running {
        /// Calls `show` on a background thread each time another launch asks
        /// for the window.
        pub fn on_show_request(self, show: impl Fn() + Send + 'static) {
            if self.event == 0 {
                return;
            }
            let spawned = std::thread::Builder::new()
                .name("clock-app-instance".into())
                .spawn(move || {
                    let event = self.event as HANDLE;
                    // SAFETY: the handle is a valid event that is never closed.
                    while unsafe { WaitForSingleObject(event, INFINITE) } == WAIT_OBJECT_0 {
                        show();
                    }
                });
            if let Err(err) = spawned {
                eprintln!("clock-app: could not listen for other launches: {err}");
            }
        }
    }

    pub fn claim() -> Launch {
        let (mutex_name, event_name) = (wide(MUTEX_NAME), wide(EVENT_NAME));

        // SAFETY: the name is NUL-terminated and outlives the call. The handle
        // is deliberately never closed: it marks this process as running.
        let mutex = unsafe { CreateMutexW(null(), 0, mutex_name.as_ptr()) };
        // SAFETY: reads the calling thread's last error, set by CreateMutexW.
        let already_running = !mutex.is_null() && unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;

        // Auto-reset, so each signal wakes the listener once. Opens the
        // existing event when another Clock created it first.
        // SAFETY: the name is NUL-terminated and outlives the call.
        let event = unsafe { CreateEventW(null(), 0, 0, event_name.as_ptr()) };

        if already_running {
            // SAFETY: both handles were just created by this process and are
            // checked for null before use.
            unsafe {
                if !event.is_null() {
                    SetEvent(event);
                    CloseHandle(event);
                }
                CloseHandle(mutex);
            }
            return Launch::AlreadyRunning;
        }

        Launch::First(Running {
            event: event as usize,
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Launch;

    pub struct Running;

    impl Running {
        pub fn on_show_request(self, _show: impl Fn() + Send + 'static) {}
    }

    pub fn claim() -> Launch {
        Launch::First(Running)
    }
}
