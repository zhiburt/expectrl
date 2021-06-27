use nix::fcntl::{open, OFlag};
use nix::libc::{signal, STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use nix::pty::PtyMaster;
use nix::pty::{grantpt, posix_openpt, unlockpt};
use nix::sys::stat::Mode;
use nix::sys::wait::{self, wait, waitpid, WaitStatus};
use nix::sys::{signal, termios};
use nix::unistd::{close, dup, dup2, fork, setsid, ForkResult, Pid};
use nix::Result;
use signal::Signal::SIGHUP;
use std::fs::File;
use std::os::unix::prelude::{AsRawFd, CommandExt, FromRawFd, RawFd};
use std::process::Command;

#[derive(Debug)]
pub(crate) struct PtyProcess {
    master: Master,
    child_pid: Pid,
}

impl PtyProcess {
    // make this result io::Result
    pub fn spawn(mut command: Command) -> Result<Self> {
        let master = Master::open()?;
        master.grant_slave_access()?;
        master.unlock_slave()?;

        let fork = unsafe { fork()? };
        match fork {
            ForkResult::Child => {
                create_new_session()?;

                let slave_name = master.get_slave_name()?;
                let slave_fd = open(slave_name.as_str(), OFlag::O_RDWR, Mode::empty())?;

                redirect_std_streams(slave_fd)?;

                off_input_echo_back()?;

                let _ = command.exec();

                Err(nix::Error::last())
            }
            ForkResult::Parent { child } => Ok(Self {
                master,
                child_pid: child,
            }),
        }
    }

    pub fn status(&self) -> Result<WaitStatus> {
        waitpid(self.child_pid, Some(wait::WaitPidFlag::WNOHANG))
    }

    pub fn signal(&self, signal: signal::Signal) -> Result<()> {
        signal::kill(self.child_pid, signal)
    }

    pub fn exit(&self) -> Result<()> {
        self.signal(signal::SIGTERM)
    }

    pub fn wait(&self) -> Result<WaitStatus> {
        waitpid(self.child_pid, None)
    }

    pub fn get_file_handle(&self) -> Result<File> {
        let fd = dup(self.master.fd.as_raw_fd())?;
        let file = unsafe { File::from_raw_fd(fd) };

        Ok(file)
    }
}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        if let Ok(WaitStatus::StillAlive) = self.status() {
            self.exit().unwrap();
            self.wait().unwrap();
        }
    }
}

#[derive(Debug)]
pub(crate) struct Master {
    fd: PtyMaster,
}

impl Master {
    pub fn open() -> Result<Self> {
        let master_fd = posix_openpt(OFlag::O_RDWR)?;
        Ok(Self { fd: master_fd })
    }

    pub fn grant_slave_access(&self) -> Result<()> {
        grantpt(&self.fd)
    }

    pub fn unlock_slave(&self) -> Result<()> {
        unlockpt(&self.fd)
    }

    pub fn get_slave_name(&self) -> Result<String> {
        get_slave_name(&self.fd)
    }
}

fn redirect_std_streams(fd: RawFd) -> Result<()> {
    // If fildes2 is already a valid open file descriptor, it shall be closed first
    close(STDIN_FILENO)?;
    close(STDOUT_FILENO)?;
    close(STDERR_FILENO)?;

    // use slave fd as std[in/out/err]
    dup2(fd, STDIN_FILENO)?;
    dup2(fd, STDOUT_FILENO)?;
    dup2(fd, STDERR_FILENO)?;

    Ok(())
}

fn off_input_echo_back() -> Result<()> {
    // Set echo off
    // Even though there may be something left behind https://stackoverflow.com/a/59034084
    let mut flags = termios::tcgetattr(STDIN_FILENO)?;
    flags.local_flags &= !termios::LocalFlags::ECHO;
    termios::tcsetattr(STDIN_FILENO, termios::SetArg::TCSANOW, &flags)?;

    Ok(())
}

fn create_new_session() -> Result<()> {
    setsid()?;
    Ok(())
}

#[cfg(target_family = "unix")]
#[cfg(not(target_os = "macos"))]
fn get_slave_name(fd: &PtyMaster) -> Result<String> {
    nix::pty::ptsname_r(fd)
}

