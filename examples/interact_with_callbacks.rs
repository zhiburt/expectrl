use expectrl::{interact::InteractOptions, spawn, Regex};

#[derive(Debug, Default)]
struct State {
    stutus_verification_counter: Option<usize>,
    wait_for_continue: Option<bool>,
    pressed_yes_on_continue: Option<bool>,
}

#[cfg(not(feature = "async"))]
fn main() {
    use expectrl::{interact::Context, session::Session};

    let mut session = spawn("python ./tests/source/ansi.py").expect("Can't spawn a session");

    let mut opts = InteractOptions::default()
        .state(State::default())
        .on_output("Continue [y/n]:", |mut ctx, _| {
            ctx.state().wait_for_continue = Some(true);
            Ok(())
        })
        .on_input("y", |mut ctx: Context<Session, _, _, _>| {
            ctx.session().send("y")?;

            if let Some(_a @ true) = ctx.state().wait_for_continue {
                ctx.state().pressed_yes_on_continue = Some(true);
            }

            Ok(())
        })
        .on_input("n", |mut ctx| {
            ctx.session().send("n")?;

            if let Some(_a @ true) = ctx.state().wait_for_continue {
                ctx.state().pressed_yes_on_continue = Some(false);
            }

            Ok(())
        })
        .on_output(Regex("status:\\s*.*\\w+.*\\r\\n"), |mut ctx, _| {
            match ctx.state().stutus_verification_counter.as_mut() {
                Some(c) => *c += 1,
                None => {
                    ctx.state().stutus_verification_counter = Some(1);
                }
            }

            Ok(())
        });

    #[cfg(not(feature = "async"))]
    {
        opts.interact_in_terminal(&mut session).unwrap();
    }
    #[cfg(feature = "async")]
    {
        futures_lite::future::block_on(opts.interact(&mut session)).unwrap();
    }

    println!("RESULTS________");
    println!(
        "Was pressed yes = {}",
        opts.get_state().pressed_yes_on_continue.unwrap_or_default()
    );
    println!(
        "Status counter = {}",
        opts.get_state()
            .stutus_verification_counter
            .unwrap_or_default()
    );
}

#[cfg(feature = "async")]
fn main() {}
