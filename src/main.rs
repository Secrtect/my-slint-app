// 这一行保留：发布版本时隐藏 Windows 的控制台黑窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::{WinitWindowAccessor, winit};
use slint_borderless_windows::TitlebarSetup;
use std::error::Error;

slint::include_modules!();

// === Win32 FFI 安全声明块 ===
#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct POINT {
    x: i32,
    y: i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct MONITORINFO {
    cb_size: u32,
    rc_monitor: RECT,
    rc_work: RECT,
    dw_flags: u32,
}

#[cfg(target_os = "windows")]
impl Default for MONITORINFO {
    fn default() -> Self {
        Self {
            cb_size: std::mem::size_of::<MONITORINFO>() as u32,
            rc_monitor: RECT::default(),
            rc_work: RECT::default(),
            dw_flags: 0,
        }
    }
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetCursorPos(lp_point: *mut POINT) -> i32;
    fn MonitorFromPoint(pt: POINT, dw_flags: u32) -> isize;
    fn GetMonitorInfoW(h_monitor: isize, lpmi: *mut MONITORINFO) -> i32;
    fn SetWindowPos(
        h_wnd: isize,
        h_wnd_insert_after: isize,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        u_flags: u32,
    ) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "shcore")]
unsafe extern "system" {
    fn GetDpiForMonitor(
        h_monitor: isize,
        dpi_type: i32, // 0 代表 MDT_EFFECTIVE_DPI
        dpi_x: *mut u32,
        dpi_y: *mut u32,
    ) -> i32;
}

fn main() -> Result<(), Box<dyn Error>> {
    // 1. 设置 Hook，使窗口初创时保持隐藏
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .with_winit_window_attributes_hook(|attributes| attributes.with_visible(false))
        .select()?;

    // 2. 创建实例
    let app = AppWindow::new()?;

    // 3. 隐身状态下绑定无边框组件
    let frame = app.as_weak().setup_borderless().expect("无边框初始化失败");

    // 4. 核心：原子化居中并显示
    let app_weak = app.as_weak();
    slint::invoke_from_event_loop(move || {
        if let Some(app) = app_weak.upgrade() {
            app.window()
                .with_winit_window(|winit_window: &winit::window::Window| {
                    let logical_w = app.get_init_width();
                    let logical_h = app.get_init_height();

                    #[cfg(target_os = "windows")]
                    {
                        if let Ok(handle) = winit_window.window_handle() {
                            if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
                                let hwnd = win32_handle.hwnd.get() as isize;

                                unsafe {
                                    // A. 探测当前鼠标所在的显示器（符合用户在哪个屏幕点击，就在哪个屏幕居中的人机工效学）
                                    let mut cursor_pos = POINT::default();
                                    GetCursorPos(&mut cursor_pos);
                                    let h_monitor = MonitorFromPoint(cursor_pos, 2); // 2 = MONITOR_DEFAULTTONEAREST

                                    // B. 获取目标显示器的实际 DPI
                                    let mut dpi_x: u32 = 96;
                                    let mut dpi_y: u32 = 96;
                                    let _ = GetDpiForMonitor(h_monitor, 0, &mut dpi_x, &mut dpi_y);
                                    let scale_factor = dpi_x as f32 / 96.0;

                                    // C. 计算在该 DPI 下精确的物理尺寸
                                    let physical_w = (logical_w * scale_factor) as i32;
                                    let physical_h = (logical_h * scale_factor) as i32;

                                    // D. 获取目标显示器的工作区（已经由系统自动扣除了任务栏）
                                    let mut monitor_info = MONITORINFO::default();
                                    GetMonitorInfoW(h_monitor, &mut monitor_info);
                                    let work_area = monitor_info.rc_work;
                                    let work_w = work_area.right - work_area.left;
                                    let work_h = work_area.bottom - work_area.top;

                                    // E. 计算工作区内的绝对居中坐标
                                    let x = work_area.left + (work_w - physical_w) / 2;
                                    let y = work_area.top + (work_h - physical_h) / 2;

                                    // F. 原子化操作：通过 Win32 API 同步、一次性地调整大小和位置
                                    // 0x0010 = SWP_NOACTIVATE, 0x0004 = SWP_NOZORDER
                                    SetWindowPos(
                                        hwnd,
                                        0,
                                        x,
                                        y,
                                        physical_w,
                                        physical_h,
                                        0x0010 | 0x0004,
                                    );
                                }
                            }
                        }
                    }

                    #[cfg(not(target_os = "windows"))]
                    {
                        // 非 Windows 平台的退化处理（保持原 winit 逻辑）
                        let scale_factor = winit_window.scale_factor();
                        let physical_w = (logical_w * scale_factor as f32) as u32;
                        let physical_h = (logical_h * scale_factor as f32) as u32;

                        let target_size = winit::dpi::PhysicalSize::new(physical_w, physical_h);
                        let _ = winit_window.request_inner_size(target_size);

                        let monitor = winit_window
                            .current_monitor()
                            .or_else(|| winit_window.primary_monitor());
                        if let Some(m) = monitor {
                            let m_size = m.size();
                            let x = (m_size.width as i32 - physical_w as i32) / 2;
                            let y = (m_size.height as i32 - physical_h as i32) / 2;
                            winit_window
                                .set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
                        }
                    }

                    // G. 此时尺寸、位置、DPI 都已经和目标显示器完美吻合，一键显现，绝无闪烁！
                    winit_window.set_visible(true);
                });
        }
    })?;

    // 5. 绑定回调
    let frame_maximize = frame.clone();
    let frame_close = frame.clone();
    let frame_drag = frame.clone();
    let frame_dblclick = frame.clone();
    let frame_minimize = frame.clone();

    app.global::<WindowControls>().on_maximize(move || {
        frame_maximize.toggle_maximized();
    });
    app.global::<WindowControls>().on_close(move || {
        frame_close.close();
    });
    app.global::<WindowControls>().on_drag(move || {
        frame_drag.drag();
    });
    app.global::<WindowControls>().on_double_click(move || {
        frame_dblclick.toggle_maximized();
    });
    app.global::<WindowControls>().on_minimize(move || {
        frame_minimize.minimize();
    });

    app.run()?;
    Ok(())
}
