//! 图形界面的后台执行器：在工作线程运行 trt 核心，经通道回传进度与结果。
//!
//! 对应 data-model.md 实体 6 与 FR-012/SC-006：窗口事件循环在主线程，耗时替换在后台线程，
//! 二者通过 [`std::sync::mpsc`] 通道通信，保证执行期间界面不冻结。

use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use crate::commands::trt::{self, ProgressUpdate, RunSummary};
use crate::error::CctError;

use super::form::GuiFormState;

/// 后台线程发送给 UI 线程的消息（参见 data-model.md 实体 6）。
pub enum GuiMessage {
    /// 执行中进度更新。
    Progress(ProgressUpdate),
    /// 成功完成，携带影响报告。
    Finished(RunSummary),
    /// 执行失败（含中文错误信息）。
    Error(String),
}

/// 一次后台执行的句柄：持有接收端，UI 每帧轮询。
pub struct RunHandle {
    /// 接收后台消息的通道接收端。
    pub receiver: Receiver<GuiMessage>,
}

/// 在后台线程执行表单对应的 trt 操作。
///
/// 先在调用线程（UI 线程）完成参数校验（快速，失败立即反馈）；校验通过后 spawn 工作线程，
/// 通过通道回传进度与最终结果/错误。返回的 [`RunHandle`] 供 UI 轮询。
pub fn start(form: &GuiFormState) -> RunHandle {
    let (tx, rx): (Sender<GuiMessage>, Receiver<GuiMessage>) = std::sync::mpsc::channel();

    // 参数校验在 UI 线程同步进行（开销极小），失败则直接通过通道回传错误。
    let options = match form.to_options() {
        Ok(o) => o,
        Err(e) => {
            let _ = tx.send(GuiMessage::Error(e.to_string()));
            return RunHandle { receiver: rx };
        }
    };

    // 后台线程执行替换/撤销。
    thread::spawn(move || {
        let progress_tx = tx.clone();
        let result = trt::execute(&options, move |u: ProgressUpdate| {
            // 进度发送失败（UI 已关闭）时忽略。
            let _ = progress_tx.send(GuiMessage::Progress(u));
        });
        let msg = match result {
            Ok(summary) => GuiMessage::Finished(summary),
            Err(e) => GuiMessage::Error(describe_error(&e)),
        };
        let _ = tx.send(msg);
    });

    RunHandle { receiver: rx }
}

/// 将错误转为面向界面的中文描述。
fn describe_error(e: &CctError) -> String {
    e.to_string()
}
