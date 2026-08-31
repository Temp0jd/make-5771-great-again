#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetWindow {
    pub handle: isize,
    pub title: String,
    pub client_width: u32,
    pub client_height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum GlobalHotkey {
    CaptureTemplate,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum TrayEvent {
    Show,
    Exit,
}

pub struct TrayGuard {
    #[cfg(windows)]
    window: isize,
    #[cfg(windows)]
    thread_id: u32,
}

impl TrayGuard {
    #[cfg(windows)]
    pub fn show_notification(&self, title: &str, message: &str) {
        windows_impl::show_tray_notification(self.window, title, message);
    }

    #[cfg(not(windows))]
    #[allow(dead_code, reason = "used by the Windows completion reminder")]
    pub fn show_notification(&self, _title: &str, _message: &str) {}
}

#[cfg(windows)]
impl Drop for TrayGuard {
    fn drop(&mut self) {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

pub struct HotkeyGuard {
    #[cfg(windows)]
    thread_id: u32,
}

#[cfg(windows)]
impl Drop for HotkeyGuard {
    fn drop(&mut self) {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

#[derive(Debug)]
pub enum PlatformError {
    #[cfg(not(windows))]
    Unsupported,
    #[cfg(windows)]
    WindowNotFound(String),
    #[cfg(windows)]
    WindowsApi(String),
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(not(windows))]
            Self::Unsupported => write!(formatter, "窗口连接仅支持 Windows"),
            #[cfg(windows)]
            Self::WindowNotFound(title) => write!(formatter, "没有找到包含“{title}”的可见窗口"),
            #[cfg(windows)]
            Self::WindowsApi(message) => write!(formatter, "Windows API 错误：{message}"),
        }
    }
}

impl std::error::Error for PlatformError {}

#[cfg(windows)]
pub fn install_global_hotkeys()
-> Result<(std::sync::mpsc::Receiver<GlobalHotkey>, HotkeyGuard), PlatformError> {
    windows_impl::install_global_hotkeys()
}

#[cfg(not(windows))]
pub fn install_global_hotkeys()
-> Result<(std::sync::mpsc::Receiver<GlobalHotkey>, HotkeyGuard), PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub fn install_tray_icon()
-> Result<(std::sync::mpsc::Receiver<TrayEvent>, TrayGuard), PlatformError> {
    windows_impl::install_tray_icon()
}

#[cfg(not(windows))]
pub fn install_tray_icon()
-> Result<(std::sync::mpsc::Receiver<TrayEvent>, TrayGuard), PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub fn flash_app_window() {
    windows_impl::flash_app_window();
}

#[cfg(not(windows))]
pub fn flash_app_window() {}

#[cfg(windows)]
pub fn open_image_file_dialog() -> Result<Option<std::path::PathBuf>, PlatformError> {
    windows_impl::open_image_file_dialog()
}

#[cfg(windows)]
pub fn open_workflow_package_dialog() -> Result<Option<std::path::PathBuf>, PlatformError> {
    windows_impl::open_workflow_package_dialog()
}

#[cfg(not(windows))]
pub fn open_workflow_package_dialog() -> Result<Option<std::path::PathBuf>, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub fn save_workflow_package_dialog(
    suggested_name: &str,
) -> Result<Option<std::path::PathBuf>, PlatformError> {
    windows_impl::save_workflow_package_dialog(suggested_name)
}

#[cfg(not(windows))]
pub fn save_workflow_package_dialog(
    _suggested_name: &str,
) -> Result<Option<std::path::PathBuf>, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(not(windows))]
pub fn open_image_file_dialog() -> Result<Option<std::path::PathBuf>, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub fn find_target_window(title_fragment: &str) -> Result<TargetWindow, PlatformError> {
    windows_impl::find_target_window(title_fragment)
}

#[cfg(not(windows))]
pub fn find_target_window(_title_fragment: &str) -> Result<TargetWindow, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub fn list_visible_windows() -> Result<Vec<TargetWindow>, PlatformError> {
    windows_impl::list_visible_windows()
}

#[cfg(not(windows))]
pub fn list_visible_windows() -> Result<Vec<TargetWindow>, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub fn is_foreground(target: &TargetWindow) -> bool {
    windows_impl::is_foreground(target)
}

#[cfg(not(windows))]
pub fn is_foreground(_target: &TargetWindow) -> bool {
    false
}

#[cfg(windows)]
pub fn focus_target(target: &TargetWindow) -> Result<(), PlatformError> {
    windows_impl::focus_target(target)
}

#[cfg(not(windows))]
pub fn focus_target(_target: &TargetWindow) -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub fn click_client(target: &TargetWindow, x: u32, y: u32) -> Result<(), PlatformError> {
    windows_impl::click_client(target, x, y)
}

#[cfg(not(windows))]
#[allow(dead_code, reason = "used by the Windows workflow executor")]
pub fn click_client(_target: &TargetWindow, _x: u32, _y: u32) -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub fn click_client_background(target: &TargetWindow, x: u32, y: u32) -> Result<(), PlatformError> {
    windows_impl::click_client_background(target, x, y)
}

#[cfg(not(windows))]
#[allow(dead_code, reason = "used by the Windows workflow executor")]
pub fn click_client_background(
    _target: &TargetWindow,
    _x: u32,
    _y: u32,
) -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub fn is_window_alive(target: &TargetWindow) -> bool {
    windows_impl::is_window_alive(target)
}

#[cfg(not(windows))]
#[allow(dead_code, reason = "used by the Windows workflow executor")]
pub fn is_window_alive(_target: &TargetWindow) -> bool {
    true
}

#[cfg(windows)]
pub fn capture_client(target: &TargetWindow) -> Result<image::RgbaImage, PlatformError> {
    windows_impl::capture_client(target)
}

#[cfg(not(windows))]
#[allow(dead_code, reason = "used by the Windows template capture UI")]
pub fn capture_client(_target: &TargetWindow) -> Result<image::RgbaImage, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
mod windows_impl {
    use std::ffi::c_void;
    use std::mem::size_of;

    use image::{ImageBuffer, RgbaImage};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, ClientToScreen, CreateCompatibleBitmap,
        CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, HGDIOBJ,
        ReleaseDC, SRCCOPY, SelectObject,
    };
    use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetOpenFileNameW, GetSaveFileNameW, OFN_FILEMUSTEXIST,
        OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_MOUSE, MOD_NOREPEAT, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
        MOUSEINPUT, RegisterHotKey, SendInput, UnregisterHotKey, VK_F6, VK_F8,
    };
    use windows::Win32::UI::Shell::{
        ExtractIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE,
        NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
        DispatchMessageW, EnumWindows, FLASHW_ALL, FLASHW_TIMERNOFG, FLASHWINFO, FindWindowW,
        FlashWindowEx, GWLP_USERDATA, GetClientRect, GetCursorPos, GetForegroundWindow,
        GetMessageW, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, HWND_MESSAGE,
        IDI_APPLICATION, IsWindow, IsWindowVisible, MF_STRING, MSG, PostMessageW, RegisterClassW,
        SetCursorPos, SetForegroundWindow, SetWindowLongPtrW, TPM_NONOTIFY, TPM_RETURNCMD,
        TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WM_APP, WM_CREATE,
        WM_DESTROY, WM_HOTKEY, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_NULL,
        WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
    };
    use windows::Win32::UI::WindowsAndMessaging::{CREATESTRUCTW, LoadIconW};
    use windows::core::{BOOL, PCWSTR, PWSTR};

    use super::{GlobalHotkey, HotkeyGuard, PlatformError, TargetWindow, TrayEvent, TrayGuard};

    const CAPTURE_HOTKEY_ID: i32 = 577_106;
    const STOP_HOTKEY_ID: i32 = 577_108;
    const TRAY_ICON_ID: u32 = 5771;
    const WM_TRAY_CALLBACK: u32 = WM_APP + 71;
    const TRAY_MENU_SHOW: u32 = 1;
    const TRAY_MENU_EXIT: u32 = 2;
    const APP_TITLE: &str = "Make 5771 Great Again";

    struct WindowListState {
        entries: Vec<(HWND, String)>,
    }

    pub fn install_global_hotkeys()
    -> Result<(std::sync::mpsc::Receiver<GlobalHotkey>, HotkeyGuard), PlatformError> {
        let (event_sender, event_receiver) = std::sync::mpsc::channel();
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);

        std::thread::spawn(move || {
            let thread_id = unsafe { GetCurrentThreadId() };
            let registration = (|| -> Result<(), String> {
                unsafe {
                    RegisterHotKey(None, CAPTURE_HOTKEY_ID, MOD_NOREPEAT, VK_F6.0 as u32)
                        .map_err(|error| format!("无法注册 F6：{error}"))?;
                    if let Err(error) =
                        RegisterHotKey(None, STOP_HOTKEY_ID, MOD_NOREPEAT, VK_F8.0 as u32)
                    {
                        let _ = UnregisterHotKey(None, CAPTURE_HOTKEY_ID);
                        return Err(format!("无法注册 F8：{error}"));
                    }
                }
                Ok(())
            })();
            if let Err(error) = registration {
                let _ = ready_sender.send(Err(error));
                return;
            }
            if ready_sender.send(Ok(thread_id)).is_err() {
                unsafe {
                    let _ = UnregisterHotKey(None, CAPTURE_HOTKEY_ID);
                    let _ = UnregisterHotKey(None, STOP_HOTKEY_ID);
                }
                return;
            }

            let mut message = MSG::default();
            while unsafe { GetMessageW(&mut message, None, 0, 0).0 } > 0 {
                if message.message == WM_HOTKEY {
                    let hotkey = match message.wParam.0 as i32 {
                        CAPTURE_HOTKEY_ID => Some(GlobalHotkey::CaptureTemplate),
                        STOP_HOTKEY_ID => Some(GlobalHotkey::Stop),
                        _ => None,
                    };
                    if hotkey.is_some_and(|event| event_sender.send(event).is_err()) {
                        break;
                    }
                }
            }
            unsafe {
                let _ = UnregisterHotKey(None, CAPTURE_HOTKEY_ID);
                let _ = UnregisterHotKey(None, STOP_HOTKEY_ID);
            }
        });

        match ready_receiver.recv() {
            Ok(Ok(thread_id)) => Ok((event_receiver, HotkeyGuard { thread_id })),
            Ok(Err(error)) => Err(PlatformError::WindowsApi(error)),
            Err(error) => Err(PlatformError::WindowsApi(format!(
                "快捷键线程启动失败：{error}"
            ))),
        }
    }

    pub fn install_tray_icon()
    -> Result<(std::sync::mpsc::Receiver<TrayEvent>, TrayGuard), PlatformError> {
        let (event_sender, event_receiver) = std::sync::mpsc::channel();
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);

        std::thread::spawn(move || {
            let thread_id = unsafe { GetCurrentThreadId() };
            let window = match unsafe { setup_tray_window(event_sender) } {
                Ok(window) => window,
                Err(error) => {
                    let _ = ready_sender.send(Err(error));
                    return;
                }
            };
            if ready_sender
                .send(Ok((thread_id, window.0 as isize)))
                .is_err()
            {
                unsafe { cleanup_tray(window) };
                return;
            }

            let mut message = MSG::default();
            while unsafe { GetMessageW(&mut message, None, 0, 0).0 } > 0 {
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            unsafe { cleanup_tray(window) };
        });

        match ready_receiver.recv() {
            Ok(Ok((thread_id, window))) => Ok((event_receiver, TrayGuard { window, thread_id })),
            Ok(Err(error)) => Err(PlatformError::WindowsApi(error)),
            Err(error) => Err(PlatformError::WindowsApi(format!(
                "托盘线程启动失败：{error}"
            ))),
        }
    }

    type TrayEventSender = std::sync::mpsc::Sender<TrayEvent>;

    unsafe fn setup_tray_window(events: TrayEventSender) -> Result<HWND, String> {
        let class_name: Vec<u16> = "M5771TrayWindow\0".encode_utf16().collect();
        let instance = unsafe { GetModuleHandleW(None) }
            .map_err(|error| format!("无法获取程序实例：{error}"))?;
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(tray_window_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        if unsafe { RegisterClassW(&wnd_class) } == 0 {
            return Err("无法注册托盘窗口类".to_owned());
        }
        let window = match unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(class_name.as_ptr()),
                PCWSTR::null(),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance.into()),
                Some(Box::into_raw(Box::new(events)) as *const c_void),
            )
        } {
            Ok(window) => window,
            Err(_) => return Err("无法创建托盘消息窗口".to_owned()),
        };

        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: window,
            uID: TRAY_ICON_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAY_CALLBACK,
            hIcon: load_tray_icon(),
            szTip: wide_array(APP_TITLE),
            ..Default::default()
        };
        if !unsafe { Shell_NotifyIconW(NIM_ADD, &mut data) }.as_bool() {
            unsafe {
                let _ = DestroyWindow(window);
            }
            return Err("无法添加系统托盘图标".to_owned());
        }
        Ok(window)
    }

    unsafe fn cleanup_tray(window: HWND) {
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: window,
            uID: TRAY_ICON_ID,
            ..Default::default()
        };
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &mut data);
            let _ = DestroyWindow(window);
        }
    }

    pub fn show_tray_notification(window: isize, title: &str, message: &str) {
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: HWND(window as *mut c_void),
            uID: TRAY_ICON_ID,
            uFlags: NIF_INFO,
            szInfoTitle: wide_array(title),
            szInfo: wide_array(message),
            dwInfoFlags: NIIF_INFO,
            ..Default::default()
        };
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &mut data);
        }
    }

    pub fn flash_app_window() {
        let title: Vec<u16> = APP_TITLE.encode_utf16().chain([0]).collect();
        let window = unsafe { FindWindowW(None, PCWSTR(title.as_ptr())) }.unwrap_or_default();
        if window.0.is_null() {
            return;
        }
        let info = FLASHWINFO {
            cbSize: size_of::<FLASHWINFO>() as u32,
            hwnd: window,
            dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
            uCount: 0,
            dwTimeout: 0,
        };
        unsafe {
            let _ = FlashWindowEx(&info);
        }
    }

    fn wide_array<const N: usize>(text: &str) -> [u16; N] {
        let mut output = [0_u16; N];
        for (slot, unit) in output.iter_mut().zip(text.encode_utf16().take(N - 1)) {
            *slot = unit;
        }
        output
    }

    fn load_tray_icon() -> windows::Win32::UI::WindowsAndMessaging::HICON {
        let mut path = vec![0_u16; 260];
        let length = unsafe { GetModuleFileNameW(None, &mut path) };
        if length > 0 {
            let icon = unsafe { ExtractIconW(None, PCWSTR(path.as_ptr()), 0) };
            // ExtractIconW returns 1 when the file has no icon resource.
            if !icon.is_invalid() && icon.0 as usize > 1 {
                return icon;
            }
        }
        unsafe { LoadIconW(None, IDI_APPLICATION) }.unwrap_or_default()
    }

    unsafe extern "system" fn tray_window_proc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_CREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                let state = unsafe { (*create).lpCreateParams };
                unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, state as isize) };
                LRESULT(0)
            }
            WM_TRAY_CALLBACK => {
                match lparam.0 as u32 {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK => send_tray_event(window, TrayEvent::Show),
                    WM_RBUTTONUP => unsafe { show_tray_menu(window) },
                    _ => {}
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                let state =
                    unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *mut TrayEventSender;
                if !state.is_null() {
                    drop(unsafe { Box::from_raw(state) });
                }
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
        }
    }

    fn send_tray_event(window: HWND, event: TrayEvent) {
        let state = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const TrayEventSender;
        if !state.is_null() {
            let _ = unsafe { &*state }.send(event);
        }
    }

    unsafe fn show_tray_menu(window: HWND) {
        let menu = match unsafe { CreatePopupMenu() } {
            Ok(menu) => menu,
            Err(_) => return,
        };
        let show_text: Vec<u16> = "显示窗口\0".encode_utf16().collect();
        let exit_text: Vec<u16> = "退出\0".encode_utf16().collect();
        unsafe {
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                TRAY_MENU_SHOW as usize,
                PCWSTR(show_text.as_ptr()),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                TRAY_MENU_EXIT as usize,
                PCWSTR(exit_text.as_ptr()),
            );
            // Required so the menu dismisses when clicking elsewhere.
            let _ = SetForegroundWindow(window);
            let mut point = POINT::default();
            let _ = GetCursorPos(&mut point);
            let command = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                None,
                window,
                None,
            );
            let _ = PostMessageW(Some(window), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(menu);
            match command.0 as u32 {
                TRAY_MENU_SHOW => send_tray_event(window, TrayEvent::Show),
                TRAY_MENU_EXIT => send_tray_event(window, TrayEvent::Exit),
                _ => {}
            }
        }
    }

    pub fn open_image_file_dialog() -> Result<Option<std::path::PathBuf>, PlatformError> {
        let filter: Vec<u16> = "图片文件\0*.png;*.jpg;*.jpeg\0所有文件\0*.*\0\0"
            .encode_utf16()
            .collect();
        let title: Vec<u16> = "导入截图\0".encode_utf16().collect();
        let mut file_buffer = vec![0_u16; 32_768];
        let mut dialog = OPENFILENAMEW {
            lStructSize: size_of::<OPENFILENAMEW>() as u32,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrFile: PWSTR(file_buffer.as_mut_ptr()),
            nMaxFile: file_buffer.len() as u32,
            lpstrTitle: PCWSTR(title.as_ptr()),
            Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
            ..Default::default()
        };

        if unsafe { GetOpenFileNameW(&mut dialog).as_bool() } {
            let length = file_buffer
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(file_buffer.len());
            let path = String::from_utf16_lossy(&file_buffer[..length]);
            return Ok(Some(std::path::PathBuf::from(path)));
        }

        let error = unsafe { CommDlgExtendedError() };
        if error.0 == 0 {
            Ok(None)
        } else {
            Err(PlatformError::WindowsApi(format!(
                "文件选择器错误代码 {}",
                error.0
            )))
        }
    }

    pub fn open_workflow_package_dialog() -> Result<Option<std::path::PathBuf>, PlatformError> {
        let filter: Vec<u16> = "Make 5771 流程包\0*.m5771pack\0所有文件\0*.*\0\0"
            .encode_utf16()
            .collect();
        let title: Vec<u16> = "导入流程分享包\0".encode_utf16().collect();
        let mut file_buffer = vec![0_u16; 32_768];
        let mut dialog = OPENFILENAMEW {
            lStructSize: size_of::<OPENFILENAMEW>() as u32,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrFile: PWSTR(file_buffer.as_mut_ptr()),
            nMaxFile: file_buffer.len() as u32,
            lpstrTitle: PCWSTR(title.as_ptr()),
            Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
            ..Default::default()
        };
        run_open_dialog(&mut dialog, &file_buffer)
    }

    pub fn save_workflow_package_dialog(
        suggested_name: &str,
    ) -> Result<Option<std::path::PathBuf>, PlatformError> {
        let filter: Vec<u16> = "Make 5771 流程包\0*.m5771pack\0所有文件\0*.*\0\0"
            .encode_utf16()
            .collect();
        let title: Vec<u16> = "导出流程分享包\0".encode_utf16().collect();
        let default_extension: Vec<u16> = "m5771pack\0".encode_utf16().collect();
        let initial = format!("{suggested_name}.m5771pack");
        let mut file_buffer = vec![0_u16; 32_768];
        for (destination, source) in file_buffer.iter_mut().zip(initial.encode_utf16()) {
            *destination = source;
        }
        let mut dialog = OPENFILENAMEW {
            lStructSize: size_of::<OPENFILENAMEW>() as u32,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrFile: PWSTR(file_buffer.as_mut_ptr()),
            nMaxFile: file_buffer.len() as u32,
            lpstrTitle: PCWSTR(title.as_ptr()),
            lpstrDefExt: PCWSTR(default_extension.as_ptr()),
            Flags: OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_OVERWRITEPROMPT,
            ..Default::default()
        };

        if unsafe { GetSaveFileNameW(&mut dialog).as_bool() } {
            return Ok(Some(path_from_dialog_buffer(&file_buffer)));
        }
        dialog_cancel_or_error("保存文件选择器")
    }

    fn run_open_dialog(
        dialog: &mut OPENFILENAMEW,
        file_buffer: &[u16],
    ) -> Result<Option<std::path::PathBuf>, PlatformError> {
        if unsafe { GetOpenFileNameW(dialog).as_bool() } {
            return Ok(Some(path_from_dialog_buffer(file_buffer)));
        }
        dialog_cancel_or_error("打开文件选择器")
    }

    fn path_from_dialog_buffer(file_buffer: &[u16]) -> std::path::PathBuf {
        let length = file_buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(file_buffer.len());
        std::path::PathBuf::from(String::from_utf16_lossy(&file_buffer[..length]))
    }

    fn dialog_cancel_or_error(
        dialog_name: &str,
    ) -> Result<Option<std::path::PathBuf>, PlatformError> {
        let error = unsafe { CommDlgExtendedError() };
        if error.0 == 0 {
            Ok(None)
        } else {
            Err(PlatformError::WindowsApi(format!(
                "{dialog_name}错误代码 {}",
                error.0
            )))
        }
    }

    pub fn find_target_window(title_fragment: &str) -> Result<TargetWindow, PlatformError> {
        let needle = title_fragment.to_lowercase();
        list_visible_windows()?
            .into_iter()
            .find(|window| window.title.to_lowercase().contains(&needle))
            .ok_or_else(|| PlatformError::WindowNotFound(title_fragment.to_owned()))
    }

    pub fn list_visible_windows() -> Result<Vec<TargetWindow>, PlatformError> {
        let mut state = WindowListState {
            entries: Vec::new(),
        };
        unsafe {
            EnumWindows(
                Some(collect_window),
                LPARAM((&mut state as *mut WindowListState) as isize),
            )
            .map_err(|error| PlatformError::WindowsApi(error.to_string()))?;
        }

        let mut windows = Vec::new();
        for (window, title) in state.entries {
            let mut rect = RECT::default();
            if unsafe { GetClientRect(window, &mut rect) }.is_err() {
                continue;
            }
            let width = (rect.right - rect.left).max(0) as u32;
            let height = (rect.bottom - rect.top).max(0) as u32;
            if width > 0 && height > 0 {
                windows.push(TargetWindow {
                    handle: window.0 as isize,
                    title,
                    client_width: width,
                    client_height: height,
                });
            }
        }
        windows.sort_by_key(|window| window.title.to_lowercase());
        Ok(windows)
    }

    pub fn is_foreground(target: &TargetWindow) -> bool {
        unsafe { GetForegroundWindow().0 as isize == target.handle }
    }

    pub fn focus_target(target: &TargetWindow) -> Result<(), PlatformError> {
        let window = HWND(target.handle as *mut c_void);
        if unsafe { SetForegroundWindow(window).as_bool() } {
            Ok(())
        } else {
            Err(PlatformError::WindowsApi(
                "无法将游戏窗口切换到前台".to_owned(),
            ))
        }
    }

    pub fn is_window_alive(target: &TargetWindow) -> bool {
        unsafe { IsWindow(Some(HWND(target.handle as *mut c_void))).as_bool() }
    }

    pub fn click_client_background(
        target: &TargetWindow,
        x: u32,
        y: u32,
    ) -> Result<(), PlatformError> {
        let window = HWND(target.handle as *mut c_void);
        if !is_window_alive(target) {
            return Err(PlatformError::WindowsApi("目标窗口已不存在".to_owned()));
        }
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(window, &mut rect)
                .map_err(|error| PlatformError::WindowsApi(error.to_string()))?;
        }
        let width = (rect.right - rect.left).max(0) as u32;
        let height = (rect.bottom - rect.top).max(0) as u32;
        if x >= width || y >= height {
            return Err(PlatformError::WindowsApi(format!(
                "点击坐标 ({x}, {y}) 超出客户区 {width} × {height}"
            )));
        }

        let lparam = LPARAM((((y as i32) << 16) | ((x as i32) & 0xFFFF)) as isize);
        unsafe {
            // wParam = MK_LBUTTON (1) signals the left button being held.
            PostMessageW(Some(window), WM_LBUTTONDOWN, WPARAM(1), lparam)
                .map_err(|error| PlatformError::WindowsApi(error.to_string()))?;
            PostMessageW(Some(window), WM_LBUTTONUP, WPARAM(0), lparam)
                .map_err(|error| PlatformError::WindowsApi(error.to_string()))?;
        }
        Ok(())
    }

    pub fn click_client(target: &TargetWindow, x: u32, y: u32) -> Result<(), PlatformError> {
        if !is_foreground(target) {
            return Err(PlatformError::WindowsApi(
                "点击已取消：游戏不在前台".to_owned(),
            ));
        }
        let window = HWND(target.handle as *mut c_void);
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(window, &mut rect)
                .map_err(|error| PlatformError::WindowsApi(error.to_string()))?;
        }
        let width = (rect.right - rect.left).max(0) as u32;
        let height = (rect.bottom - rect.top).max(0) as u32;
        if x >= width || y >= height {
            return Err(PlatformError::WindowsApi(format!(
                "点击坐标 ({x}, {y}) 超出客户区 {width} × {height}"
            )));
        }

        let mut point = POINT {
            x: x as i32,
            y: y as i32,
        };
        if !unsafe { ClientToScreen(window, &mut point).as_bool() } {
            return Err(PlatformError::WindowsApi("无法换算点击位置".to_owned()));
        }
        unsafe {
            SetCursorPos(point.x, point.y)
                .map_err(|error| PlatformError::WindowsApi(error.to_string()))?;
        }
        let inputs = [
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dwFlags: MOUSEEVENTF_LEFTDOWN,
                        ..Default::default()
                    },
                },
            },
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dwFlags: MOUSEEVENTF_LEFTUP,
                        ..Default::default()
                    },
                },
            },
        ];
        let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if sent != inputs.len() as u32 {
            return Err(PlatformError::WindowsApi(
                "系统未接受完整的鼠标点击事件".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn capture_client(target: &TargetWindow) -> Result<RgbaImage, PlatformError> {
        let window = HWND(target.handle as *mut c_void);
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(window, &mut rect)
                .map_err(|error| PlatformError::WindowsApi(error.to_string()))?;
        }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            return Err(PlatformError::WindowsApi(
                "目标窗口客户区尺寸无效".to_owned(),
            ));
        }

        let mut origin = POINT::default();
        if !unsafe { ClientToScreen(window, &mut origin).as_bool() } {
            return Err(PlatformError::WindowsApi(
                "无法取得客户区屏幕坐标".to_owned(),
            ));
        }

        let screen_dc = unsafe { GetDC(None) };
        if screen_dc.is_invalid() {
            return Err(PlatformError::WindowsApi("无法获取屏幕 DC".to_owned()));
        }
        let memory_dc = unsafe { CreateCompatibleDC(Some(screen_dc)) };
        if memory_dc.is_invalid() {
            unsafe { ReleaseDC(None, screen_dc) };
            return Err(PlatformError::WindowsApi("无法创建内存 DC".to_owned()));
        }
        let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, width, height) };
        if bitmap.is_invalid() {
            unsafe {
                let _ = DeleteDC(memory_dc);
                ReleaseDC(None, screen_dc);
            }
            return Err(PlatformError::WindowsApi("无法创建截图位图".to_owned()));
        }

        let previous = unsafe { SelectObject(memory_dc, HGDIOBJ(bitmap.0)) };
        let copy_result = unsafe {
            BitBlt(
                memory_dc,
                0,
                0,
                width,
                height,
                Some(screen_dc),
                origin.x,
                origin.y,
                SRCCOPY,
            )
        };

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0_u8; width as usize * height as usize * 4];
        let copied_lines = if copy_result.is_ok() {
            unsafe {
                GetDIBits(
                    memory_dc,
                    bitmap,
                    0,
                    height as u32,
                    Some(pixels.as_mut_ptr().cast()),
                    &mut info,
                    DIB_RGB_COLORS,
                )
            }
        } else {
            0
        };

        unsafe {
            SelectObject(memory_dc, previous);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(memory_dc);
            ReleaseDC(None, screen_dc);
        }

        if let Err(error) = copy_result {
            return Err(PlatformError::WindowsApi(error.to_string()));
        }
        if copied_lines != height {
            return Err(PlatformError::WindowsApi("截图像素读取不完整".to_owned()));
        }

        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        ImageBuffer::from_raw(width as u32, height as u32, pixels)
            .ok_or_else(|| PlatformError::WindowsApi("无法构造截图图像".to_owned()))
    }

    unsafe extern "system" fn collect_window(window: HWND, parameter: LPARAM) -> BOOL {
        if !unsafe { IsWindowVisible(window).as_bool() } {
            return BOOL(1);
        }

        let length = unsafe { GetWindowTextLengthW(window) };
        if length <= 0 {
            return BOOL(1);
        }

        let mut buffer = vec![0_u16; length as usize + 1];
        let read = unsafe { GetWindowTextW(window, &mut buffer) };
        if read <= 0 {
            return BOOL(1);
        }

        let title = String::from_utf16_lossy(&buffer[..read as usize]);
        let state = unsafe { &mut *(parameter.0 as *mut WindowListState) };
        state.entries.push((window, title));
        BOOL(1)
    }
}
