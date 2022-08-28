use std::{io::{Result, Read}, marker::PhantomData, thread::JoinHandle};

use crossbeam_channel::Sender;

/// Blocking implements a reading operation on a different thread.
/// It stops the thread once any [`Error`] is encountered.
pub struct Blocking<R> {
    _id: usize,
    _thread: JoinHandle<()>,
    _reader: PhantomData<R>,
}

impl<R> Blocking<R> {
    /// Creates a new blocking reader, spawning a new thread for it.
    pub fn new(id: usize, mut reader: R, sendr: Sender<(usize, Result<Option<u8>>)>) -> Self
    where
        R: Read + Send + 'static,
    {
        let handle = std::thread::spawn(move || {
            let mut buffer = Vec::new();
            let mut buf = [0; 1];
            loop {
                match reader.read(&mut buf) {
                    Ok(n) => {
                        // try send failed tries
                        if !buffer.is_empty() {
                            for b in buffer.drain(..).collect::<Vec<_>>() {
                                try_send(id, Ok(Some(b)), &sendr, &mut buffer);
                            }
                        }

                        if n == 0 {
                            try_send(id, Ok(None), &sendr, &mut buffer);
                            break;
                        } else {
                            try_send(id, Ok(Some(buf[0])), &sendr, &mut buffer);
                        }
                    },
                    Err(err) => {
                        // stopping the thread on error
                        try_send(id, Err(err), &sendr, &mut buffer);
                        break;
                    },
                }
            }
        });

        Self {
            _id: id,
            _thread: handle,
            _reader: PhantomData,
        }
    }

    pub fn join(self) -> std::thread::Result<()> {
        self._thread.join()
    }
}

fn try_send(id: usize, msg: Result<Option<u8>>, sendr: &Sender<(usize, Result<Option<u8>>)>, buf: &mut Vec<u8>) {
    match sendr.send((id, msg)) {
        Ok(_) => (),
        Err(err) => {
            if let Ok(Some(b)) = err.0.1 {
                buf.push(b);
            }
        },
    }
}
