// 这一行保留：发布版本时隐藏 Windows 的控制台黑窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
// 1. 引入无边框初始化特征
use slint_borderless_windows::TitlebarSetup;

// 引入自动生成的 UI 模块
slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    // unsafe {
    //     std::env::set_var("SLINT_BACKEND", "winit-software");
    // }

    // 2. 创建窗口实例
    let app = AppWindow::new()?;

    // 3. 核心：启动无边框底层机制并接管 winit 句柄
    // 必须要先执行这行，让无边框库完成 Win32 样式（去边框、加阴影）的修改
    let frame = app.as_weak().setup_borderless().expect("无边框初始化失败");

    // ==================== 完美的无闪烁居中逻辑（动态属性版） ====================
    // 1. 从 Slint 获取当前的屏幕缩放因子 (DPI Scale)，确保高分屏下计算不偏差
    let scale_factor = app.window().scale_factor();

    // 2. 动态从 .slint 获取导出的逻辑宽高属性（对应 init-width 和 init-height）
    let init_width_logical = app.get_init_width();
    let init_height_logical = app.get_init_height();

    // 3. 结合 DPI 缩放换算出对应的真实物理像素尺寸
    let win_p_width = (init_width_logical * scale_factor) as i32;
    let win_p_height = (init_height_logical * scale_factor) as i32;

    // 4. 使用 windows-sys 直接获取主显示器的物理分辨率并计算居中坐标
    let (target_x, target_y) = unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
        };
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

        let x = (screen_width - win_p_width) / 2;
        let y = (screen_height - win_p_height) / 2;
        (x, y)
    };

    // 5. 将计算好的物理坐标设置给窗口
    app.window()
        .set_position(slint::PhysicalPosition::new(target_x, target_y));
    // ===========================================================================

    // ==================== 新增：应用 Windows 11 Mica 效果 ====================
    #[cfg(target_os = "windows")]
    {
        let app_weak = app.as_weak();
        slint::invoke_from_event_loop(move || {
            if let Some(app) = app_weak.upgrade() {
                let handle = app.window().window_handle();
                let res = window_vibrancy::apply_mica(&handle, Some(true));
                println!("apply_mica result (event loop): {:?}", res);
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
