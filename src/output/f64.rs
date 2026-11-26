//! Lossless binary output: the f64 values exactly as the driver collected them.
//!
//! The other formats round values (CSV prints `{:.9}`; NetCDF, Zarr and Parquet store f32),
//! which is fine for analysis but not for bit-for-bit validation of a model re-implementation.
//! Per location this writes two files into `output_root`:
//!
//! - `<loc>.f64`: little-endian f64, variable-major: all steps of the first column, then all
//!   steps of the second, and so on (`numpy.fromfile(p, "<f8").reshape(n_vars, n_steps)`).
//! - `<loc>.json`: `{"columns": [...], "n_steps": N, "start_epoch": s, "interval": dt}`.

use std::fs;
use std::path::PathBuf;

use crate::error::{function_failed, BmiResult};

pub struct F64Store {
    output_path: PathBuf,
    start_epoch: i64,
    interval: i64,
}

impl F64Store {
    pub fn new(output_path: PathBuf, start_epoch: i64, interval: i64) -> Self {
        Self {
            output_path,
            start_epoch,
            interval,
        }
    }
}

impl super::DivideDataStore for F64Store {
    fn write_location(&mut self, loc_id: &str, columns: &[(String, Vec<f64>)]) -> BmiResult<()> {
        let n_steps = columns.first().map(|(_, v)| v.len()).unwrap_or(0);
        if let Some((name, v)) = columns.iter().find(|(_, v)| v.len() != n_steps) {
            return Err(function_failed(
                "f64_writer",
                format!("{loc_id}: column {name} has {} steps, expected {n_steps}", v.len()),
            ));
        }

        let mut bytes = Vec::with_capacity(columns.len() * n_steps * 8);
        for (_, vals) in columns {
            for v in vals {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        let names: Vec<&str> = columns.iter().map(|(n, _)| n.as_str()).collect();
        let meta = serde_json::json!({
            "columns": names,
            "n_steps": n_steps,
            "start_epoch": self.start_epoch,
            "interval": self.interval,
        });

        let write = |ext: &str, data: &[u8]| {
            let p = self.output_path.join(format!("{loc_id}.{ext}"));
            fs::write(&p, data).map_err(|e| {
                function_failed("f64_writer", format!("Failed to write {}: {}", p.display(), e))
            })
        };
        write("f64", &bytes)?;
        write("json", meta.to_string().as_bytes())?;
        Ok(())
    }
}
