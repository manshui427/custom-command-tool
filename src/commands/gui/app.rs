//! 图形界面的 eframe 应用：子命令标签栏、表单渲染、执行状态机与结果展示。
//!
//! 对应 data-model.md 实体 5 与 FR-009/FR-010/FR-012/FR-016/FR-017。
//! 即时模式：每帧根据 [`GuiExecState`] 重绘；执行中轮询后台通道推进状态。
//! 标签栏从注册表读取可用子命令，当前仅 trt 有完整表单，其余显示占位提示。
//!
//! 视觉风格：参照浅色卡片式桌面应用——浅灰页面背景 + 白色圆角卡片 + 描边按钮。

use crate::commands::trt::{ProgressUpdate, RunSummary, SearchResult};
use crate::registry;

use eframe::egui;

use super::form::GuiFormState;
use super::runner::{self, GuiMessage, RunHandle};

// ── 配色常量（浅色卡片主题）─────────────────────────────────────────────

/// 页面背景：浅灰 #f5f5f5。
const PAGE_BG: egui::Color32 = egui::Color32::from_rgb(245, 245, 245);
/// 卡片/表面背景：纯白。
const CARD_BG: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);
/// 描边/分隔线：#d0d0d0。
const BORDER: egui::Color32 = egui::Color32::from_rgb(208, 208, 208);
/// 主色蓝（按钮填充、强调）：#2b6cb0。
const ACCENT: egui::Color32 = egui::Color32::from_rgb(43, 108, 176);
/// 主色蓝悬停：#357abd。
const ACCENT_HOVER: egui::Color32 = egui::Color32::from_rgb(53, 122, 189);
/// 成功绿。
const SUCCESS_GREEN: egui::Color32 = egui::Color32::from_rgb(19, 122, 56);
/// 失败红。
const DANGER: egui::Color32 = egui::Color32::from_rgb(201, 42, 42);
/// 正文深灰 #222。
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(34, 34, 34);
/// 次要灰 #888。
const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(136, 136, 136);
/// 禁用态文字/描边浅灰 #bbb。
const TEXT_DISABLED: egui::Color32 = egui::Color32::from_rgb(187, 187, 187);
/// 按钮背景 #e8e8e8。
const BTN_BG: egui::Color32 = egui::Color32::from_rgb(232, 232, 232);
/// 输入框聚焦描边 #999。
const INPUT_FOCUS: egui::Color32 = egui::Color32::from_rgb(153, 153, 153);
/// 滚动条 #ccc。
const SCROLLBAR: egui::Color32 = egui::Color32::from_rgb(204, 204, 204);

