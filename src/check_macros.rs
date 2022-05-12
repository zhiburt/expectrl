//! This module contains a `check!` macros.

/// Check macros provides a convient way to check if things are available in a stream of a process.
///
/// It falls into a corresponding branch when a pattern was matched.
/// It runs checks from top to bottom.
/// It doesn't wait until any of them be available.
/// If you want to wait until any of the input available you must use your own approach for example putting it in a loop.
///
/// You can specify a default branch which will be called if nothing was matched.
///
/// The macros levareges [crate::Session::check] function, so its just made for convience.
///
/// # Example
/// ```no_run
/// # let mut session = expectrl::spawn("cat").unwrap();
/// #
/// loop {
///     expectrl::check!{
///         &mut session,
///         world = "\r" => {
///             // handle end of line
///         },
///         _ = "Hello World" => {
///             // handle Hello World
///         },
///         default => {
///             // handle no matches
///         },
///     }
///     .unwrap();
/// }
/// ```
#[cfg(not(feature = "async"))]
#[macro_export]
macro_rules! check {
    (@check ($($tokens:tt)*) ($session:expr)) => {
        $crate::check!(@case $session, ($($tokens)*), (), ())
    };
    (@check ($session:expr, $($tokens:tt)*) ()) => {
        $crate::check!(@check ($($tokens)*) ($session))
    };
    (@check ($session:expr, $($tokens:tt)*) ($session2:expr)) => {
        compile_error!("Wrong number of session arguments")
    };
    (@check ($($tokens:tt)*) ()) => {
        compile_error!("Please provide a session as a first argument")
    };
    (@check () ($session:expr)) => {
        // there's no reason to run 0 checks so we issue a error.
        compile_error!("There's no reason in running check with no arguments. Please supply a check branches")
    };
    (@case $session:expr, ($var:tt = $exp:expr => $body:tt, $($tail:tt)*), ($($head:tt)*), ($($default:tt)*)) => {
        // regular case
        //
        // note: we keep order correct by putting head at the beggining
        $crate::check!(@case $session, ($($tail)*), ($($head)* $var = $exp => $body, ), ($($default)*))
    };
    (@case $session:expr, ($var:tt = $exp:expr => $body:tt $($tail:tt)*), ($($head:tt)*), ($($default:tt)*)) => {
        // allow missing comma
        //
        // note: we keep order correct by putting head at the beggining
        $crate::check!(@case $session, ($($tail)*), ($($head)* $var = $exp => $body, ), ($($default)*))
    };
    (@case $session:expr, (default => $($tail:tt)*), ($($head:tt)*), ($($default:tt)+)) => {
        // A repeated default branch
        compile_error!("Only 1 default case is allowed")
    };
    (@case $session:expr, (default => $body:tt, $($tail:tt)*), ($($head:tt)*), ()) => {
        // A default branch
        $crate::check!(@case $session, ($($tail)*), ($($head)*), ( { $body; #[allow(unreachable_code)] Result::<(), $crate::Error>::Ok(()) } ))
    };
    (@case $session:expr, (default => $body:tt $($tail:tt)*), ($($head:tt)*), ()) => {
        // A default branch
        // allow missed comma `,`
        $crate::check!(@case $session, ($($tail)*), ($($head)*), ( { $body; #[allow(unused_qualifications)] Result::<(), $crate::Error>::Ok(()) } ))
    };
    (@case $session:expr, (), ($($head:tt)*), ()) => {
        // there's no default branch
        // so we make up our own.
        $crate::check!(@case $session, (), ($($head)*), ( { #[allow(unused_qualifications)] Result::<(), $crate::Error>::Ok(()) } ))
    };
    (@case $session:expr, (), ($($tail:tt)*), ($($default:tt)*)) => {
        // last point of @case
        // call code generation via @branch
        $crate::check!(@branch $session, ($($tail)*), ($($default)*))
    };
    // We need to use a variable for pattern mathing,
    // user may chose to drop var name using a placeholder '_',
    // in which case we can't call a method on such identificator.
    //
    // We could use an approach like was described
    //
    // ```
    // Ok(_____random_var_name_which_supposedly_mustnt_be_used) if !_____random_var_name_which_supposedly_mustnt_be_used.is_empty() =>
    // {
    //      let $var = _____random_var_name_which_supposedly_mustnt_be_used;
    // }
    // ```
    //
    // The question is which solution is more effichient.
    // I took the following approach because there's no chance we influence user's land via the variable name we pick.
    (@branch $session:expr, ($var:tt = $exp:expr => $body:tt, $($tail:tt)*), ($($default:tt)*)) => {
        match $crate::session::Session::check($session, $exp) {
            result if result.as_ref().map(|found| !found.is_empty()).unwrap_or(false) => {
                let $var = result.unwrap();
                $body;
                #[allow(unreachable_code)]
                Ok(())
            }
            Ok(_) => {
                $crate::check!(@branch $session, ($($tail)*), ($($default)*))
            }
            Err(err) => Err(err),
        }
    };
    (@branch $session:expr, (), ($default:tt)) => {
        // A standart default branch
        $default
    };
    (@branch $session:expr, ($($tail:tt)*), ($($default:tt)*)) => {
        compile_error!(
            concat!(
                "No supported syntax tail=(",
                stringify!($($tail,)*),
                ") ",
                "default=(",
                stringify!($($default,)*),
                ") ",
        ))
    };
    // Entry point
    ($($tokens:tt)*) => {
        $crate::check!(@check ($($tokens)*) ())
    };
}

/// See sync version.
///
/// Async version of macros use the same approach as sync.
/// It doesn't use any future features.
/// So it may be better better you to use you own approach how to check which one done first.
/// For example you can use `futures_lite::future::race`.
///
// async version completely the same as sync version expect 2 words '.await' and 'async'
// meaning its a COPY && PASTE
#[cfg(feature = "async")]
#[macro_export]
macro_rules! check {
    (@check ($($tokens:tt)*) ($session:expr)) => {
        $crate::check!(@case $session, ($($tokens)*), (), ())
    };
    (@check ($session:expr, $($tokens:tt)*) ()) => {
        $crate::check!(@check ($($tokens)*) ($session))
    };
    (@check ($session:expr, $($tokens:tt)*) ($session2:expr)) => {
        compile_error!("Wrong number of session arguments")
    };
    (@check ($($tokens:tt)*) ()) => {
        compile_error!("Please provide a session as a first argument")
    };
    (@check () ($session:expr)) => {
        // there's no reason to run 0 checks so we issue a error.
        compile_error!("There's no reason in running check with no arguments. Please supply a check branches")
    };
    (@case $session:expr, ($var:tt = $exp:expr => $body:tt, $($tail:tt)*), ($($head:tt)*), ($($default:tt)*)) => {
        // regular case
        //
        // note: we keep order correct by putting head at the beggining
        $crate::check!(@case $session, ($($tail)*), ($($head)* $var = $exp => $body, ), ($($default)*))
    };
    (@case $session:expr, ($var:tt = $exp:expr => $body:tt $($tail:tt)*), ($($head:tt)*), ($($default:tt)*)) => {
        // allow missing comma
        //
        // note: we keep order correct by putting head at the beggining
        $crate::check!(@case $session, ($($tail)*), ($($head)* $var = $exp => $body, ), ($($default)*))
    };
    (@case $session:expr, (default => $($tail:tt)*), ($($head:tt)*), ($($default:tt)+)) => {
        // A repeated default branch
        compile_error!("Only 1 default case is allowed")
    };
    (@case $session:expr, (default => $body:tt, $($tail:tt)*), ($($head:tt)*), ()) => {
        // A default branch
        $crate::check!(@case $session, ($($tail)*), ($($head)*), ( { $body; #[allow(unreachable_code)] Result::<(), $crate::Error>::Ok(()) } ))
    };
    (@case $session:expr, (default => $body:tt $($tail:tt)*), ($($head:tt)*), ()) => {
        // A default branch
        // allow missed comma `,`
        $crate::check!(@case $session, ($($tail)*), ($($head)*), ( { $body; Result::<(), $crate::Error>::Ok(()) } ))
    };
    (@case $session:expr, (), ($($head:tt)*), ()) => {
        // there's no default branch
        // so we make up our own.
        $crate::check!(@case $session, (), ($($head)*), ( { Result::<(), $crate::Error>::Ok(()) } ))
    };
    (@case $session:expr, (), ($($tail:tt)*), ($($default:tt)*)) => {
        // last point of @case
        // call code generation via @branch
        $crate::check!(@branch $session, ($($tail)*), ($($default)*))
    };
    // We need to use a variable for pattern mathing,
    // user may chose to drop var name using a placeholder '_',
    // in which case we can't call a method on such identificator.
    //
    // We could use an approach like was described
    //
    // ```
    // Ok(_____random_var_name_which_supposedly_mustnt_be_used) if !_____random_var_name_which_supposedly_mustnt_be_used.is_empty() =>
    // {
    //      let $var = _____random_var_name_which_supposedly_mustnt_be_used;
    // }
    // ```
    //
    // The question is which solution is more effichient.
    // I took the following approach because there's no chance we influence user's land via the variable name we pick.
    (@branch $session:expr, ($var:tt = $exp:expr => $body:tt, $($tail:tt)*), ($($default:tt)*)) => {
        match $crate::session::Session::check(&mut $session, $exp).await {
            result if result.as_ref().map(|found| !found.is_empty()).unwrap_or(false) => {
                let $var = result.unwrap();
                $body;
                #[allow(unreachable_code)]
                Ok(())
            }
            Ok(_) => {
                $crate::check!(@branch $session, ($($tail)*), ($($default)*))
            }
            Err(err) => Err(err),
        }
    };
    (@branch $session:expr, (), ($default:tt)) => {
        // A standart default branch
        $default
    };
    (@branch $session:expr, ($($tail:tt)*), ($($default:tt)*)) => {
        compile_error!(
            concat!(
                "No supported syntax tail=(",
                stringify!($($tail,)*),
                ") ",
                "default=(",
                stringify!($($default,)*),
                ") ",
        ))
    };
    // Entry point
    ($($tokens:tt)*) => {
        async {
            $crate::check!(@check ($($tokens)*) ())
        }
    };
}

#[cfg(test)]
mod tests {
    #[allow(unused_variables)]
    #[allow(unused_must_use)]
    #[test]
    #[ignore = "Testing in compile time"]
    fn test_check() {
        let mut session = crate::spawn("").unwrap();
        crate::check! {
            &mut session,
            as11d = "zxc" => {},
        };
        crate::check! {
            &mut session,
            as11d = "zxc" => {},
            asbb = "zxc123" => {},
        };
        crate::check! { session, };

        // crate::check! {
        //     as11d = "zxc" => {},
        //     asbb = "zxc123" => {},
        // };

        // panic on unused session
        // crate::check! { session };

        // trailing commas
        crate::check! {
            &mut session,
            as11d = "zxc" => {}
            asbb = "zxc123" => {}
        };
        crate::check! {
            &mut session,
            as11d = "zxc" => {}
            default => {}
        };

        #[cfg(not(feature = "async"))]
        {
            let _ = crate::check! {
                &mut session,
                as11d = "zxc" => {},
            }
            .unwrap();
            (crate::check! {
                &mut session,
                as11d = "zxc" => {},
            })
            .unwrap();
            (crate::check! {
                &mut session,
                as11d = "zxc" => {},
            })
            .unwrap();
            (crate::check! {
                &mut session,
                as11d = "zxc" => {
                    println!("asd")
                },
            })
            .unwrap();
        }
        #[cfg(feature = "async")]
        async {
            let _ = crate::check! {
                session,
                as11d = "zxc" => {},
            }
            .await
            .unwrap();
            (crate::check! {
                session,
                as11d = "zxc" => {},
            })
            .await
            .unwrap();
            (crate::check! {
                session,
                as11d = "zxc" => {},
            })
            .await
            .unwrap();
            (crate::check! {
                session,
                as11d = "zxc" => {
                    println!("asd")
                },
            })
            .await
            .unwrap();
        };
    }
}