/// Getting a slave name on darvin platform
/// https://blog.tarq.io/ptsname-on-osx-with-rust/
#[cfg(target_os = "macos")]
fn get_slave_name(fd: &PtyMaster) -> Result<String> {
    use nix::libc::ioctl;
    use nix::libc::TIOCPTYGNAME;
    use std::ffi::CStr;
    use std::os::raw::c_char;
    use std::os::unix::prelude::AsRawFd;

    // ptsname_r is a linux extension but ptsname isn't thread-safe
    // we could use a static mutex but instead we re-implemented ptsname_r with a syscall
    // ioctl(fd, TIOCPTYGNAME, buf) manually
    // the buffer size on OSX is 128, defined by sys/ttycom.h
    let mut buf: [c_char; 128] = [0; 128];

    let fd = fd.as_raw_fd();
    unsafe {
        match ioctl(fd, TIOCPTYGNAME as u64, &mut buf) {
            0 => {
                let string = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
                return Ok(string);
            }
            _ => Err(nix::Error::last()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::io::BufReader;
    use std::time;
    use std::{
        io::{Read, Write},
        thread,
    };

    use super::*;

    #[test]
    fn pty_cat() -> Result<()> {
        let command = Command::new("cat");
        let proc = PtyProcess::spawn(command)?;
        let mut file = proc.get_file_handle()?;

        let size = write(&file, "hello cat");
        assert_eq!(read_exact(&file, size).unwrap(), "hello cat");

        let size = write(&file, "hello cat second time");
        assert_eq!(read_exact(&file, size).unwrap(), "hello cat second time");

        // without a timeout it sometimes fails to receive a ^C
        thread::sleep(time::Duration::from_millis(100));

        // Ctrl-C is etx(End of text). Thus send \x03.
        file.write_all(&[3]).unwrap(); // send ^C
        file.flush().unwrap();

        assert_eq!(
            WaitStatus::Signaled(proc.child_pid, signal::Signal::SIGINT, false),
            proc.wait()?
        );

        Ok(())
    }

    #[test]
    fn ptyprocess_check_terminal_line_settings() -> Result<()> {
        let mut command = Command::new("stty");
        command.arg("-a");
        let proc = PtyProcess::spawn(command)?;
        let file = proc.get_file_handle()?;

        let (output, err) = read_all(file);

        println!("{}", output);

        assert!(output.split_whitespace().any(|word| word == "-echo"));
        assert_eq!(
            err.unwrap().to_string(),
            "Input/output error (os error 5)".to_string()
        );

        Ok(())
    }

    #[test]
    fn create_pty() -> Result<()> {
        let master = Master::open()?;
        master.grant_slave_access()?;
        master.unlock_slave()?;
        let slavename = master.get_slave_name()?;
        assert!(slavename.starts_with("/dev"));
        println!("slave name {}", slavename);
        Ok(())
    }

    #[test]
    fn release_pty_master() -> Result<()> {
        let master = Master::open()?;
        let old_master_fd = master.fd.as_raw_fd();

        drop(master);

        let master = Master::open()?;

        assert_eq!(master.fd.as_raw_fd(), old_master_fd);

        Ok(())
    }

    fn write<W: Write + Read, S: AsRef<str>>(mut writer: W, msg: S) -> usize {
        let msg = msg.as_ref();
        write!(writer, "{}", msg).unwrap();
        writer.flush().unwrap();

        msg.len()
    }

    fn read_exact<R: Read>(mut reader: R, length: usize) -> io::Result<String> {
        let mut buf = vec![0u8; length];
        reader.read_exact(&mut buf[..])?;

        Ok(String::from_utf8(buf).unwrap())
    }

    // read's input by byte so it returns a erorr only on last byte.
    fn read_all<R: Read>(reader: R) -> (String, Option<io::Error>) {
        let mut reader = BufReader::new(reader);
        let mut string = Vec::new();
        let buf = &mut [0u8; 1];
        loop {
            match reader.read(&mut buf[..]) {
                Ok(0) => return (String::from_utf8_lossy(&string).to_string(), None),
                Ok(_) => {
                    string.push(buf[0]);
                }
                Err(e) => return (String::from_utf8_lossy(&string).to_string(), Some(e)),
            }
        }
    }
}
