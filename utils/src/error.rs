use std::fmt::Display;
use std::process::Termination;
use std::ops::ControlFlow;

#[derive(Clone, Copy)]
pub struct ExitCode {
    pub code: i32,
}

impl ExitCode {
    pub const SUCCESS: Self = Self { code: 0 };
    pub const FAILURE: Self = Self { code: 1 };

    #[inline(always)]
    pub const fn success(self) -> bool {
        self.code == 0
    }

    #[inline(always)]
    pub const fn failure(self) -> bool {
        self.code != 0
    }

    #[inline(always)]
    pub const fn new(code: i32) -> ExitCode {
        Self { code }
    }

    /// Run the `f` function and returns its exit code if the current code indicates success (0).
    /// On other situations, don't run the function at all, returning the current code.
    pub fn and_then<F>(self, f: F) -> Self
    where
        F: Fn() -> ExitCode,
    {
        if self.success() {
            f()
        } else {
            self
        }
    }

    /// Run the `f` function, passing the current code, and returns its exit code if the current code indicates failure (anything except 0).
    /// If the code indicates success, don't run the function at all, returning the current code.
    pub fn or_else<F>(self, f: F) -> Self
    where
        F: Fn(i32) -> ExitCode,
    {
        if self.failure() {
            self
        } else {
            f(self.code)
        }
    }
}

impl Termination for ExitCode {
    fn report(self) -> std::process::ExitCode {
        std::process::ExitCode::from(self.code as u8)
    }
}

pub enum CliError {
    /// Indicates an error that doesn't show anything on the screen.
    Silent,
    /// Indicates an error that shows something on the screen.
    Display(Box<dyn Display + 'static>),
}

impl CliError {
    pub fn from_display<D>(display: D) -> Self
    where
        D: Display + 'static,
    {
        Self::Display(Box::new(display))
    }
}

pub struct CliResult<T = ()> {
    pub inner: Result<T, CliError>,
}

impl CliResult<()> {
    pub const EMPTY_OK: Self = Self { inner: Ok(()) };
}

impl<T> CliResult<T> {
    pub const fn ok(value: T) -> Self {
        Self { inner: Ok(value) }
    }

    pub const fn silent_err() -> Self {
        Self {
            inner: Err(CliError::Silent),
        }
    }

    pub const fn new(inner: Result<T, CliError>) -> Self {
        Self { inner }
    }

    pub fn display_err<D>(display: D) -> Self
    where
        D: Display + 'static,
    {
        Self {
            inner: Err(CliError::Display(Box::new(display))),
        }
    }

    pub fn from_display_result<D>(result: Result<T, D>) -> Self
    where
        D: Display + 'static,
    {
        match result {
            Ok(val) => Self { inner: Ok(val) },
            Err(display) => Self {
                inner: Err(CliError::Display(Box::new(display))),
            },
        }
    }

    /// Process the current value and return an according exit code.
    ///
    /// Might display things to the stderr if needed.
    pub fn process(&self) -> ExitCode {
        match self.inner {
            Ok(_) => ExitCode::SUCCESS,
            Err(CliError::Silent) => ExitCode::FAILURE,
            Err(CliError::Display(ref why)) => {
                eprintln!("Error: {}", why);

                ExitCode::FAILURE
            }
        }
    }
}

impl<T> From<Result<T, CliError>> for CliResult<T> {
    fn from(result: Result<T, CliError>) -> Self {
        Self { inner: result }
    }
}

impl<T> Into<Result<T, CliError>> for CliResult<T> {
    fn into(self) -> Result<T, CliError> {
        self.inner
    }
}

impl<T> std::ops::FromResidual<CliError> for CliResult<T> {
    fn from_residual(residual: CliError) -> Self {
        Self { inner: Err(residual) }
    }
}

impl<T> std::ops::Try for CliResult<T> {
    type Output = T;
    type Residual = CliError;

    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self.inner {
            Ok(x) => ControlFlow::Continue(x),
            Err(x) => ControlFlow::Break(x),
        }
    }

    fn from_output(output: T) -> Self {
        Self { inner: Ok(output) }
    }
}
