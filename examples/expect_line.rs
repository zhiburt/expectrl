use expectrl::{self, Any, Eof};

#[cfg(not(feature = "async"))]
fn main() {
    let mut session = expectrl::spawn("ls -al").expect("Can't spawn a session");

    loop {
        let m = session
            .expect(Any(vec![Box::new("\r"), Box::new("\n"), Box::new(Eof)]))
            .expect("Expect failed");

        let is_eof = m.first().is_empty();
        if is_eof {
            break;
        }

        if m.first() == [b'\n'] {
            continue;
        }

        println!("{:?}", String::from_utf8_lossy(m.first()));
    }
}

#[cfg(feature = "async")]
fn main() {
    futures_lite::future::block_on(async {
        let mut session = expectrl::spawn("ls -al").expect("Can't spawn a session");

        loop {
            let m = session
                .expect(Any(vec![Box::new("\r"), Box::new("\n"), Box::new(Eof)]))
                .await
                .expect("Expect failed");

            let is_eof = m.first().is_empty();
            if is_eof {
                break;
            }

            if m.first() == [b'\n'] {
                continue;
            }

            println!("{:?}", String::from_utf8_lossy(m.first()));
        }
    })
}
