// 这一行保留：发布版本时隐藏 Windows 的控制台黑窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
// 1. 引入无边框初始化特征
use slint_borderless_windows::TitlebarSetup;
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

// 引入自动生成的 UI 模块
slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    unsafe {
        std::env::set_var("SLINT_BACKEND", "winit-software");
    }
    // ========== 强制 wgpu 使用 Vulkan 后端 ==========
    // unsafe {
    //     std::env::set_var("WGPU_BACKEND", "vulkan");
    // }
    // // ==============================================

    // 2. 创建窗口实例
    let app = AppWindow::new()?;

    // 3. 核心：启动无边框底层机制并接管 winit 句柄
    // 必须要先执行这行，让无边框库完成 Win32 样式（去边框、加阴影）的修改
    let frame = app.as_weak().setup_borderless().expect("无边框初始化失败");

    // ==================== 修复后的无闪烁居中逻辑（逻辑坐标版） ====================
    // 1. 使用 windows-sys 获取系统真实 DPI 并换算出缩放比例，绕过 Slint 初始化的 1.0 陷阱
    // (注意：需要确保依赖中开启了 windows-sys 的 "Win32_UI_HiDpi" 和 "Win32_UI_WindowsAndMessaging" 特性)
    let (screen_width_phys, screen_height_phys, real_scale) = unsafe {
        let dpi = GetDpiForSystem();
        let scale = dpi as f32 / 96.0;

        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        (w, h, scale)
    };

    // 2. 动态从 .slint 获取导出的逻辑宽高属性
    let init_width_logical = app.get_init_width();
    let init_height_logical = app.get_init_height();

    // 3. 将 Windows API 获取的物理屏幕尺寸转换为逻辑尺寸
    let screen_w_logical = screen_width_phys as f32 / real_scale;
    let screen_h_logical = screen_height_phys as f32 / real_scale;

    // 4. 在逻辑坐标系下计算居中坐标
    let target_x_logical = (screen_w_logical - init_width_logical) / 2.0;
    let target_y_logical = (screen_h_logical - init_height_logical) / 2.0;

    // 5. 使用 LogicalPosition 传递给 Slint，底层会自动根据最终的 DPI 换算物理坐标，完美对齐
    app.window().set_position(slint::LogicalPosition::new(
        target_x_logical,
        target_y_logical,
    ));
    // ===========================================================================

    // ==================== 新增：应用 Windows 11 Mica 效果 ====================
    #[cfg(target_os = "windows")]
    {
        let app_weak = app.as_weak();
        slint::invoke_from_event_loop(move || {
            if let Some(app) = app_weak.upgrade() {
                let handle = app.window().window_handle();

                if let Err(e) = window_vibrancy::apply_mica(&handle, None) {
                    // 失败了：保持默认，Slint 会使用 Palette.background 渲染纯色
                    println!("应用 Mica 失败: {:?}，已降级为系统自适应纯色背景", e);
                } else {
                    // 成功了：通知 Slint 把背景切成 transparent 透出云母
                    app.set_is_mica_active(true);
                    println!("成功应用 Mica 效果");
                }
            }
        })
        .expect("Failed to queue event loop initialization");
    }
    // ===========================================================================

    // 4. 克隆 frame 实例，分别绑定给标题栏控制器的各个回调
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
        let _ = frame_drag.drag();
    });

    app.global::<WindowControls>().on_double_click(move || {
        frame_dblclick.toggle_maximized();
    });

    app.global::<WindowControls>().on_minimize(move || {
        frame_minimize.minimize();
    });

    // 5. 阻塞并运行（此时窗口直接在中央完美呈现，无任何坐标跳跃闪烁）
    app.run()?;

    Ok(())
}
