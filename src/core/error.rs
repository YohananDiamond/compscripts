use std::fmt::Display;
use std::process::Termination;

/// TODO
#[derive(Clone, Copy)]
pub struct ExitCode(pub i32);

impl ExitCode {
    pub fn and<F>(self, f: F) -> Self
    where
        F: Fn() -> ExitCode,
    {
        if self.0 == 0 {
            f()
        } else {
            self
        }
    }
}

impl Termination for ExitCode {
    fn report(self) -> i32 {
        self.0
    }
}

/// An enum used for the different kinds of CLI app results.
pub enum ExitResult<'a> {
    /// Indicates success.
    Ok,
    /// Indicates an error that doesn't show anything on the screen.
    SilentErr,
    /// Indicates an error that shows something on the screen.
    DisplayErr(Box<dyn Display + 'a>),
}

impl<'a> ExitResult<'a> {
    pub fn from_display_result<T: Display + 'a>(thing: Result<(), T>) -> Self {
        match thing {
            Ok(()) => ExitResult::Ok,
            Err(e) => ExitResult::from(e),
        }
    }
}

impl From<ExitResult<'_>> for ExitCode {
    fn from(r: ExitResult) -> ExitCode {
        match r {
            ExitResult::Ok => ExitCode(0),
            ExitResult::SilentErr => ExitCode(1),
            ExitResult::DisplayErr(e) => {
                eprintln!("Error: {}", e);
                ExitCode(1)
            }
        }
    }
}

impl<'a, T: Display + 'a> From<T> for ExitResult<'a> {
    fn from(thing: T) -> ExitResult<'a> {
        ExitResult::DisplayErr(Box::new(thing))
    }
}
