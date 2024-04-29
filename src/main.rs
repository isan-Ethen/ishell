use nix::{
    fcntl::{open, OFlag},
    libc::{STDIN_FILENO, STDOUT_FILENO},
    sys::{
        signal::{kill, SIGTERM},
        stat::Mode,
        wait::{waitpid, WaitStatus},
    },
    unistd::{chdir, close, dup2, execvp, fork, getpid, ForkResult},
};
use std::path::PathBuf;
use std::{env, ffi::CString, io, io::Write};

enum State {
    IN,
    OUT,
    NORMAL,
}

fn main() {
    let home_dir = env::var("HOME").unwrap();

    loop {
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
                Err(why) => {
                    eprintln!("Can not read line: {}", why);
                }
            };
        }
        let mut inputs = buffer.split_whitespace();
        if let Some(command) = inputs.next() {
            match command {
                "cd" => {
                    if let Some(path) = inputs.next() {
                        match chdir(path) {
                            Ok(_) => {}
                            Err(why) => eprintln!("Couldn't change directory: {}", why),
                        }
                    } else {
                        let mut home = PathBuf::new();
                        home.push(home_dir.as_str());
                        chdir(&home).unwrap();
                    }
                }
                "clear" => println!("\x1B[2J\x1B[1;1H"),
                "exit" => break,
                _ => match unsafe { fork() }.expect("fork failed") {
                    ForkResult::Parent { child } => {
                        println!("parent {}", std::process::id());
                        match waitpid(child, None).expect("waitpid failed") {
                            WaitStatus::Exited(pid, status) => {
                                println!("Exit: pid={:?}, status={:?}", pid, status)
                            }
                            WaitStatus::Signaled(pid, status, _) => {
                                println!("Signal: pid={:?}, status={:?}", pid, status)
                            }
                            _ => println!("Other exit"),
                        }
                    }
                    ForkResult::Child => {
                        println!("child {}", std::process::id());
                        let command = CString::new(command).unwrap();
                        let mut args: Vec<CString> = Vec::from([command.clone()]);
                        let mut status = State::NORMAL;
                        for arg in inputs.into_iter() {
                            if arg == ">" {
                                status = State::OUT;
                                continue;
                            } else if arg == "<" {
                                status = State::IN;
                                continue;
                            }
                            match status {
                                State::IN => {
                                    let fd = open(arg, OFlag::O_RDONLY, Mode::S_IRUSR).unwrap();
                                    dup2(fd, STDIN_FILENO).unwrap();
                                    close(fd).unwrap();
                                    break;
                                }
                                State::OUT => {
                                    let fd = open(arg, OFlag::O_WRONLY, Mode::S_IWUSR).unwrap();
                                    dup2(fd, STDOUT_FILENO).unwrap();
                                    close(fd).unwrap();
                                    break;
                                }
                                State::NORMAL => args
                                    .push(CString::new(arg).expect("Can not cast arg to CString")),
                            }
                        }
                        match execvp(&command, &args[..]) {
                            Err(why) => {
                                eprintln!("Execution failed: {}", why);
                                match kill(getpid(), SIGTERM) {
                                    Ok(_) => {}
                                    Err(why) => {
                                        eprintln!("Couldn't terminate child: {}", why);
                                    }
                                }
                            }
                            Ok(_) => {}
                        }
                    }
                },
            }
        }
    }
}
