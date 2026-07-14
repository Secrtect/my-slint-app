// 这一行保留：发布版本时隐藏 Windows 的控制台黑窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
// 1. 引入无边框初始化特征
use slint_borderless_windows::TitlebarSetup;

// 引入自动生成的 UI 模块
slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    unsafe {
        std::env::set_var("SLINT_BACKEND", "winit-software");
    }
    // 2. 创建窗口实例
    let app = AppWindow::new()?;

    // 3. 核心：启动无边框底层机制并接管 winit 句柄
    let frame = app.as_weak().setup_borderless().expect("无边框初始化失败");

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
        frame_drag.drag();
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
