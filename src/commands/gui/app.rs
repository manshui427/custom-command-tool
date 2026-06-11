//! 图形界面的 eframe 应用：子命令标签栏、表单渲染、执行状态机与结果展示。
//!
//! 对应 data-model.md 实体 5 与 FR-009/FR-010/FR-012/FR-016/FR-017。
//! 即时模式：每帧根据 [`GuiExecState`] 重绘；执行中轮询后台通道推进状态。
//! 标签栏从注册表读取可用子命令，当前仅 trt 有完整表单，其余显示占位提示。

use crate::commands::trt::{ProgressUpdate, RunSummary};
use crate::registry;

use eframe::egui;

use super::form::GuiFormState;
use super::runner::{self, GuiMessage, RunHandle};

// ── 配色常量 ──────────────────────────────────────────────────────────

const ACCENT: egui::Color32 = egui::Color32::from_rgb(91, 155, 213);
const ACCENT_FILL: egui::Color32 = egui::Color32::from_rgb(70, 130, 180);
const ORANGE: egui::Color32 = egui::Color32::from_rgb(237, 162, 32);
const SUCCESS_GREEN: egui::Color32 = egui::Color32::from_rgb(80, 200, 120);
const TAB_ACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(50, 60, 75);
const TAB_INACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(30, 35, 45);

// ── 子命令标签 ────────────────────────────────────────────────────────

/// GUI 中可选的子命令面板。
///
/// 从注册表动态获取。
/// 新增子命令只需在 registry 中注册并在此枚举中加一项+对应渲染。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    /// 文本替换工具（trt）。
    Trt,
    /// 尚未实现 GUI 表单的子命令占位。
    Placeholder { alias: &'static str, description: &'static str },
}

/// 构建标签列表：从注册表读取。
fn build_tabs() -> Vec<ActiveTab> {
    registry::SUBCOMMANDS
        .iter()
        .map(|sc| {
            if sc.alias == "trt" {
                ActiveTab::Trt
            } else {
                ActiveTab::Placeholder {
                    alias: sc.alias,
                    description: sc.description,
                }
            }
        })
        .collect()
}

impl ActiveTab {
    /// 标签显示文本（别名）。
    fn label(&self) -> &'static str {
        match self {
            ActiveTab::Trt => "文本替换",
            ActiveTab::Placeholder { alias, .. } => alias,
        }
    }

    /// 标签提示文本（描述）。
    fn tooltip(&self) -> &'static str {
        match self {
            ActiveTab::Trt => "文本替换",
            ActiveTab::Placeholder { description, .. } => description,
        }
    }
}

// ── 状态机 ────────────────────────────────────────────────────────────

/// 图形界面执行状态机（参见 data-model.md 实体 5）。
enum GuiExecState {
    /// 空闲：可编辑表单、可点击执行。
    Idle,
    /// 撤销前等待确认（FR-016）。
    Confirming,
    /// 后台执行中，显示进度，禁用执行按钮（防重入）。
    Running {
        progress: ProgressUpdate,
        handle: RunHandle,
    },
    /// 完成：展示统计 + 被影响文件列表。
    Done(RunSummary),
    /// 失败：展示错误信息。
    Failed(String),
}

/// cct 图形操作界面应用。
pub struct CctApp {
    /// 当前选中的子命令标签。
    active_tab: ActiveTab,
    /// 所有可用标签（从注册表构建）。
    tabs: Vec<ActiveTab>,
    /// trt 参数表单状态。
    form: GuiFormState,
    /// 当前执行状态。
    state: GuiExecState,
}

impl Default for CctApp {
    fn default() -> Self {
        let tabs = build_tabs();
        Self {
            active_tab: tabs.first().copied().unwrap_or(ActiveTab::Trt),
            tabs,
            form: GuiFormState::default(),
            state: GuiExecState::Idle,
        }
    }
}

impl CctApp {
    // ── 状态推进 ──────────────────────────────────────────────────

    fn launch(&mut self) {
        let handle = runner::start(&self.form);
        self.state = GuiExecState::Running {
            progress: ProgressUpdate::default(),
            handle,
        };
    }

    fn on_execute_clicked(&mut self) {
        if self.form.undo {
            self.state = GuiExecState::Confirming;
        } else {
            self.launch();
        }
    }

    fn poll_running(&mut self, ctx: &egui::Context) {
        let mut next: Option<GuiExecState> = None;
        if let GuiExecState::Running { progress, handle } = &mut self.state {
            while let Ok(msg) = handle.receiver.try_recv() {
                match msg {
                    GuiMessage::Progress(u) => *progress = u,
                    GuiMessage::Finished(summary) => {
                        next = Some(GuiExecState::Done(summary));
                        break;
                    }
                    GuiMessage::Error(e) => {
                        next = Some(GuiExecState::Failed(e));
                        break;
                    }
                }
            }
            ctx.request_repaint();
        }
        if let Some(s) = next {
            self.state = s;
        }
    }

