//! What only one system needs.

/// A note from the system saying what has started playing; quiet about failing, as a note is a nicety.
/// On every system: notify-rust speaks to Windows' toasts, a Mac's Notification Centre and D-Bus elsewhere.
pub fn announce(title: &str, artist: &str) {
    let (title, artist) = (title.to_string(), artist.to_string());
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new()
            .summary(&title)
            .body(&artist)
            .appname("Noctorium")
            .timeout(notify_rust::Timeout::Milliseconds(5000))
            .show();
    });
}

/// Ties every program Noctorium starts -- yt-dlp, mpv -- to Noctorium itself, so that they end when it does,
/// whether it is closed, crashes or is ended from the Task Manager. Without this a yt-dlp left running
/// carries on, and the one-file yt-dlp leaves the copy of itself it unpacked lying in the temporary folder.
///
/// A job object that kills what is in it when the last handle to it closes, which is when this process
/// ends; children join their parent's job by themselves. Elsewhere nothing is needed: a closed pipe or a
/// timed-out wait already ends them, and mpv leaves when its song does.
#[cfg(windows)]
pub fn tie_children_to_this_process() {
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return;
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let set = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if set != 0 {
            // The handle is left open on purpose: it closes, and the job ends, when this process does.
            AssignProcessToJobObject(job, GetCurrentProcess());
        }
    }
}

#[cfg(not(windows))]
pub fn tie_children_to_this_process() {}

/// A media key on the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKey {
    PlayPause,
    Next,
    Previous,
}

/// What the hot-key thread heard: a media key, or one of the listener's global chords by its index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pressed {
    Media(MediaKey),
    Global(usize),
}

/// The hot-key thread, which lets go of its keys when this is dropped.
pub struct HotKeys {
    #[cfg(windows)]
    thread: u32,
    /// The media keys the system let Noctorium have.
    pub media: Vec<&'static str>,
    /// Which global chords were granted, by index; one another program holds is false.
    pub global: Vec<bool>,
}

impl Drop for HotKeys {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
            if self.thread != 0 {
                PostThreadMessageW(self.thread, WM_QUIT, 0, 0);
            }
        }
    }
}

/// Listens for the keyboard's media keys and the listener's global chords from any program, as the Kotlin
/// desktop does: registered with the system's own hot-key service rather than a keyboard hook, on a thread
/// with a message loop of its own. Stop is left to whoever else wants it -- nothing here stops.
///
/// `chords` are each a Windows virtual-key code and RegisterHotKey modifiers.
#[cfg(windows)]
pub fn listen(chords: Vec<(u32, u32)>, on_press: impl Fn(Pressed) + Send + 'static) -> HotKeys {
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};
    const MEDIA: [(i32, u32, MediaKey, &str); 3] = [
        (1, 0xB3, MediaKey::PlayPause, "Play/Pause"),
        (2, 0xB0, MediaKey::Next, "Next"),
        (3, 0xB1, MediaKey::Previous, "Previous"),
    ];
    const FIRST_GLOBAL: i32 = 100;
    let (sender, receiver) = std::sync::mpsc::channel();
    let count = chords.len();
    std::thread::Builder::new()
        .name("hot-keys".into())
        .spawn(move || unsafe {
            let media: Vec<&'static str> = MEDIA
                .iter()
                .filter(|(id, vk, _, _)| RegisterHotKey(std::ptr::null_mut(), *id, MOD_NOREPEAT, *vk) != 0)
                .map(|(_, _, _, name)| *name)
                .collect();
            let global: Vec<bool> = chords
                .iter()
                .enumerate()
                .map(|(i, (vk, modifiers))| RegisterHotKey(std::ptr::null_mut(), FIRST_GLOBAL + i as i32, modifiers | MOD_NOREPEAT, *vk) != 0)
                .collect();
            let _ = sender.send((GetCurrentThreadId(), media, global));
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                if message.message == WM_HOTKEY {
                    let id = message.wParam as i32;
                    if id >= FIRST_GLOBAL {
                        on_press(Pressed::Global((id - FIRST_GLOBAL) as usize));
                    } else if let Some((_, _, key, _)) = MEDIA.iter().find(|(m, _, _, _)| *m == id) {
                        on_press(Pressed::Media(*key));
                    }
                }
            }
            for (id, _, _, _) in MEDIA {
                UnregisterHotKey(std::ptr::null_mut(), id);
            }
            for i in 0..count {
                UnregisterHotKey(std::ptr::null_mut(), FIRST_GLOBAL + i as i32);
            }
        })
        .ok();
    let (thread, media, global) = receiver
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap_or((0, Vec::new(), vec![false; count]));
    HotKeys { thread, media, global }
}

#[cfg(not(windows))]
pub fn listen(chords: Vec<(u32, u32)>, _on_press: impl Fn(Pressed) + Send + 'static) -> HotKeys {
    HotKeys { media: Vec::new(), global: vec![false; chords.len()] }
}
