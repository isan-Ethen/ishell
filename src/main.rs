use nix::{
    sys::{
        signal::{kill, SIGTERM},
        wait::*,
    },
    unistd::{execv, fork, getpid, ForkResult},
};
use std::{env, ffi::CString, io, io::Write};

#[allow(unreachable_code)]
fn main() {
    let home_dir = env::var("HOME").unwrap();

    loop {
        println!("current {}", std::process::id());
        let cmd_root: String = String::from("/bin/");
        let mut buffer = String::new();
        let current_dir = env::current_dir()
            .unwrap()
            .into_os_string()
            .into_string()
            .unwrap();
        loop {
            print!("{}$ ", current_dir.replace(&home_dir, "~"));
            io::stdout().flush().unwrap();

            match io::stdin().read_line(&mut buffer) {
                Ok(_) => break,
                Err(e) => {
                    eprintln!("Can not read line: {e}");
                }
            };
        }
        let mut inputs = buffer.split_whitespace();
        if let Some(command) = inputs.next() {
            match command {
                "cd" => {
                    if let Some(path) = inputs.next() {
                        match env::set_current_dir(path) {
                            Ok(_) => {}
                            Err(why) => eprintln!("Couldn't change directory: {}", why),
                        }
                    } else {
                        env::set_current_dir(&home_dir).unwrap();
                    }
                }
                "clear" => println!("\x1B[2J\x1B[1;1H"),
                "exit" => break,
                _ => {
                    let dir = cmd_root + command;
                    match unsafe { fork() }.expect("fork failed") {
                        ForkResult::Parent { child } => {
                            println!("parent {}", std::process::id());
                            match waitpid(child, None).expect("wait_pid failed") {
                                WaitStatus::Exited(pid, status) => {
                                    println!("exit: pid{:?}, status={:?}", pid, status)
                                }
                                WaitStatus::Signaled(pid, status, _) => {
                                    println!("signal: pid={:?}, status{:?}", pid, status)
                                }
                                _ => println!("abnormal exit"),
                            }
                        }
                        ForkResult::Child => {
                            println!("child {}", std::process::id());
                            let dir = CString::new(dir).expect("Can not cast dir to CString");
                            let mut args: Vec<CString> = Vec::from([dir.clone()]);
                            for arg in inputs.into_iter() {
                                args.push(CString::new(arg).expect("Can not cast arg to CString"));
                            }
                            match execv(&dir, &args[..]) {
                                Err(why) => {
                                    eprintln!("Execution failed: {}", why);
                                    match kill(getpid(), SIGTERM) {
                                        Ok(_) => {}
                                        Err(err) => {
                                            eprintln!("Child process: Error terminating: {}", err)
                                        }
                                    }
                                }
                                Ok(_) => {}
                            }
                        }
                    }
                }
            }
        }
    }
}
