// 这一行保留：发布版本时隐藏 Windows 的控制台黑窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
// 1. 引入无边框初始化特征
mod borderless;
use borderless::TitlebarSetup;

use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETWORKAREA, SystemParametersInfoW};

// 引入自动生成的 UI 模块
slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    // 2. 创建窗口实例
    let app = AppWindow::new()?;

    // 3. 核心：启动无边框底层机制并接管 winit 句柄
    let frame = app.as_weak().setup_borderless().expect("无边框初始化失败");

    // ==================== 修复后的无闪烁居中逻辑（含防止超屏边界截断） ====================
    // 使用 SystemParametersInfoW 获取工作区（去除任务栏后的显示区域）物理坐标与尺寸
    let (work_left_phys, work_top_phys, work_w_phys, work_h_phys, real_scale) = unsafe {
        let dpi = GetDpiForSystem();
        let scale = dpi as f32 / 96.0;

        let mut work_area = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };

        // 获取主显示器扣除任务栏后的工作区 RECT
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work_area as *mut _ as *mut _, 0);

        let left = work_area.left;
        let top = work_area.top;
        let width = work_area.right - work_area.left;
        let height = work_area.bottom - work_area.top;

        (left, top, width, height, scale)
    };

    // 动态从 .slint 获取导出的逻辑宽高属性
    let init_width_logical = app.get_init_width();
    let init_height_logical = app.get_init_height();

    // 将 Windows API 获取的工作区物理坐标及尺寸转换为逻辑坐标/尺寸
    let work_left_logical = work_left_phys as f32 / real_scale;
    let work_top_logical = work_top_phys as f32 / real_scale;
    let work_w_logical = work_w_phys as f32 / real_scale;
    let work_h_logical = work_h_phys as f32 / real_scale;

    // --- 防超屏处理 ---
    // 取设置尺寸与当前屏幕工作区尺寸的最小值，防止超出屏幕导致关闭按钮等被遮挡
    let target_w_logical = init_width_logical.min(work_w_logical);
    let target_h_logical = init_height_logical.min(work_h_logical);

    // 如果超出了屏幕工作区大小，把窗口尺寸重新设置为限制后的实际逻辑尺寸
    if target_w_logical < init_width_logical || target_h_logical < init_height_logical {
        app.window()
            .set_size(slint::LogicalSize::new(target_w_logical, target_h_logical));
    }

    // 基于最终确认的逻辑尺寸重新计算居中坐标
    let target_x_logical = work_left_logical + (work_w_logical - target_w_logical) / 2.0;
    let target_y_logical = work_top_logical + (work_h_logical - target_h_logical) / 2.0;

    // 设置逻辑坐标，让 Slint 底层自动根据 DPI 换算
    app.window().set_position(slint::LogicalPosition::new(
        target_x_logical,
        target_y_logical,
    ));
    // ===========================================================================

    // ==================== 应用 Windows 11 Mica 效果 ====================
    #[cfg(target_os = "windows")]
    {
        let app_weak = app.as_weak();
        slint::invoke_from_event_loop(move || {
            if let Some(app) = app_weak.upgrade() {
                let handle = app.window().window_handle();

                if let Err(e) = window_vibrancy::apply_mica(&handle, None) {
                    println!("应用 Mica 失败: {:?}，已降级为系统自适应纯色背景", e);
                } else {
                    app.set_is_mica_active(true);
                    println!("成功应用 Mica 效果");
                }
            }
        })
        .expect("Failed to queue event loop initialization");
    }
    // ===========================================================================

    // 4. 克隆 frame 实例绑定回调
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

    // 5. 阻塞并运行
    app.run()?;

    Ok(())
}
