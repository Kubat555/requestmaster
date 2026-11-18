//! Text on the border between JavaScript and Rust: the only place where text is cut, and the reading of a
//! JavaScript string that may hold a lone surrogate.

use rquickjs::function::This;
use rquickjs::{Ctx, Function, Object};

/// Stands for a text of the script that could not be read at all.
pub(crate) const UNREADABLE: &str = "<unreadable text>";

/// The longest prefix of `text` of at most `max_chars` Unicode scalar values.
pub(crate) fn clip(text: &str, max_chars: usize) -> &str {
    match text.char_indices().nth(max_chars) {
        Some((end, _)) => text.get(..end).unwrap_or(text),
        None => text,
    }
}

/// `String.prototype.toWellFormed` of the context. Taken before the text of the script runs, so the script
/// cannot replace it.
pub(crate) fn well_formed_function<'js>(ctx: &Ctx<'js>) -> Option<Function<'js>> {
    let string: Object<'js> = ctx.globals().get("String").ok()?;
    let prototype: Object<'js> = string.get("prototype").ok()?;
    prototype.get("toWellFormed").ok()
}

/// Reads a JavaScript string. A string with a lone surrogate does not turn into a Rust string as it is: it goes
/// through `toWellFormed` first (the surrogate becomes U+FFFD).
pub(crate) fn read<'js>(
    string: &rquickjs::String<'js>,
    well_formed: Option<&Function<'js>>,
) -> String {
    if let Ok(text) = string.to_string() {
        return text;
    }
    let repaired = well_formed
        .and_then(|function| {
            function
                .call::<_, rquickjs::String<'js>>((This(string.clone()),))
                .ok()
        })
        .and_then(|repaired| repaired.to_string().ok());
    match repaired {
        Some(text) => text,
        None => {
            // A failed call leaves its exception pending in the context.
            let _ = string.ctx().catch();
            UNREADABLE.to_string()
        }
    }
}
