use crate::{hlt_loop, println, process::exit_current_process};

pub fn entry_point() -> ! {
    println!("Aight!");
    exit_current_process(0);

    hlt_loop();
}
