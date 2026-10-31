#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Model inference runs in a child of this exe; it must never start the app.
    if std::env::args().nth(1).as_deref() == Some(vee_app_lib::AI_WORKER_FLAG) {
        return vee_app_lib::ai_worker_main();
    }
    vee_app_lib::run()
}
