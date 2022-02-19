use expectrl::{self, Any, Eof};

#[cfg(not(feature = "async"))]
fn main() {
    let mut session = expectrl::spawn("ls -al").expect("Can't spawn a session");

    loop {
        let m = session
            .expect(Any::boxed(vec![
                Box::new("\r"),
                Box::new("\n"),
                Box::new(Eof),
            ]))
            .expect("Expect failed");

        println!("{:?}", String::from_utf8_lossy(m.as_bytes()));

        let is_eof = m.matches()[0].is_empty();
        if is_eof {
            break;
        }

        if m.matches()[0] == [b'\n'] {
            continue;
        }

        println!("{:?}", String::from_utf8_lossy(m.matches()[0]));
    }
}

#[cfg(feature = "async")]
fn main() {
    futures_lite::future::block_on(async {
        let mut session = expectrl::spawn("ls -al").expect("Can't spawn a session");

        loop {
            let m = session
                .expect(Any::boxed(vec![
                    Box::new("\r"),
                    Box::new("\n"),
                    Box::new(Eof),
                ]))
                .await
                .expect("Expect failed");

            let is_eof = m.matches()[0].is_empty();
            if is_eof {
                break;
            }

            if m.matches()[0] == [b'\n'] {
                continue;
            }

            println!("{:?}", String::from_utf8_lossy(m.matches()[0]));
        }
    })
}