/// 卡片圆角。
const CARD_ROUNDING: f32 = 4.0;
/// 控件圆角。
const CONTROL_ROUNDING: f32 = 4.0;

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
            ActiveTab::Trt => "trt",
            ActiveTab::Placeholder { alias, .. } => alias,
        }
    }

    /// 标签提示文本（描述）。
    fn tooltip(&self) -> &'static str {
        match self {
            ActiveTab::Trt => "text-replace-tool",
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
    /// 替换/撤销完成：展示统计 + 被影响文件列表。
    Done(RunSummary),
    /// 查找完成：展示命中文件列表。
    SearchDone(SearchResult),
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
                    GuiMessage::SearchFinished(result) => {
                        next = Some(GuiExecState::SearchDone(result));
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

    /// 配置浅色主题。
    fn configure_visuals(ctx: &egui::Context) {
        let mut visuals = egui::Visuals::light();

        visuals.panel_fill = PAGE_BG;
        visuals.window_fill = CARD_BG;
        visuals.extreme_bg_color = PAGE_BG;

        visuals.widgets.noninteractive.bg_fill = CARD_BG;
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);

        let widget_stroke = egui::Stroke::new(1.0, BORDER);
        let widget_stroke_hover = egui::Stroke::new(1.0, INPUT_FOCUS);
        let widget_stroke_active = egui::Stroke::new(1.5, INPUT_FOCUS);
        for w in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            w.bg_fill = CARD_BG;
            w.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
            w.bg_stroke = widget_stroke;
            w.corner_radius = CONTROL_ROUNDING.into();
        }
        visuals.widgets.hovered.bg_stroke = widget_stroke_hover;
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
        visuals.widgets.active.bg_stroke = widget_stroke_active;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
        visuals.widgets.open.bg_stroke = widget_stroke_hover;
        visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);

        visuals.selection.bg_fill = ACCENT.linear_multiply(0.15);
        visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
        visuals.hyperlink_color = ACCENT;

        ctx.set_visuals(visuals);

        let mut style = (*ctx.global_style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 4.0);
        style.spacing.button_padding = egui::vec2(12.0, 4.0);
        style.spacing.window_margin = egui::Margin::same(0);
        ctx.set_global_style(style);
    }

    // ── 卡片容器辅助 ──────────────────────────────────────────────

    /// 白色圆角描边卡片。
    fn card(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
        egui::Frame::group(ui.style())
            .fill(CARD_BG)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .inner_margin(egui::Margin::symmetric(16, 8))
            .corner_radius(CARD_ROUNDING)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                add_contents(ui);
            });
    }

    /// 卡片小标题：小号大写灰色文本。
    fn card_title(ui: &mut egui::Ui, text: &str) {
        ui.label(
            egui::RichText::new(text.to_uppercase())
                .size(11.0)
                .strong()
                .color(TEXT_SECONDARY),
        );
        ui.add_space(6.0);
    }

    // ── 标签栏 ────────────────────────────────────────────────────

    fn ui_tab_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(CARD_BG)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
                    // 标签 Copy，克隆一份以便迭代时修改 self.active_tab。
                    let tabs = self.tabs.clone();
                    let mut clicked: Option<ActiveTab> = None;
                    for tab in tabs.iter() {
                        let is_active = *tab == self.active_tab;
                        if Self::ui_tab(ui, tab.label(), tab.tooltip(), is_active) && !is_active {
                            clicked = Some(*tab);
                        }
                    }
                    if let Some(t) = clicked {
                        self.active_tab = t;
                        self.state = GuiExecState::Idle;
                    }
                });
            });
    }

    /// 自绘单个标签：顶部圆角、激活态与下方内容融合。返回是否被点击。
    fn ui_tab(ui: &mut egui::Ui, label: &str, tooltip: &str, is_active: bool) -> bool {
        const H: f32 = 28.0;
        const PAD_X: f32 = 14.0;
        let text_color = if is_active { TEXT_PRIMARY } else { TEXT_SECONDARY };
        let galley = ui.painter().layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(12.0),
            text_color,
        );
        let text_size = galley.size();
        let (rect, resp) = ui.allocate_exact_size(
            egui::vec2(PAD_X * 2.0 + text_size.x, H),
            egui::Sense::click(),
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let bg = if is_active { PAGE_BG } else { CARD_BG };
            let stroke = if is_active {
                egui::Stroke::new(1.0, BORDER)
            } else {
                egui::Stroke::NONE
            };
            painter.rect(
                rect,
                egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 },
                bg,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.galley(
                egui::pos2(rect.left() + PAD_X, rect.center().y - text_size.y / 2.0),
                galley,
                text_color,
            );
        }

        resp.on_hover_text(tooltip).clicked()
    }

    // ── trt 表单 ──────────────────────────────────────────────────

    /// 渲染 trt 表单：单张卡片含参数区 + 选项区 + 执行按钮 + 进度/状态。
    ///
    /// 返回本帧是否点击了「执行」（由调用方在借用结束后处理，避免闭包内 `&mut self`）。
    fn ui_trt_panel(&mut self, ui: &mut egui::Ui, editable: bool) -> bool {
        // 撤销模式：清空除「目标目录」外的全部参数（下方再将其禁用）。
        if self.form.undo {
            self.form.old_text.clear();
            self.form.new_text.clear();
            self.form.rules_file.clear();
            self.form.backup = false;
            self.form.case_sensitive = false;
            self.form.use_regex = false;
            self.form.search = false;
        } else if self.form.search {
            // 查找模式：清空并禁用 备份 / 撤销 / 新文本 / 规则文件；
            // 保留旧文本（作查找文本）、大小写、正则。
            self.form.new_text.clear();
            self.form.rules_file.clear();
            self.form.backup = false;
            self.form.undo = false;
        }
        let undo = self.form.undo;
        let search = self.form.search;
        let mut execute_clicked = false;
        Self::card(ui, |ui| {
            Self::card_title(ui, "参数");

            ui.add_enabled_ui(editable, |ui| {
                // 目标目录（撤销模式下仍可编辑）
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
                    Self::label_cell(ui, "目标目录");
                    let input_w = (ui.available_width() - 74.0).max(80.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.form.directory)
                            .desired_width(input_w)
                            .hint_text("选择要处理的目录…"),
                    );
                    if ui.add(Self::picker_button("选择…")).clicked()
                        && let Some(dir) = rfd::FileDialog::new().pick_folder()
                    {
                        self.form.directory = dir.display().to_string();
                    }
                });

                // 旧文本 / 查找文本：撤销模式禁用；查找模式仍可用（作为查找文本）。
                ui.add_enabled_ui(!undo, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
                        let (label, hint) = if search {
                            ("查找文本", "要查找的文本…")
                        } else {
                            ("旧文本", "要替换或查找的文本…")
                        };
                        Self::label_cell(ui, label);
                        let input_w = (ui.available_width() - 2.0).max(80.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.form.old_text)
                                .desired_width(input_w)
                                .hint_text(hint),
                        );
                    });
                });

                // 新文本 / 规则文件：撤销或查找模式下清空并禁用。
                ui.add_enabled_ui(!undo && !search, |ui| {
                    // 新文本
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
                        Self::label_cell(ui, "新文本");
                        let input_w = (ui.available_width() - 2.0).max(80.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.form.new_text)
                                .desired_width(input_w)
                                .hint_text("替换后的文本…"),
                        );
                    });

                    // 规则文件（带「选择…」文件选择）
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
                        Self::label_cell(ui, "规则文件");
                        let input_w = (ui.available_width() - 74.0).max(80.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.form.rules_file)
                                .desired_width(input_w)
                                .hint_text("选择规则文件…"),
                        );
                        if ui.add(Self::picker_button("选择…")).clicked()
                            && let Some(f) = rfd::FileDialog::new().pick_file()
                        {
                            self.form.rules_file = f.display().to_string();
                        }
                    });
                });

                // 分隔线
                ui.add_space(6.0);
                let (line_rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 1.0),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(line_rect, 0.0, BORDER);
                ui.add_space(6.0);

                // 复选框行（自绘蓝色复选框 + mockup 中的 title 提示）
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(16.0, 6.0);
                    // 备份：撤销或查找模式禁用。
                    Self::checkbox(ui, &mut self.form.backup, "备份", !undo && !search)
                        .on_hover_text("替换前创建备份 ZIP");
                    // 大小写 / 正则：撤销模式禁用（查找模式下仍适用）。
                    Self::checkbox(ui, &mut self.form.case_sensitive, "大小写敏感", !undo)
                        .on_hover_text("区分大小写匹配");
                    Self::checkbox(ui, &mut self.form.use_regex, "正则", !undo)
                        .on_hover_text("使用正则表达式匹配");
                    // 查找模式：撤销模式禁用。
                    Self::checkbox(ui, &mut self.form.search, "查找模式", !undo)
                        .on_hover_text("仅查找包含指定文本的文件，不修改");
                    // 撤销：查找模式禁用（二者互斥）。
                    Self::checkbox(ui, &mut self.form.undo, "撤销", !search)
                        .on_hover_text("撤销最近一次替换操作");
                });

                // 执行按钮（右对齐，固定行高）。
                // 用 allocate_ui_with_layout 固定高度，避免 with_layout 撑满卡片剩余高度、
                // 把按钮垂直居中后在其上方留出大片空白。
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 28.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        // 按钮文案随模式变化：撤销 / 查找 / 执行。
                        let label = if undo {
                            "撤销"
                        } else if search {
                            "查找"
                        } else {
                            "执行"
                        };
                        if ui.add(Self::primary_button(label)).clicked() {
                            execute_clicked = true;
                        }
                    },
                );
            });

            // 进度条（执行中，置于卡片内、不随表单变灰）
            if let GuiExecState::Running { progress, .. } = &self.state {
                ui.add_space(10.0);
                let is_search = self.form.search;
                Self::ui_progress_bar(ui, progress, is_search);
            }

            // 状态消息（置于卡片内，对应 mockup .status-msg）
            match &self.state {
                GuiExecState::Done(summary) => {
                    ui.add_space(8.0);
                    if self.form.undo {
                        Self::ui_status_msg(
                            ui,
                            &format!("撤销完成，已还原 {} 个文件。", summary.files_modified),
                            StatusKind::Success,
                        );
                    } else {
                        Self::ui_status_msg(ui, "操作完成。", StatusKind::Success);
                    }
                }
                GuiExecState::SearchDone(_) => {
                    ui.add_space(8.0);
                    Self::ui_status_msg(ui, "查找完成。", StatusKind::Success);
                }
                GuiExecState::Failed(err) => {
                    ui.add_space(8.0);
                    Self::ui_status_msg(ui, err, StatusKind::Error);
                }
                _ => {}
            }
        });
        execute_clicked
    }

    /// 右对齐固定宽（90px）的字段标签单元（对应 mockup `.form-label`）。
    /// 所在行被禁用时（如撤销模式），标签随之变浅。
    fn label_cell(ui: &mut egui::Ui, text: &str) {
        let color = if ui.is_enabled() {
            TEXT_SECONDARY
        } else {
            TEXT_DISABLED
        };
        ui.allocate_ui_with_layout(
            egui::vec2(90.0, 26.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.label(egui::RichText::new(text).color(color).size(12.0));
            },
        );
    }

    /// "选择…"次要按钮。
    fn picker_button(text: &'static str) -> egui::Button<'static> {
        egui::Button::new(egui::RichText::new(text).color(TEXT_PRIMARY).size(12.0))
            .fill(BTN_BG)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .min_size(egui::vec2(64.0, 28.0))
            .corner_radius(CONTROL_ROUNDING)
    }

    /// 自绘复选框（对应 mockup `.checkbox-item`）：14px 方框，
    /// 勾选时蓝色填充 + 白色对勾，未勾选为浅底 + 描边；右侧 12px 文字。
    /// 所在作用域被禁用时变灰且不可点击（撤销模式下的其余选项）。
    fn checkbox(ui: &mut egui::Ui, checked: &mut bool, label: &str, enabled: bool) -> egui::Response {
        const BOX: f32 = 14.0;
        const GAP: f32 = 6.0;
        let enabled = enabled && ui.is_enabled();
        let text_color = if enabled { TEXT_PRIMARY } else { TEXT_DISABLED };
        let galley = ui.painter().layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(12.0),
            text_color,
        );
        let text_size = galley.size();
        let (rect, mut resp) = ui.allocate_exact_size(
            egui::vec2(BOX + GAP + text_size.x, BOX.max(text_size.y)),
            egui::Sense::click(),
        );
        if enabled && resp.clicked() {
            *checked = !*checked;
            resp.mark_changed();
        }
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let box_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.center().y - BOX / 2.0),
                egui::vec2(BOX, BOX),
            );
            // 启用与禁用两套配色。
            let (fill, border, check) = if enabled {
                (ACCENT, BORDER, egui::Color32::WHITE)
            } else {
                (
                    egui::Color32::from_rgb(200, 200, 200),
                    TEXT_DISABLED,
                    egui::Color32::from_rgb(245, 245, 245),
                )
            };
            if *checked {
                painter.rect(
                    box_rect,
                    2.0,
                    fill,
                    egui::Stroke::new(1.0, fill),
                    egui::StrokeKind::Inside,
                );
                let (l, t) = (box_rect.left(), box_rect.top());
                painter.add(egui::Shape::line(
                    vec![
                        egui::pos2(l + 3.0, t + 7.0),
                        egui::pos2(l + 6.0, t + 10.0),
                        egui::pos2(l + 11.0, t + 4.0),
                    ],
                    egui::Stroke::new(2.0, check),
                ));
            } else {
                painter.rect(
                    box_rect,
                    2.0,
                    PAGE_BG,
                    egui::Stroke::new(1.0, border),
                    egui::StrokeKind::Inside,
                );
            }
            let text_pos =
                egui::pos2(box_rect.right() + GAP, rect.center().y - text_size.y / 2.0);
            painter.galley(text_pos, galley, text_color);
        }
        if enabled {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            resp
        }
    }

    // ── 占位面板 ──────────────────────────────────────────────────

    fn ui_placeholder_panel(&self, ui: &mut egui::Ui, alias: &str, description: &str) {
        Self::card(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    egui::RichText::new(format!("{alias} — {description}"))
                        .strong()
                        .color(TEXT_PRIMARY)
                        .size(14.0),
                );
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("图形界面支持开发中，请暂用命令行：")
                        .color(TEXT_SECONDARY),
                );
                ui.label(
                    egui::RichText::new(format!("cct {alias} …"))
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(40.0);
            });
        });
    }

    // ── 操作按钮 ──────────────────────────────────────────────────

    /// 主操作按钮：填充蓝色背景 + 白色文字。
    fn primary_button(text: &str) -> egui::Button<'_> {
        egui::Button::new(
            egui::RichText::new(text).strong().color(egui::Color32::WHITE),
        )
        .fill(ACCENT)
        .stroke(egui::Stroke::new(1.0, ACCENT_HOVER))
        .min_size(egui::vec2(80.0, 28.0))
        .corner_radius(CONTROL_ROUNDING)
    }

    // ── 进度条 ────────────────────────────────────────────────────

    fn ui_progress_bar(ui: &mut egui::Ui, progress: &ProgressUpdate, is_search: bool) {
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(250, 250, 250))
            .stroke(egui::Stroke::new(1.0, BORDER))
            .corner_radius(CONTROL_ROUNDING)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(14.0));
                    let ratio = if progress.scanned > 0 {
                        progress.modified as f32 / progress.scanned as f32
                    } else {
                        0.0
                    };
                    let pct = (ratio * 100.0).round() as u32;
                    // 进度轨道
                    let track_width = ui.available_width() - 100.0;
                    let (track_rect, _) = ui.allocate_exact_size(
                        egui::vec2(track_width.max(100.0), 6.0),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter();
                    // 轨道背景
                    painter.rect_filled(track_rect, 3.0, SCROLLBAR);
                    // 填充
                    let fill_width = track_rect.width() * ratio;
                    if fill_width > 0.0 {
                        painter.rect_filled(
                            egui::Rect::from_min_size(track_rect.min, egui::vec2(fill_width, 6.0)),
                            3.0,
                            ACCENT,
                        );
                    }
                    ui.label(
                        egui::RichText::new(format!("{pct}%"))
                            .size(11.0)
                            .color(TEXT_SECONDARY),
                    );
                });
                ui.add_space(4.0);
                let progress_text = if is_search {
                    format!("已扫描 {}，已找到 {}", progress.scanned, progress.modified)
                } else {
                    format!("已扫描 {}，已修改 {}", progress.scanned, progress.modified)
                };
                ui.label(
                    egui::RichText::new(progress_text)
                        .size(11.0)
                        .color(TEXT_SECONDARY),
                );
            });
    }

    // ── 状态消息 ──────────────────────────────────────────────────

    fn ui_status_msg(ui: &mut egui::Ui, text: &str, kind: StatusKind) {
        let (bg, fg, stroke_color) = match kind {
            StatusKind::Success => (
                egui::Color32::from_rgb(232, 248, 237),
                SUCCESS_GREEN,
                egui::Color32::from_rgb(170, 225, 185),
            ),
            StatusKind::Error => (
                egui::Color32::from_rgb(255, 235, 235),
                DANGER,
                egui::Color32::from_rgb(255, 190, 190),
            ),
        };
        egui::Frame::new()
            .fill(bg)
            .stroke(egui::Stroke::new(1.0, stroke_color))
            .corner_radius(CONTROL_ROUNDING)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(text).color(fg).size(12.0));
            });
    }

    // ── 结果 ──────────────────────────────────────────────────────

    /// 始终显示的「结果」卡片（对应 mockup `section`「结果」）。
    ///
    /// 空闲/确认/执行中显示 0/0 与带表头的空表格；完成后按替换/查找/撤销填充。
    fn ui_results_card(&self, ui: &mut egui::Ui) {
        Self::card(ui, |ui| {
            Self::card_title(ui, "结果");
            match &self.state {
                // 撤销完成：仅文案，无文件表。
                GuiExecState::Done(summary) if self.form.undo => {
                    ui.label(
                        egui::RichText::new(format!(
                            "已撤销：还原 {} 个文件。",
                            summary.files_modified
                        ))
                        .color(SUCCESS_GREEN),
                    );
                }
                // 替换完成：核心统计 + 次要统计 + 备份路径 + 被影响文件表。
                GuiExecState::Done(summary) => {
                    Self::result_stats(
                        ui,
                        "被影响文件数：",
                        summary.files_modified,
                        "总替换次数：",
                        summary.total_replacements,
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "扫描 {} · 跳过二进制 {} · 失败 {}",
                            summary.files_scanned,
                            summary.files_skipped_binary,
                            summary.files_failed
                        ))
                        .color(TEXT_SECONDARY)
                        .size(11.0),
                    );
                    if let Some(p) = &summary.backup_path {
                        ui.label(
                            egui::RichText::new(format!("备份已生成：{}", p.display()))
                                .color(SUCCESS_GREEN)
                                .size(11.0),
                        );
                    }
                    ui.add_space(8.0);
                    Self::ui_file_list_table(ui, &summary.affected_files);
                }
                // 查找完成：命中统计 + 次要统计 + 命中文件表。
                GuiExecState::SearchDone(result) => {
                    Self::result_stats(
                        ui,
                        "命中文件数：",
                        result.hits.len() as u64,
                        "总匹配次数：",
                        result.total_matches,
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "扫描 {} · 跳过二进制 {} · 失败 {}",
                            result.files_scanned,
                            result.files_skipped_binary,
                            result.files_failed
                        ))
                        .color(TEXT_SECONDARY)
                        .size(11.0),
                    );
                    ui.add_space(8.0);
                    Self::ui_search_file_list_table(ui, &result.hits);
                }
                // 空闲/确认/执行中：占位 0/0 + 空表头（随查找模式切换标签）。
                _ => {
                    if self.form.search {
                        Self::result_stats(ui, "命中文件数：", 0, "总匹配次数：", 0);
                        ui.add_space(8.0);
                        Self::ui_search_file_list_table(ui, &[]);
                    } else {
                        Self::result_stats(ui, "被影响文件数：", 0, "总替换次数：", 0);
                        ui.add_space(8.0);
                        Self::ui_file_list_table(ui, &[]);
                    }
                }
            }
        });
    }

    /// 结果区两组「标签：值」统计（值加粗），组间留 20px（对应 mockup `.result-header`）。
    fn result_stats(ui: &mut egui::Ui, l1: &str, v1: u64, l2: &str, v2: u64) {
        let stat = |ui: &mut egui::Ui, label: &str, value: u64| {
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(egui::RichText::new(label).color(TEXT_SECONDARY).size(12.0));
                ui.label(
                    egui::RichText::new(value.to_string())
                        .strong()
                        .color(TEXT_PRIMARY)
                        .size(12.0),
                );
            });
        };
        ui.horizontal(|ui| {
            stat(ui, l1, v1);
            ui.add_space(20.0);
            stat(ui, l2, v2);
        });
    }

    /// 文件列表表格（替换结果）。
    fn ui_file_list_table(
        ui: &mut egui::Ui,
        files: &[crate::commands::trt::AffectedFile],
    ) {
        egui::Frame::new()
            .stroke(egui::Stroke::new(1.0, BORDER))
            .corner_radius(CONTROL_ROUNDING)
            .show(ui, |ui| {
                // 表头
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(250, 250, 250))
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("文件路径")
                                    .strong()
                                    .size(11.0)
                                    .color(TEXT_SECONDARY),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new("替换次数")
                                        .strong()
                                        .size(11.0)
                                        .color(TEXT_SECONDARY),
                                );
                            });
                        });
                    });
                // 分隔线
                egui::Frame::new().fill(BORDER).show(ui, |ui| {
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
                });
                // 表体：填满结果卡剩余高度（表为最后元素），长列表在表内滚动，整窗不滚动。
                egui::ScrollArea::vertical()
                    .max_height((ui.available_height() - 6.0).max(60.0))
                    .show(ui, |ui| {
                        const MAX_SHOWN: usize = 1000;
                        for f in files.iter().take(MAX_SHOWN) {
                            egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(12, 5))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(f.path.display().to_string())
                                                .size(12.0)
                                                .color(TEXT_PRIMARY),
                                        )
                                        .on_hover_text(f.path.display().to_string());
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "{}",
                                                        f.replacements
                                                    ))
                                                    .size(12.0)
                                                    .color(TEXT_SECONDARY),
                                                );
                                            },
                                        );
                                    });
                                });
                            // 行间分隔线
                            if !std::ptr::eq(f, files.last().unwrap()) {
                                egui::Frame::new()
                                    .fill(egui::Color32::from_rgb(240, 240, 240))
                                    .show(ui, |ui| {
                                        ui.allocate_exact_size(
                                            egui::vec2(ui.available_width(), 1.0),
                                            egui::Sense::hover(),
                                        );
                                    });
                            }
                        }
                        if files.len() > MAX_SHOWN {
                            ui.label(
                                egui::RichText::new(format!(
                                    "… 其余 {} 项未显示",
                                    files.len() - MAX_SHOWN
                                ))
                                .color(TEXT_SECONDARY)
                                .size(11.0),
                            );
                        }
                    });
            });
    }

    /// 文件列表表格（查找结果）。
    fn ui_search_file_list_table(ui: &mut egui::Ui, hits: &[crate::commands::trt::SearchHit]) {
        egui::Frame::new()
            .stroke(egui::Stroke::new(1.0, BORDER))
            .corner_radius(CONTROL_ROUNDING)
            .show(ui, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(250, 250, 250))
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("文件路径")
                                    .strong()
                                    .size(11.0)
                                    .color(TEXT_SECONDARY),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new("匹配次数")
                                        .strong()
                                        .size(11.0)
                                        .color(TEXT_SECONDARY),
                                );
                            });
                        });
                    });
                egui::Frame::new().fill(BORDER).show(ui, |ui| {
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
                });
                egui::ScrollArea::vertical()
                    .max_height((ui.available_height() - 6.0).max(60.0))
                    .show(ui, |ui| {
                        const MAX_SHOWN: usize = 1000;
                        for hit in hits.iter().take(MAX_SHOWN) {
                            egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(12, 5))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(hit.path.display().to_string())
                                                .size(12.0)
                                                .color(TEXT_PRIMARY),
                                        )
                                        .on_hover_text(hit.path.display().to_string());
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "{}",
                                                        hit.match_count
                                                    ))
                                                    .size(12.0)
                                                    .color(TEXT_SECONDARY),
                                                );
                                            },
                                        );
                                    });
                                });
                            if !std::ptr::eq(hit, hits.last().unwrap()) {
                                egui::Frame::new()
                                    .fill(egui::Color32::from_rgb(240, 240, 240))
                                    .show(ui, |ui| {
                                        ui.allocate_exact_size(
                                            egui::vec2(ui.available_width(), 1.0),
                                            egui::Sense::hover(),
                                        );
                                    });
                            }
                        }
                        if hits.len() > MAX_SHOWN {
                            ui.label(
                                egui::RichText::new(format!(
                                    "… 其余 {} 项未显示",
                                    hits.len() - MAX_SHOWN
                                ))
                                .color(TEXT_SECONDARY)
                                .size(11.0),
                            );
                        }
                    });
            });
    }

    // ── 撤销确认对话框 ────────────────────────────────────────────

    /// 居中的撤销确认弹窗（FR-016）：模态遮罩 + 阴影卡片。
    /// 确认则启动撤销；取消 / 点击遮罩 / Esc 回到空闲。
    fn ui_confirm_undo(&mut self, ctx: &egui::Context) {
        const WARN: egui::Color32 = egui::Color32::from_rgb(230, 160, 30);
        let mut confirmed = false;
        let mut cancelled = false;

        let frame = egui::Frame::new()
            .fill(CARD_BG)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(20))
            .shadow(egui::Shadow {
                offset: [0, 6],
                blur: 24,
                spread: 0,
                color: egui::Color32::from_black_alpha(60),
            });

        let resp = egui::Modal::new(egui::Id::new("confirm_undo_modal"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_max_width(340.0);

                // 标题行：琥珀色警示徽标 + 粗体标题。
                ui.horizontal(|ui| {
                    let (icon_rect, _) =
                        ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                    {
                        let p = ui.painter();
                        let c = icon_rect.center();
                        p.circle_filled(c, 10.0, WARN);
                        p.text(
                            c,
                            egui::Align2::CENTER_CENTER,
                            "!",
                            egui::FontId::proportional(14.0),
                            egui::Color32::WHITE,
                        );
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("确认撤销")
                            .strong()
                            .size(15.0)
                            .color(TEXT_PRIMARY),
                    );
                });

                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(
                        "撤销将还原目标目录的上一次替换，并删除对应备份。该操作不可恢复。",
                    )
                    .color(TEXT_SECONDARY)
                    .size(12.5),
                );

                ui.add_space(18.0);

                // 按钮行：右对齐，取消（次要）+ 确认撤销（危险红）。
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 30.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        let confirm = egui::Button::new(
                            egui::RichText::new("确认撤销")
                                .strong()
                                .color(egui::Color32::WHITE),
                        )
                        .fill(DANGER)
                        .stroke(egui::Stroke::new(1.0, DANGER))
                        .min_size(egui::vec2(92.0, 30.0))
                        .corner_radius(CONTROL_ROUNDING);
                        if ui.add(confirm).clicked() {
                            confirmed = true;
                        }
                        ui.add_space(8.0);
                        let cancel = egui::Button::new(
                            egui::RichText::new("取消").color(TEXT_PRIMARY),
                        )
                        .fill(BTN_BG)
                        .stroke(egui::Stroke::new(1.0, BORDER))
                        .min_size(egui::vec2(72.0, 30.0))
                        .corner_radius(CONTROL_ROUNDING);
                        if ui.add(cancel).clicked() {
                            cancelled = true;
                        }
                    },
                );
            });

        // 点击遮罩或按 Esc 视为取消。
        if resp.should_close() {
            cancelled = true;
        }

        if confirmed {
            self.launch();
        } else if cancelled {
            self.state = GuiExecState::Idle;
        }
    }
}

