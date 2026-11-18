//! The JavaScript files built in `prelude/built/`: compiled to bytecode once per process, loaded into the
//! context of every script.

use rquickjs::convert::Coerced;
use rquickjs::module::WriteOptions;
use rquickjs::promise::PromiseState;
use rquickjs::{Context, Ctx, Error, Exception, FromJs, Module, Runtime, Value};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bundle {
    /// `console` and `pm.*`.
    Prelude,
    /// chai 4.4.1 with the assertions of a response; loaded when a script first asks for it.
    Chai,
}

static PRELUDE_BYTECODE: OnceLock<Vec<u8>> = OnceLock::new();
static CHAI_BYTECODE: OnceLock<Vec<u8>> = OnceLock::new();

impl Bundle {
    fn name(self) -> &'static str {
        match self {
            Bundle::Prelude => "rqm:prelude",
            Bundle::Chai => "rqm:chai",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Bundle::Prelude => include_str!("../prelude/built/prelude.js"),
            Bundle::Chai => include_str!("../prelude/built/chai.js"),
        }
    }

    fn cell(self) -> &'static OnceLock<Vec<u8>> {
        match self {
            Bundle::Prelude => &PRELUDE_BYTECODE,
            Bundle::Chai => &CHAI_BYTECODE,
        }
    }

    /// The bytecode of the bundle; compiled on first use, in an engine of its own. The error is a text: the
    /// exception stays in the engine that is dropped here.
    fn bytecode(self) -> Result<&'static [u8], String> {
        let cell = self.cell();
        if let Some(bytes) = cell.get() {
            return Ok(bytes);
        }
        let runtime = Runtime::new().map_err(|error| error.to_string())?;
        let context = Context::full(&runtime).map_err(|error| error.to_string())?;
        let bytes = context.with(|ctx| {
            Module::declare(ctx.clone(), self.name(), self.source())
                .and_then(|module| {
                    module.write(WriteOptions {
                        strip_source: true,
                        strip_debug: true,
                        ..Default::default()
                    })
                })
                .map_err(|error| match error {
                    Error::Exception => thrown_text(&ctx),
                    other => other.to_string(),
                })
        })?;
        Ok(cell.get_or_init(|| bytes))
    }
}

/// What `String(error)` gives for the exception pending in the context.
fn thrown_text(ctx: &Ctx<'_>) -> String {
    Coerced::<rquickjs::String<'_>>::from_js(ctx, ctx.catch())
        .and_then(|text| text.0.to_string())
        .unwrap_or_else(|_| "unknown error".to_string())
}

/// Makes sure the bytecode of the bundle is there. No code of a script runs here (the compiling engine is
/// a fresh one), so a caller may do it with the stopwatch of the script standing; `load` then only runs
/// the bundle.
pub(crate) fn compile(ctx: &Ctx<'_>, bundle: Bundle) -> rquickjs::Result<()> {
    bytecode(ctx, bundle).map(|_| ())
}

fn bytecode(ctx: &Ctx<'_>, bundle: Bundle) -> rquickjs::Result<&'static [u8]> {
    bundle.bytecode().map_err(|error| {
        Exception::throw_message(ctx, &format!("{} does not compile: {error}", bundle.name()))
    })
}

/// Runs the bundle in the context. The job queue is not driven here: a bundle may be loaded in the middle
/// of a script, and the microtasks of the script must not run ahead of their time. A bundle has no top-level
/// `await`, so the promise of its evaluation is settled at once.
pub(crate) fn load(ctx: &Ctx<'_>, bundle: Bundle) -> rquickjs::Result<()> {
    let bytes = bytecode(ctx, bundle)?;
    // SAFETY: `Module::load` trusts its bytes blindly. These were written by `Module::write` of this very
    // engine in this very process (`Bundle::bytecode`) and are never read from a disk or from anywhere else.
    let module = unsafe { Module::load(ctx.clone(), bytes) }?;
    let (_, promise) = module.eval()?;
    match promise.state() {
        PromiseState::Resolved => Ok(()),
        PromiseState::Rejected => match promise.result::<Value<'_>>() {
            Some(Err(error)) => Err(error),
            _ => Err(not_loaded(ctx, bundle)),
        },
        PromiseState::Pending => Err(not_loaded(ctx, bundle)),
    }
}

fn not_loaded(ctx: &Ctx<'_>, bundle: Bundle) -> Error {
    Exception::throw_message(ctx, &format!("{} did not finish loading", bundle.name()))
}
