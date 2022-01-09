use std::{io::{Read, Result, Write}, os::unix::prelude::AsRawFd};

use futures_lite::io::Empty;

use crate::process::NonBlocking;

pub struct EmptyStream;

impl Write for EmptyStream {
    fn write(&mut self, _: &[u8]) -> Result<usize> {
        Ok(0)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Read for EmptyStream {
    fn read(&mut self, _: &mut [u8]) -> Result<usize> {
        Ok(0)
    }
}

impl AsRawFd for EmptyStream {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        // it must be save as long as 
        0
    }
}