/// 状态消息类型。
enum StatusKind {
    Success,
    Error,
}

// ── eframe App 实现 ───────────────────────────────────────────────────

impl eframe::App for CctApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        CctApp::configure_visuals(&ctx);
        self.poll_running(&ctx);

        // ── 标签栏 ──
        self.ui_tab_bar(ui);

        ui.add_space(6.0);

        // ── 主内容区：不滚动、紧凑布局；左右各留 12px 边距，标题/标签栏保持整宽。──
        egui::Frame::new()
            .inner_margin(egui::Margin { left: 12, right: 12, top: 0, bottom: 8 })
            .show(ui, |ui| {
                match self.active_tab {
                    ActiveTab::Trt => {
                        // 参数卡片（含进度条与状态消息），返回本帧是否点击「执行」。
                        let editable = !matches!(self.state, GuiExecState::Running { .. });
                        let execute_clicked = self.ui_trt_panel(ui, editable);

                        ui.add_space(2.0);

                        // 结果卡片：始终显示；文件表填满剩余高度，长列表在表内滚动，整窗不滚动。
                        self.ui_results_card(ui);

                        // 执行点击在卡片借用结束后处理。
                        if execute_clicked {
                            self.on_execute_clicked();
                        }

                        // 撤销前确认弹窗。
                        if matches!(self.state, GuiExecState::Confirming) {
                            self.ui_confirm_undo(&ctx);
                        }
                    }
                    ActiveTab::Placeholder { alias, description } => {
                        self.ui_placeholder_panel(ui, alias, description);
                    }
                }
            });
    }

    /// 窗口清屏色：浅灰页面背景。
    ///
    /// eframe 默认清屏色为近黑（`rgba(12,12,12,180)`）；而 `App::ui` 提供的根
    /// `Ui` 无背景，未被白色卡片/栏覆盖的区域会透出该底色。这里改为 [`PAGE_BG`]，
    /// 使整窗为浅色页面 + 白色卡片，符合 mockup。
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        PAGE_BG.to_normalized_gamma_f32()
    }
}
