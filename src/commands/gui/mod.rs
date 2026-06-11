//! 图形操作界面子命令（`cct gui`）。
//!
//! 启动桌面原生窗口（eframe/egui），把 trt 参数做成表单、点击执行替代命令行
//! （参见 contracts/cli-gui.md）。仅在启用 `gui` feature 时编译。
//!
//! 子模块：
//! - [`form`]：参数表单状态与到 trt 参数的映射/校验；
//! - [`runner`]：后台线程执行 + 通道回传进度/结果；
//! - [`app`]：eframe 应用（表单渲染、状态机、结果展示）。

mod app;
mod form;
mod runner;

use crate::error::{CctError, CctResult};

use app::CctApp;

/// 启动图形界面。
///
/// 打开一个桌面窗口并进入事件循环，窗口关闭后返回。
/// 当无可用图形环境或窗口初始化失败时（如无头服务器、无 X11/Wayland 转发的远程会话），
/// 返回 [`CctError::Gui`] 并附明确中文提示，由调用方输出到 stderr 并以非零码退出（FR-015/SC-007）。
pub fn run() -> CctResult<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([580.0, 480.0])
            .with_min_inner_size([420.0, 360.0]),
        ..Default::default()
    };
    eframe::run_native(
        "自定义工具",
        options,
        Box::new(|cc| {
            setup_chinese_fonts(&cc.egui_ctx);
            Ok(Box::<CctApp>::default())
        }),
    )
    .map_err(|e| {
        CctError::Gui(format!(
            "无法启动图形界面（当前环境可能无图形桌面支持）：{e}。\n\
             请在桌面环境运行，或改用命令行：自定义工具 文本替换 -d <目录> -o <旧> -n <新>"
        ))
    })
}

/// 加载支持中文的字体，避免界面出现方块/乱码。
///
/// 加载 Windows 系统字体 Microsoft YaHei（msyh.ttc）并注册到 egui 字体族。
/// egui 查找字形时会按字体族列表顺序依次尝试，msyh 中的 CJK 字形会被自动命中。
fn setup_chinese_fonts(ctx: &eframe::egui::Context) {
    let mut fonts = eframe::egui::FontDefinitions::default();

    #[cfg(target_os = "windows")]
    {
        const YAHEI_PATH: &str = "C:\\Windows\\Fonts\\msyh.ttc";
        if let Ok(font_data) = std::fs::read(YAHEI_PATH) {
            fonts.font_data.insert(
                "msyh".to_owned(),
                std::sync::Arc::new(eframe::egui::FontData::from_owned(font_data)),
            );
            // 将中文字体插入字体族首位，egui 优先在此查找 CJK 字形。
            for family in [
                eframe::egui::FontFamily::Proportional,
                eframe::egui::FontFamily::Monospace,
            ] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .insert(0, "msyh".to_owned());
            }
        }
    }

    ctx.set_fonts(fonts);
}