    // ── 配色 ──────────────────────────────────────────────────────

    fn configure_visuals(ctx: &egui::Context) {
        let mut style = (*ctx.global_style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        ctx.set_global_style(style);
    }

    // ── 标签栏 ────────────────────────────────────────────────────

    fn ui_tab_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for tab in self.tabs.iter() {
                let is_active = *tab == self.active_tab;
                let bg = if is_active { TAB_ACTIVE_BG } else { TAB_INACTIVE_BG };
                let text_color = if is_active { ACCENT } else { egui::Color32::GRAY };
                let btn = egui::Button::new(
                    egui::RichText::new(tab.label()).color(text_color).strong(),
                )
                .fill(bg)
                .min_size(egui::vec2(80.0, 28.0))
                .corner_radius(4.0);
                let resp = ui.add(btn).on_hover_text(tab.tooltip());
                if resp.clicked() && !is_active {
                    self.active_tab = *tab;
                    self.state = GuiExecState::Idle;
                }
            }
        });
    }

    // ── trt 表单 ──────────────────────────────────────────────────

    fn ui_trt_panel(&mut self, ui: &mut egui::Ui, editable: bool) {
        ui.add_enabled_ui(editable, |ui| {
            egui::Grid::new("trt_form")
                .num_columns(2)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("目标目录").strong());
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.form.directory)
                                .desired_width(260.0)
                                .hint_text("选择或输入目录路径…"),
                        );
                        if ui.small_button("选择…").clicked()
                            && let Some(dir) = rfd::FileDialog::new().pick_folder()
                        {
                            self.form.directory = dir.display().to_string();
                        }
                    });
                    ui.end_row();

                    ui.label(egui::RichText::new("旧文本").strong());
                    ui.add(
                        egui::TextEdit::singleline(&mut self.form.old_text)
                            .desired_width(260.0)
                            .hint_text("要被替换的内容…"),
                    );
                    ui.end_row();

                    ui.label(egui::RichText::new("新文本").strong());
                    ui.add(
                        egui::TextEdit::singleline(&mut self.form.new_text)
                            .desired_width(260.0)
                            .hint_text("替换后的内容…"),
                    );
                    ui.end_row();

                    ui.label(egui::RichText::new("规则文件").strong());
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.form.rules_file)
                                .desired_width(260.0)
                                .hint_text("可选，:$#split#$: 分隔的规则文件…"),
                        );
                        if ui.small_button("选择…").clicked()
                            && let Some(f) = rfd::FileDialog::new().pick_file()
                        {
                            self.form.rules_file = f.display().to_string();
                        }
                    });
                    ui.end_row();

                    ui.label(egui::RichText::new("选项").strong());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 16.0;
                        ui.checkbox(&mut self.form.backup, "备份");
                        ui.checkbox(&mut self.form.case_sensitive, "大小写敏感");
                        ui.checkbox(&mut self.form.use_regex, "正则");
                        ui.checkbox(&mut self.form.undo, "撤销");
                    });
                    ui.end_row();
                });
        });
    }

    // ── 占位面板 ──────────────────────────────────────────────────

    fn ui_placeholder_panel(&self, ui: &mut egui::Ui, alias: &str, description: &str) {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(
                egui::RichText::new(format!("{alias} — {description}"))
                    .strong()
                    .color(ACCENT),
            );
            ui.add_space(12.0);
            ui.label(egui::RichText::new("图形界面支持开发中，请暂用命令行：").color(egui::Color32::GRAY));
            ui.label(egui::RichText::new(format!("自定义工具 {alias} …")).color(egui::Color32::LIGHT_GRAY));
        });
    }

    // ── 结果 ──────────────────────────────────────────────────────

    fn ui_result(&self, ui: &mut egui::Ui, summary: &RunSummary) {
        if self.form.undo {
            ui.label(
                egui::RichText::new(format!("已撤销：还原 {} 个文件。", summary.files_modified))
                    .strong()
                    .color(SUCCESS_GREEN),
            );
            return;
        }

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{}", summary.files_modified))
                    .strong()
                    .color(ACCENT),
            );
            ui.label(" 个文件被影响，共替换 ");
            ui.label(
                egui::RichText::new(format!("{}", summary.total_replacements))
                    .strong()
                    .color(ACCENT),
            );
            ui.label(" 处");
        });

        ui.horizontal(|ui| {
            ui.label("扫描");
            ui.label(egui::RichText::new(format!("{}", summary.files_scanned)).color(ACCENT));
            ui.label("，跳过二进制");
            ui.label(
                egui::RichText::new(format!("{}", summary.files_skipped_binary)).color(ACCENT),
            );
            ui.label("，失败");
            ui.label(egui::RichText::new(format!("{}", summary.files_failed)).color(ACCENT));
        });

        if let Some(p) = &summary.backup_path {
            ui.label(egui::RichText::new(format!("备份已生成：{}", p.display())).color(SUCCESS_GREEN));
        }

        ui.separator();
        ui.label(egui::RichText::new("被影响文件：").strong());

        const MAX_SHOWN: usize = 1000;
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                for f in summary.affected_files.iter().take(MAX_SHOWN) {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("•").color(ACCENT));
                        ui.label(f.path.display().to_string());
                        ui.label(
                            egui::RichText::new(format!("{} 处", f.replacements)).color(ACCENT),
                        );
                    });
                }
                if summary.affected_files.len() > MAX_SHOWN {
                    ui.label(format!(
                        "… 其余 {} 项未显示",
                        summary.affected_files.len() - MAX_SHOWN
                    ));
                }
            });
    }
}

