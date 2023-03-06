use std::borrow::Cow;

use super::Context;
use crate::{session::Proc, Error, Session};

type Result<T> = std::result::Result<T, Error>;

/// Interact options (aka callbacks you can set to be callled being in an interactive mode).
#[derive(Debug)]
pub struct InteractOptions<C, IF, OF, IA, OA, WA> {
    pub(crate) state: C,
    pub(crate) input_filter: Option<IF>,
    pub(crate) output_filter: Option<OF>,
    pub(crate) input_action: Option<IA>,
    pub(crate) output_action: Option<OA>,
    pub(crate) idle_action: Option<WA>,
}

type DefaultOps<S, I, O, C> = InteractOptions<
    C,
    NoFilter,
    NoFilter,
    NoAction<Session<Proc, S>, I, O, C>,
    NoAction<Session<Proc, S>, I, O, C>,
    NoAction<Session<Proc, S>, I, O, C>,
>;

impl<S, I, O> Default for DefaultOps<S, I, O, ()> {
    fn default() -> Self {
        Self::new(())
    }
}

impl<S, I, O, C> DefaultOps<S, I, O, C> {
    /// Set a state.
    pub fn new(state: C) -> Self {
        Self {
            state,
            input_filter: None,
            output_filter: None,
            input_action: None,
            output_action: None,
            idle_action: None,
        }
    }
}

impl<C, IF, OF, IA, OA, WA> InteractOptions<C, IF, OF, IA, OA, WA> {
    /// Get a reference on state
    pub fn get_state(&self) -> &C {
        &self.state
    }

    /// Get a mut reference on state
    pub fn get_state_mut(&mut self) -> &mut C {
        &mut self.state
    }

    /// Returns a inner state.
    pub fn into_inner(self) -> C {
        self.state
    }

    /// Sets the output filter.
    /// The output_filter will be passed all the output from the child process.
    ///
    /// The filter isn't applied to user's `read` calls through the [`Context`] in callbacks.
    pub fn output_filter<F>(self, filter: F) -> InteractOptions<C, IF, F, IA, OA, WA>
    where
        F: FnMut(&[u8]) -> Result<Cow<'_, [u8]>>,
    {
        InteractOptions {
            input_filter: self.input_filter,
            output_filter: Some(filter),
            input_action: self.input_action,
            output_action: self.output_action,
            idle_action: self.idle_action,
            state: self.state,
        }
    }

    /// Sets the input filter.
    /// The input_filter will be passed all the keyboard input from the user.
    ///
    /// The input_filter is run BEFORE the check for the escape_character.
    /// The filter is called BEFORE calling a on_input callback if it's set.
    pub fn input_filter<F>(self, filter: F) -> InteractOptions<C, F, OF, IA, OA, WA>
    where
        F: FnMut(&[u8]) -> Result<Cow<'_, [u8]>>,
    {
        InteractOptions {
            input_filter: Some(filter),
            output_filter: self.output_filter,
            input_action: self.input_action,
            output_action: self.output_action,
            idle_action: self.idle_action,
            state: self.state,
        }
    }
}

impl<S, I, O, C, IF, OF, OA, WA>
    InteractOptions<C, IF, OF, NoAction<Session<Proc, S>, I, O, C>, OA, WA>
{
    /// Puts a hanlder which will be called when users input is detected.
    ///
    /// Be aware that currently async version doesn't take a Session as an argument.
    /// See <https://github.com/zhiburt/expectrl/issues/16>.
    pub fn on_input<F>(self, action: F) -> InteractOptions<C, IF, OF, F, OA, WA>
    where
        F: FnMut(Context<'_, Session<Proc, S>, I, O, C>) -> Result<bool>,
    {
        InteractOptions {
            input_filter: self.input_filter,
            output_filter: self.output_filter,
            input_action: Some(action),
            output_action: self.output_action,
            idle_action: self.idle_action,
            state: self.state,
        }
    }
}

impl<S, I, O, C, IF, OF, IA, WA>
    InteractOptions<C, IF, OF, IA, NoAction<Session<Proc, S>, I, O, C>, WA>
{
    /// Puts a hanlder which will be called when process output is detected.
    ///
    /// IMPORTANT:
    ///
    /// Please be aware that your use of [Session::expect], [Session::check] and any `read` operation on session
    /// will cause the read bytes not to apeard in the output stream!
    pub fn on_output<F>(self, action: F) -> InteractOptions<C, IF, OF, IA, F, WA>
    where
        F: FnMut(Context<'_, Session<Proc, S>, I, O, C>) -> Result<bool>,
    {
        InteractOptions {
            input_filter: self.input_filter,
            output_filter: self.output_filter,
            input_action: self.input_action,
            output_action: Some(action),
            idle_action: self.idle_action,
            state: self.state,
        }
    }
}

impl<S, I, O, C, IF, OF, IA, OA>
    InteractOptions<C, IF, OF, IA, OA, NoAction<Session<Proc, S>, I, O, C>>
{
    /// Puts a handler which will be called on each interaction when no input is detected.
    pub fn on_idle<F>(self, action: F) -> InteractOptions<C, IF, OF, IA, OA, F>
    where
        F: FnMut(Context<'_, Session<Proc, S>, I, O, C>) -> Result<bool>,
    {
        InteractOptions {
            input_filter: self.input_filter,
            output_filter: self.output_filter,
            input_action: self.input_action,
            output_action: self.output_action,
            idle_action: Some(action),
            state: self.state,
        }
    }
}

/// A helper type to set a default action to [`InteractSession`].
pub type NoAction<S, I, O, C> = fn(Context<'_, S, I, O, C>) -> Result<bool>;

/// A helper type to set a default filter to [`InteractSession`].
pub type NoFilter = fn(&[u8]) -> Result<Cow<'_, [u8]>>;
