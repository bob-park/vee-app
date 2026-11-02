#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    // Model inference runs in a child of this exe; it must never start the app.
    if args.first().is_some_and(|a| a == vee_app_lib::AI_WORKER_FLAG) {
        vee_app_lib::ai_worker_main();
        return ExitCode::SUCCESS;
    }
    // PDF → Markdown too, so a PDF that crashes PDFium never takes the app down.
    if args.first().is_some_and(|a| a == vee_app_lib::PDF_WORKER_FLAG) {
        return vee_app_lib::pdf_worker_main(&args[1..]);
    }
    vee_app_lib::run();
    ExitCode::SUCCESS
}
