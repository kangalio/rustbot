use crate::{ban::unban_users, command_history::clear_command_history, Error};
use serenity::client::Context;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread::sleep,
    time::Duration,
};

static JOBS_THREAD_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn start_jobs(cx: Context) {
    if !JOBS_THREAD_INITIALIZED.load(Ordering::SeqCst) {
        JOBS_THREAD_INITIALIZED.store(true, Ordering::SeqCst);
        std::thread::spawn(move || -> Result<(), Error> {
            loop {
                unban_users(&cx)?;
                clear_command_history(&cx)?;

                sleep(Duration::from_secs(3600));
            }
        });
    }
}
