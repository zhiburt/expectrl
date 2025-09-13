use expectrl::{check, spawn, Error, Expect};

#[cfg(not(feature = "async"))]
fn main() {
    let mut p = spawn("python ./tests/source/ansi.py").unwrap();

    loop {
        let result = check! {
            &mut p,
            _ = "Password: " => {
                println!("Set password to SECURE_PASSWORD");
                p.send_line("SECURE_PASSWORD").unwrap();
            },
            _ = "Continue [y/n]:" => {
                println!("Stop processing");
                p.send_line("n").unwrap();
            },
        };

        match result {
            Ok(_) => {}
            Err(Error::Eof) => break,
            Err(_) => result.unwrap(),
        };
    }
}

#[cfg(feature = "async")]
fn main() {
    use expectrl::AsyncExpect;

    let f = async {
        let mut p = spawn("python ./tests/source/ansi.py").unwrap();

        loop {
            let result = check! {
                &mut p,
                _ = "Password: " => {
                    println!("Set password to SECURE_PASSWORD");
                    p.send_line("SECURE_PASSWORD").await.unwrap();
                },
                _ = "Continue [y/n]:" => {
                    println!("Stop processing");
                    p.send_line("n").await.unwrap();
                },
            }
            .await;

            match result {
                Ok(_) => {}
                Err(Error::Eof) => break,
                Err(_) => result.unwrap(),
            };
        }
    };

    futures_lite::future::block_on(f);
}
