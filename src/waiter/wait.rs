use std::{
    io::{self, Read},
    time::Duration,
};

use crossbeam_channel::Receiver;

use super::blocking::Blocking;

pub struct Wait2<R1, R2> {
    recv: Receiver<(usize, io::Result<Option<u8>>)>,
    b1: Blocking<R1>,
    b2: Blocking<R2>,
    timeout: Duration,
}

pub enum Recv {
    R1(io::Result<Option<u8>>),
    R2(io::Result<Option<u8>>),
    Timeout,
}

impl<R1, R2> Wait2<R1, R2> {
    pub fn new(r1: R1, r2: R2) -> Self
    where
        R1: Send + Read + 'static,
        R2: Send + Read + 'static,
    {
        let (sndr, recv) = crossbeam_channel::unbounded();

        let b1 = Blocking::new(0, r1, sndr.clone());
        let b2 = Blocking::new(1, r2, sndr);

        Self {
            b1,
            b2,
            recv,
            timeout: Duration::from_secs(5),
        }
    }

    pub fn join(self) -> std::thread::Result<()> {
        self.b1.join()?;
        self.b2.join()?;
        Ok(())
    }

    pub fn recv(&mut self) -> Result<Recv, crossbeam_channel::RecvError> {
        match self.recv.recv_timeout(self.timeout) {
            Ok((id, result)) => match id {
                0 => Ok(Recv::R1(result)),
                1 => Ok(Recv::R2(result)),
                _ => unreachable!(),
            },
            Err(err) => {
                if err.is_timeout() {
                    Ok(Recv::Timeout)
                } else {
                    Err(crossbeam_channel::RecvError)
                }
            }
        }
    }
}
