use nix::{
    fcntl::{open, OFlag},
    libc::{STDIN_FILENO, STDOUT_FILENO},
    sys::{
        signal::{kill, SIGTERM},
        stat::Mode,
        wait::{waitpid, WaitStatus},
    },
    unistd::{chdir, close, dup2, execvp, fork, getpid, pipe, ForkResult},
};
use std::os::fd::RawFd;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::{env, ffi::CString, fs, io, io::Write};

static HOME_DIR: OnceLock<String> = OnceLock::new();

#[derive(Debug)]
struct Command {
    argv: Vec<CString>,
    infd: Option<RawFd>,
    outfd: Option<RawFd>,
}

impl Command {
    fn new(argv: Vec<CString>) -> Self {
        Self {
            argv,
            infd: None,
            outfd: None,
        }
    }

    fn from_fd(argv: Vec<CString>, infd: Option<RawFd>, outfd: Option<RawFd>) -> Self {
        Self { argv, infd, outfd }
    }

    fn change_outfd(&mut self, outfd: Option<RawFd>) {
        self.outfd = outfd;
    }

    fn execute(&self) {
        match unsafe { fork() }.expect("fork failed") {
            ForkResult::Parent { child } => {
                // println!("parent {}", std::process::id());
                match waitpid(child, None).expect("waitpid failed") {
                    WaitStatus::Exited(pid, status) => {
                        println!("Exit: pid={:?}, status={:?}", pid, status)
                    }
                    WaitStatus::Signaled(pid, status, _) => {
                        println!("Signal: pid={:?}, status={:?}", pid, status)
                    }
                    _ => eprintln!("Other waitstatus"),
                }
            }
            ForkResult::Child => {
                // println!("child {}", std::process::id());
                if let Some(fd) = self.infd {
                    dup2(fd, STDIN_FILENO).expect("Duplicate fd1 to fd2 failed");
                }
                if let Some(fd) = self.outfd {
                    dup2(fd, STDOUT_FILENO).expect("Duplicate fd1 to fd2 failed");
                }
                match execvp(&self.argv[0], &self.argv) {
                    Err(why) => {
                        eprintln!("Execute command failed: {}", why);
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
        }
        // }
    }
}

fn main() {
    let _ = HOME_DIR.set(env::var("HOME").expect("Couldn't get HOME"));

    loop {
        let mut buffer = String::new();
        let current_dir = env::current_dir()
            .unwrap()
            .into_os_string()
            .into_string()
            .unwrap();
        loop {
            print!("{}$ ", current_dir.replace(HOME_DIR.get().unwrap(), "~"));
            io::stdout().flush().unwrap();

            match io::stdin().read_line(&mut buffer) {
                Ok(_) => break,
                Err(why) => {
                    eprintln!("Can not read line: {}", why);
                }
            };
        }
        let (mut commands, mut pipev) = line_parser(buffer, current_dir);
        // println!("{:?}", commands);
        run_commands(&mut commands, &mut pipev);
    }
}

fn line_parser(buffer: String, current_dir: String) -> (Vec<Command>, Vec<(RawFd, RawFd)>) {
    let mut inputs = buffer
        .split_whitespace()
        .into_iter()
        .chain(std::iter::once(";"));
    let mut commands: Vec<Command> = Vec::new();
    let mut args: Vec<CString> = Vec::new();
    let mut fd: Option<RawFd> = None;
    let mut pipev: Vec<(RawFd, RawFd)> = Vec::new();
    while inputs.size_hint().0 > 0usize {
        if let Some(arg) = inputs.next() {
            match arg {
                "cd" => {
                    if let Some(path) = inputs.next() {
                        match chdir(path) {
                            Ok(_) => {}
                            Err(why) => eprintln!("Couldn't change directory: {}", why),
                        }
                    } else {
                        let mut home = PathBuf::new();
                        home.push(HOME_DIR.get().unwrap().as_str());
                        match chdir(&home) {
                            Ok(_) => {}
                            Err(why) => eprintln!("Couldn't change directory: {}", why),
                        }
                    }
                }
                "<" => {
                    if let Some(arg) = inputs.next() {
                        match open(arg, OFlag::O_RDONLY, Mode::S_IRUSR) {
                            Ok(fd0) => {
                                commands.push(Command::from_fd(args.clone(), Some(fd0), None));
                            }
                            Err(why) => {
                                eprintln!("Couldn't open {} as FileDescriptor: {}", arg, why);
                                commands.push(Command::from_fd(args.clone(), None, None));
                            }
                        }
                        args.clear();
                    } else {
                        eprintln!("File Descriptor wasn't gived");
                    }
                }
                ">" => {
                    if let Some(arg) = inputs.next() {
                        match open(arg, OFlag::O_WRONLY, Mode::S_IWUSR) {
                            Ok(fd1) => {
                                if let Some(fd0) = fd {
                                    commands.push(Command::from_fd(
                                        args.clone(),
                                        Some(fd0),
                                        Some(fd1),
                                    ));
                                } else {
                                    commands.push(Command::from_fd(args.clone(), None, Some(fd1)));
                                }
                            }
                            Err(why) => {
                                eprintln!("Couldn't open {} as FileDescriptor: {}", arg, why);
                                if let Some(fd0) = fd {
                                    commands.push(Command::from_fd(args.clone(), Some(fd0), None));
                                } else {
                                    commands.push(Command::from_fd(args.clone(), None, None));
                                }
                            }
                        }
                        args.clear();
                    } else {
                        eprintln!("File Descriptor wasn't gived");
                    }
                }
                "*" => {
                    if let Ok(readdir) = fs::read_dir(&current_dir) {
                        for entry in readdir {
                            if let Ok(file) = entry {
                                args.push(
                                    CString::new(file.file_name().into_string().unwrap()).unwrap(),
                                );
                            }
                        }
                    }
                }
                "|" => {
                    let pipe = pipe().expect("Couldn't generate pipe");
                    if args.len() > 0 {
                        commands.push(Command::from_fd(args.clone(), fd.take(), Some(pipe.1)));
                    } else {
                        if let Some(command) = commands.last_mut() {
                            command.change_outfd(Some(pipe.1));
                        } else {
                            eprintln!("There is no command before |")
                        }
                    }
                    fd = Some(pipe.0);
                    args.clear();
                    pipev.push(pipe);
                }
                ";" => {
                    if args.len() > 0 {
                        if let Some(fd0) = fd {
                            commands.push(Command::from_fd(args.clone(), Some(fd0), None));
                        } else {
                            commands.push(Command::new(args.clone()));
                        }
                        args.clear();
                    }
                }
                _ => {
                    args.push(CString::new(arg).expect("Can not cast arg to CString"));
                }
            }
        }
    }
    (commands, pipev)
}

fn run_commands(commands: &mut Vec<Command>, pipev: &mut Vec<(RawFd, RawFd)>) {
    let mut commands = commands.iter();
    let mut pipev = pipev.iter();
    let mut is_pipe = false;

    while let Some(command) = commands.next() {
        if command.outfd.is_some() {
            is_pipe = true;
        }
        if command.infd.is_some() && is_pipe {
            // println!("{:?}", pipev);
            if let Some((_, outpipe)) = pipev.next() {
                close(*outpipe).expect("Close outpipe failed");
                // close(*inpipe).expect("Close inpipe failed");
                // println!("Pipe closed");
            }
        }
        command.execute();
    }
}
