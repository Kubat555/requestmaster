//! The measuring mode: the window reports how long things took, and [`AppCore`] appends that to a file.

use std::io::Write;

use crate::core::AppCore;
use crate::dto::TimingSample;

impl AppCore {
    /// Appends `name<TAB>ms` lines to the measure file; without one does nothing.
    pub fn report_timings(&self, samples: Vec<TimingSample>) -> Result<(), String> {
        let Some(path) = &self.measure_file else {
            return Ok(());
        };
        // One write for the whole batch, so that a reader of the file never sees half a line.
        let lines: String = samples.iter().map(|sample| format!("{}\t{}\n", sample.name, sample.ms)).collect();
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| file.write_all(lines.as_bytes()))
            .map_err(|error| format!("cannot write the timings: {error}"))
    }
}
