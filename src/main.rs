// 这一行保留：发布版本时隐藏 Windows 的控制台黑窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
// 1. 引入无边框初始化特征
use slint_borderless_windows::TitlebarSetup;

// 引入 winit 访问器和底层 winit 模块
use slint::winit_030::{WinitWindowAccessor, winit};

// 引入自动生成的 UI 模块
slint::include_modules!();

// === 开启安全声明块 ===
#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    // 获取系统指标（如屏幕宽高）的 Win32 函数
    fn GetSystemMetrics(nIndex: i32) -> i32;
}

fn main() -> Result<(), Box<dyn Error>> {
    // 1. 使用 BackendSelector 安全地替代原先的 unsafe 环境变量设置
    // 同时通过 Hook 在操作系统创建窗口的瞬间强制使其“不可见”（Visible = false）
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into()) // 启用软件渲染
        .with_winit_window_attributes_hook(|attributes| {
            attributes.with_visible(false) // 阻止窗口在默认位置瞬间闪烁，保持隐藏直到我们手动将其居中并显示
        })
        .select()?;

    // 2. 创建窗口实例（此时底层窗口已创建，但处于隐身状态）
    let app = AppWindow::new()?;

    // 3. 启动无边框底层机制并接管 winit 句柄
    // 在隐身状态下绑定无边框 subclass，能完美避免无边框窗口初始化时特有的白色边框瞬间闪烁
    let frame = app.as_weak().setup_borderless().expect("无边框初始化失败");

    // 4. 核心优化：利用事件循环队列，在首帧布局完成后再执行居中并显示。
    // 这能彻底防止我们在 main 阶段设置的尺寸和位置被 Slint 随后的默认启动逻辑覆盖。
    let app_weak = app.as_weak();
    slint::invoke_from_event_loop(move || {
        if let Some(app) = app_weak.upgrade() {
            app.window()
                .with_winit_window(|winit_window: &winit::window::Window| {
                    let scale_factor = winit_window.scale_factor(); // 获取真实的 DPI 缩放比例

                    // 读取 Slint 导出的偏好尺寸（逻辑像素），转换为物理像素
                    let logical_w = app.get_init_width();
                    let logical_h = app.get_init_height();

                    let physical_w = (logical_w * scale_factor as f32) as u32;
                    let physical_h = (logical_h * scale_factor as f32) as u32;

                    // 强制让 winit 采用这个大小
                    let target_size = winit::dpi::PhysicalSize::new(physical_w, physical_h);
                    let _ = winit_window.request_inner_size(target_size);

                    // === 使用条件编译变量声明，完美干掉 unused_assignments 警告 ===
                    let (x, y) = {
                        // 如果是 Windows，直接调用 API 初始化 coords，无需先设为 None
                        #[cfg(target_os = "windows")]
                        let coords = unsafe {
                            let screen_w = GetSystemMetrics(0); // 0 代表 SM_CXSCREEN
                            let screen_h = GetSystemMetrics(1); // 1 代表 SM_CYSCREEN
                            Some((
                                (screen_w - physical_w as i32) / 2,
                                (screen_h - physical_h as i32) / 2,
                            ))
                        };

                        // 如果不是 Windows，直接初始化为 None
                        #[cfg(not(target_os = "windows"))]
                        let coords: Option<(i32, i32)> = None;

                        // 统一进行回退处理
                        coords.unwrap_or_else(|| {
                            let monitor = winit_window
                                .current_monitor()
                                .or_else(|| winit_window.primary_monitor());
                            if let Some(monitor) = monitor {
                                let monitor_size = monitor.size();
                                (
                                    (monitor_size.width as i32 - physical_w as i32) / 2,
                                    (monitor_size.height as i32 - physical_h as i32) / 2,
                                )
                            } else {
                                (0, 0)
                            }
                        })
                    };

                    // 移动窗口到居中坐标
                    winit_window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));

                    // 坐标和尺寸全部就绪，完美居中显现！
                    winit_window.set_visible(true);
                });
        }
    })?;

    // 5. 克隆 frame 实例，分别绑定给标题栏控制器的各个回调
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

    // 6. 阻塞并运行
    app.run()?;

    Ok(())
}
