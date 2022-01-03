use std::io::{Read, Result, Write};

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

impl NonBlocking for EmptyStream {
    fn set_non_blocking(&mut self) -> Result<()> {
        Ok(())
    }

    fn set_blocking(&mut self) -> Result<()> {
        Ok(())
    }
}