// ── eframe App 实现 ───────────────────────────────────────────────────

impl eframe::App for CctApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        CctApp::configure_visuals(&ctx);
        self.poll_running(&ctx);

        // ── 标题 ──
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.heading(egui::RichText::new("自定义工具").color(ACCENT).strong());
        });
        ui.add_space(4.0);

        // ── 标签栏 ──
        self.ui_tab_bar(ui);
        ui.separator();
        ui.add_space(6.0);

        // ── 子命令面板 ──
        let editable = !matches!(self.state, GuiExecState::Running { .. });
        match self.active_tab {
            ActiveTab::Trt => self.ui_trt_panel(ui, editable),
            ActiveTab::Placeholder { alias, description } => {
                self.ui_placeholder_panel(ui, alias, description);
            }
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(8.0);

        // ── 操作区（仅 trt 标签显示） ──
        if self.active_tab == ActiveTab::Trt {
            let mut execute_clicked = false;
            match &self.state {
                GuiExecState::Idle | GuiExecState::Done(_) | GuiExecState::Failed(_) => {
                    ui.vertical_centered(|ui| {
                        let btn_text = if self.form.undo { "撤销" } else { "执行" };
                        let btn_color = if self.form.undo { ORANGE } else { ACCENT_FILL };
                        let btn = egui::Button::new(
                            egui::RichText::new(btn_text).strong().color(egui::Color32::WHITE),
                        )
                        .fill(btn_color)
                        .min_size(egui::vec2(120.0, 36.0))
                        .corner_radius(6.0);
                        if ui.add(btn).clicked() {
                            execute_clicked = true;
                        }
                    });
                }
                GuiExecState::Running { progress, .. } => {
                    let p = *progress;
                    ui.vertical_centered(|ui| {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(egui::RichText::new("执行中…").color(ACCENT));
                        });
                        let ratio = if p.scanned > 0 {
                            p.modified as f32 / p.scanned as f32
                        } else {
                            0.0
                        };
                        ui.add(
                            egui::ProgressBar::new(ratio)
                                .desired_width(320.0)
                                .show_percentage()
                                .text(format!("已扫描 {}，已修改 {}", p.scanned, p.modified)),
                        );
                    });
                }
                GuiExecState::Confirming => {}
            }
            if execute_clicked {
                self.on_execute_clicked();
            }

            ui.add_space(8.0);

            // ── 撤销确认对话框 ──
            if matches!(self.state, GuiExecState::Confirming) {
                let mut confirmed = false;
                let mut cancelled = false;
                egui::Window::new("确认撤销")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(&ctx, |ui| {
                        ui.add_space(8.0);
                        ui.label("撤销将还原目标目录的上一次替换并删除对应备份，确定继续？");
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let confirm_btn = egui::Button::new(
                                egui::RichText::new("确认撤销").strong().color(egui::Color32::WHITE),
                            )
                            .fill(ORANGE)
                            .min_size(egui::vec2(100.0, 30.0))
                            .corner_radius(6.0);
                            if ui.add(confirm_btn).clicked() {
                                confirmed = true;
                            }
                            ui.add_space(16.0);
                            if ui.button("取消").clicked() {
                                cancelled = true;
                            }
                        });
                    });
                if confirmed {
                    self.launch();
                } else if cancelled {
                    self.state = GuiExecState::Idle;
                }
            }

            // ── 结果 / 错误 ──
            match &self.state {
                GuiExecState::Done(summary) => {
                    ui.separator();
                    ui.add_space(6.0);
                    self.ui_result(ui, summary);
                }
                GuiExecState::Failed(err) => {
                    ui.separator();
                    ui.add_space(6.0);
                    ui.colored_label(
                        egui::Color32::from_rgb(230, 60, 60),
                        egui::RichText::new(format!("执行失败：{err}")).strong(),
                    );
                }
                _ => {}
            }
        }
    }
}