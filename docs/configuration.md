# Configuration Reference

The config file is `<data_dir>/config/realization.json`. The format is compatible with [ngen](https://github.com/NOAA-OWP/ngen) realization configs -- extra ngen-specific fields are silently ignored.

## Top-level fields

| Field | Required | Description |
|-------|----------|-------------|
| `global.formulations` | yes | Array with one `bmi_multi` formulation entry |
| `global.forcing.path` | yes | Path to NetCDF forcing file (relative to data_dir) |
| `time.start_time` | yes | Simulation start, `"YYYY-MM-DD HH:MM:SS"` |
| `time.end_time` | yes | Simulation end, `"YYYY-MM-DD HH:MM:SS"` |
| `time.output_interval` | no | Output interval in seconds (default: 3600) |

## Formulation fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | yes | Must be `"bmi_multi"` for multi-model orchestration |
| `params.modules` | yes | Array of module configs (see below) |
| `params.main_output_variable` | no | Primary output variable name |
| `params.output_variables` | no | Variables to include in output. If empty, all model outputs are written. |

## Module fields (shared across all adapter types)

| Field | Required | Description |
|-------|----------|-------------|
| `name` | yes | Adapter type: `"bmi_c"`, `"bmi_fortran"`, `"bmi_python"`, or `"bmi_c++"` |
| `params.model_type_name` | yes | Model identifier (e.g., `"CFE"`, `"NoahOWP"`, `"SLOTH"`) |
| `params.init_config` | yes | Path to model config file. `{{id}}` is replaced with the location ID at runtime. |
| `params.main_output_variable` | yes | The primary output variable of this model |
| `params.library_file` | depends | Path to shared library. Required for C and Fortran adapters. |
| `params.registration_function` | no | Name of the C registration function (default: `"register_bmi"`) |
| `params.variables_names_map` | no | Maps model input names to source variable names (from forcings or upstream models) |
| `params.model_params` | no | Parameters set on the model after initialization via `set_value()`. Values may be numbers, numeric strings, or arrays of numbers (for array variables such as per-soil-layer parameters). A single value is broadcast to every element of an array variable; any other length mismatch is refused with a warning, as is a failed `set_value`. Per-location values can be given in `catchments` (below). |
| `params.python_type` | no | Python class path for the Python adapter (e.g., `"lstm.bmi_lstm.bmi_LSTM"`) |
| `params.use_value_ptr` | no | Default `true`. Couple variables by reading/writing the model's memory via `get_value_ptr` (checked against `get_value`/`set_value` on the first timestep). Set `false` if the model's `set_value`/`get_value` do more than copy memory. |

## Per-location parameters (`catchments`)

To give each location its own parameter values (e.g. for calibration), add a top-level
`catchments` block in ngen's format. For each location listed, every module's `model_params` is
merged over the global module with the same `model_type_name`: keys given here replace the global
value, other global keys are kept. Everything else (libraries, `init_config`, variable mappings,
forcing) comes from `global`; other ngen keys in the block, such as `forcing`, are ignored.
Locations not listed use the global modules unchanged. A module here whose `model_type_name` isn't
in the global formulation is an error.

```json
"catchments": {
  "cat-27": {"formulations": [{"name": "bmi_multi", "params": {"modules": [
    {"name": "bmi_c", "params": {"model_type_name": "CFE",
                                 "model_params": {"b": 6.2, "satdk": 3.4e-6}}},
    {"name": "bmi_fortran", "params": {"model_type_name": "NoahOWP",
                                       "model_params": {"SCAMAX": 0.92, "BEXP": [5.1, 5.1, 5.1, 5.1]}}}
  ]}}]}
}
```

## Library usage

```rust
use bmi_driver::{ModelRunner, BmiError};

fn main() -> Result<(), BmiError> {
    let mut runner = ModelRunner::from_config("config/realization.json")?;
    runner.initialize("cat-123")?;
    runner.run()?;

    let outputs = runner.main_outputs()?;
    println!("Completed {} timesteps", outputs.len());

    runner.finalize()?;
    Ok(())
}
```
