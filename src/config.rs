use crate::error::{BmiError, BmiResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// How to resample when the source has a coarser timestep than the destination.
/// Example: daily source feeding an hourly model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownsampleMode {
    /// Repeat the source value for all sub-steps (sample-and-hold)
    Repeat,
    /// Linearly interpolate between consecutive source values
    Interpolate,
}

impl Default for DownsampleMode {
    fn default() -> Self {
        Self::Repeat
    }
}

/// How to resample when the source has a finer timestep than the destination.
/// Example: 5-minute source feeding an hourly model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpsampleMode {
    /// Arithmetic mean of all source values in the window
    Mean,
    /// Most frequent value (for categorical/integer data)
    Mode,
    /// Minimum value in the window
    Min,
    /// Maximum value in the window
    Max,
}

impl Default for UpsampleMode {
    fn default() -> Self {
        Self::Mean
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Csv,
    Netcdf,
    #[cfg(feature = "zarr")]
    Zarr,
    #[cfg(feature = "parquet")]
    Parquet,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Csv
    }
}

fn is_default_output_format(f: &OutputFormat) -> bool {
    *f == OutputFormat::Csv
}

fn output_root_default() -> String {
    "outputs/bmi-driver".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RealizationConfig {
    pub global: GlobalConfig,
    pub time: TimeConfig,
    #[serde(
        default = "output_root_default",
        skip_serializing_if = "String::is_empty"
    )]
    pub output_root: String,
    #[serde(default, skip_serializing_if = "is_default_output_format")]
    pub output_format: OutputFormat,
    /// Per-location overrides, in ngen's realization format:
    /// `"catchments": { "cat-1": { "formulations": [ { "name": "bmi_multi", "params": { "modules": [...] } } ] } }`.
    /// Only each module's `model_params` are taken from here (merged over the global module with
    /// the same `model_type_name`); everything else comes from `global`. See `modules_for`.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub catchments: HashMap<String, CatchmentConfig>,
}

/// One entry of the `catchments` block. Other ngen keys (e.g. `forcing`) are accepted and ignored.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CatchmentConfig {
    #[serde(default)]
    pub formulations: Vec<FormulationConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalConfig {
    pub formulations: Vec<FormulationConfig>,
    pub forcing: ForcingConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TimeConfig {
    pub start_time: String,
    pub end_time: String,
    #[serde(default = "default_interval")]
    pub output_interval: i64,
}

fn default_interval() -> i64 {
    3600
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ForcingConfig {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FormulationConfig {
    pub name: String,
    pub params: FormulationParams,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FormulationParams {
    #[serde(default, skip_serializing)]
    pub name: String,
    #[serde(default, skip_serializing)]
    pub model_type_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub main_output_variable: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_variables: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<ModuleConfig>,
    #[serde(default, skip_serializing)]
    pub library_file: String,
    #[serde(default, skip_serializing)]
    pub init_config: String,
    #[serde(default, skip_serializing)]
    pub registration_function: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub variables_names_map: HashMap<String, String>,
    #[serde(default, skip_serializing)]
    pub model_params: HashMap<String, f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModuleConfig {
    pub name: String,
    pub params: ModuleParams,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModuleParams {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model_type_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub library_file: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub init_config: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub registration_function: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub main_output_variable: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub variables_names_map: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub model_params: HashMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub python_type: String,
    #[serde(default, skip_serializing)]
    pub allow_exceed_end_time: bool,
    #[serde(default, skip_serializing)]
    pub fixed_time_step: bool,
    #[serde(default, skip_serializing)]
    pub uses_forcing_file: bool,
    #[serde(default, skip_serializing_if = "is_default_downsample")]
    pub downsample_mode: DownsampleMode,
    #[serde(default, skip_serializing_if = "is_default_upsample")]
    pub upsample_mode: UpsampleMode,
    /// Read/write coupled variables through the model's `get_value_ptr` when it provides one.
    /// Set to `false` for models whose `set_value`/`get_value` do more than copy memory.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub use_value_ptr: bool,
}

fn default_true() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}

fn is_default_downsample(m: &DownsampleMode) -> bool {
    *m == DownsampleMode::Repeat
}
fn is_default_upsample(m: &UpsampleMode) -> bool {
    *m == UpsampleMode::Mean
}

impl ModuleParams {
    pub fn params_f64(&self) -> HashMap<String, f64> {
        self.model_params
            .iter()
            .filter_map(|(k, v)| {
                let val = match v {
                    serde_json::Value::Number(n) => n.as_f64(),
                    serde_json::Value::String(s) => s.parse().ok(),
                    _ => None,
                };
                val.map(|f| (k.clone(), f))
            })
            .collect()
    }

    /// Numeric `model_params` as value lists: a number (or numeric string) gives one value, a JSON
    /// array of numbers gives all of them (for array variables such as per-soil-layer parameters).
    /// Entries that aren't numeric are returned in the second list so callers can report them.
    pub fn param_values(&self) -> (Vec<(String, Vec<f64>)>, Vec<String>) {
        let mut vals = Vec::new();
        let mut skipped = Vec::new();
        let mut keys: Vec<&String> = self.model_params.keys().collect();
        keys.sort();
        for k in keys {
            let v = &self.model_params[k];
            let parsed = match v {
                serde_json::Value::Number(n) => n.as_f64().map(|f| vec![f]),
                serde_json::Value::String(s) => s.parse().ok().map(|f| vec![f]),
                serde_json::Value::Array(a) => a.iter().map(|x| x.as_f64()).collect::<Option<Vec<f64>>>(),
                _ => None,
            };
            match parsed {
                Some(p) if !p.is_empty() => vals.push((k.clone(), p)),
                _ => skipped.push(k.clone()),
            }
        }
        (vals, skipped)
    }

    pub fn params_string(&self) -> HashMap<String, String> {
        self.model_params
            .iter()
            .map(|(k, v)| {
                let s = match v {
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                (k.clone(), s)
            })
            .collect()
    }

    pub fn init_config(&self, loc_id: &str) -> String {
        self.init_config.replace("{{id}}", loc_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmiAdapterType {
    C,
    Fortran,
    #[cfg(feature = "python")]
    Python,
}

impl BmiAdapterType {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "bmi_c" => Some(Self::C),
            "bmi_fortran" => Some(Self::Fortran),
            #[cfg(feature = "python")]
            "bmi_python" => Some(Self::Python),
            _ => None,
        }
    }
}

impl RealizationConfig {
    pub fn from_file(path: impl AsRef<Path>) -> BmiResult<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|e| BmiError::ConfigNotFound {
            path: format!("{}: {}", path.display(), e),
        })?;
        serde_json::from_str(&content).map_err(|e| BmiError::FunctionFailed {
            model: "config".into(),
            func: format!("parse: {}", e),
        })
    }

    pub fn modules(&self) -> Vec<&ModuleConfig> {
        self.global
            .formulations
            .iter()
            .filter(|f| f.name == "bmi_multi")
            .flat_map(|f| f.params.modules.iter())
            .collect()
    }

    /// The modules to run at one location: the global modules, with each module's `model_params`
    /// overlaid by that location's entry in `catchments` (matched by `model_type_name`; keys in
    /// the catchment entry replace global keys, other global keys are kept). Locations without an
    /// entry get the global modules unchanged.
    pub fn modules_for(&self, loc_id: &str) -> BmiResult<Vec<ModuleConfig>> {
        let mut modules: Vec<ModuleConfig> = self.modules().into_iter().cloned().collect();
        let Some(cat) = self.catchments.get(loc_id) else {
            return Ok(modules);
        };
        for f in cat.formulations.iter().filter(|f| f.name == "bmi_multi") {
            for over in &f.params.modules {
                let target = modules
                    .iter_mut()
                    .find(|m| m.params.model_type_name == over.params.model_type_name)
                    .ok_or_else(|| BmiError::FunctionFailed {
                        model: "config".into(),
                        func: format!(
                            "catchments.{}: module '{}' is not in the global formulation",
                            loc_id, over.params.model_type_name
                        ),
                    })?;
                for (k, v) in &over.params.model_params {
                    target.params.model_params.insert(k.clone(), v.clone());
                }
            }
        }
        Ok(modules)
    }

    pub fn main_output(&self) -> Option<&str> {
        self.global
            .formulations
            .first()
            .map(|f| f.params.main_output_variable.as_str())
    }

    /// Resolve which output variables to write.
    ///
    /// - `output_variables: ["all"]` → None (write all available variables)
    /// - `output_variables: [...]`   → Some(list) (write only those)
    /// - `output_variables` empty    → Some(vec![main_output_variable])
    pub fn output_variables(&self) -> Option<Vec<String>> {
        let formulation = self.global.formulations.first()?;
        let output_vars = &formulation.params.output_variables;

        if output_vars.len() == 1 && output_vars[0].eq_ignore_ascii_case("all") {
            None
        } else if output_vars.is_empty() {
            let main = &formulation.params.main_output_variable;
            if main.is_empty() {
                None
            } else {
                Some(vec![main.clone()])
            }
        } else {
            Some(output_vars.clone())
        }
    }

    /// Model outputs the runner must read: every coupled source plus the written outputs.
    /// None means every output is needed (`output_variables: ["all"]`).
    pub fn required_outputs(&self) -> Option<HashSet<String>> {
        let mut needed: HashSet<String> = self.output_variables()?.into_iter().collect();
        if let Some(main) = self.main_output().filter(|m| !m.is_empty()) {
            needed.insert(main.to_string());
        }
        for module in self.modules() {
            needed.extend(module.params.variables_names_map.values().cloned());
        }
        Some(needed)
    }
}

pub fn minify_file(path: &Path) -> BmiResult<()> {
    let config = RealizationConfig::from_file(path)?;
    let json = serde_json::to_string_pretty(&config).map_err(|e| BmiError::FunctionFailed {
        model: "config".into(),
        func: format!("serialize: {}", e),
    })?;
    fs::write(path, json).map_err(|e| BmiError::FunctionFailed {
        model: "config".into(),
        func: format!("write {}: {}", path.display(), e),
    })?;
    Ok(())
}

pub fn parse_datetime(s: &str) -> BmiResult<i64> {
    let p: Vec<&str> = s.split(&[' ', '-', ':'][..]).collect();
    if p.len() != 6 {
        return Err(BmiError::FunctionFailed {
            model: "config".into(),
            func: format!("Invalid datetime: {}", s),
        });
    }

    let parse = |i: usize| -> BmiResult<i64> {
        p[i].parse().map_err(|_| BmiError::FunctionFailed {
            model: "config".into(),
            func: format!("Invalid datetime part: {}", p[i]),
        })
    };

    let (year, month, day) = (parse(0)? as i32, parse(1)? as u32, parse(2)? as u32);
    let (hour, min, sec) = (parse(3)?, parse(4)?, parse(5)?);

    let leap = |y: i32| (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    let mut days: i64 = 0;
    for y in 1970..year {
        days += if leap(y) { 366 } else { 365 };
    }
    for m in 1..month {
        days += days_in_month[(m - 1) as usize] as i64;
        if m == 2 && leap(year) {
            days += 1;
        }
    }
    days += (day - 1) as i64;

    Ok(days * 86400 + hour * 3600 + min * 60 + sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(json: &str) -> RealizationConfig {
        serde_json::from_str(json).unwrap()
    }

    const BASE: &str = r#"{
      "global": {"formulations": [{"name": "bmi_multi", "params": {"main_output_variable": "Q_OUT", "modules": [
        {"name": "bmi_fortran", "params": {"model_type_name": "NoahOWP", "model_params": {"MFSNO": 1.0, "HVT": 5.0}}},
        {"name": "bmi_c", "params": {"model_type_name": "CFE", "model_params": {"b": 4.0, "satdk": 1e-6}}}
      ]}}],
        "forcing": {"path": "./forcings/forcings.nc"}},
      "time": {"start_time": "2020-01-01 00:00:00", "end_time": "2020-01-02 00:00:00"}
    }"#;

    fn params(mods: &[ModuleConfig], model: &str) -> HashMap<String, serde_json::Value> {
        mods.iter().find(|m| m.params.model_type_name == model).unwrap().params.model_params.clone()
    }

    #[test]
    fn no_catchments_block_gives_global_modules() {
        let c = config(BASE);
        assert!(c.catchments.is_empty());
        let m = c.modules_for("cat-1").unwrap();
        assert_eq!(params(&m, "CFE")["b"], 4.0);
    }

    #[test]
    fn catchment_overrides_merge_by_model_type_name() {
        let mut v: serde_json::Value = serde_json::from_str(BASE).unwrap();
        v["catchments"] = serde_json::json!({
            "cat-2": {"formulations": [{"name": "bmi_multi", "params": {"modules": [
                {"name": "bmi_c", "params": {"model_type_name": "CFE", "model_params": {"b": 9.5}}}
            ]}}], "forcing": {"path": "./forcings/forcings.nc", "provider": "NetCDF"}}
        });
        let c: RealizationConfig = serde_json::from_value(v).unwrap();
        let m2 = c.modules_for("cat-2").unwrap();
        assert_eq!(params(&m2, "CFE")["b"], 9.5); // overridden
        assert_eq!(params(&m2, "CFE")["satdk"], 1e-6); // kept from global
        assert_eq!(params(&m2, "NoahOWP")["HVT"], 5.0); // untouched module
        let m1 = c.modules_for("cat-1").unwrap();
        assert_eq!(params(&m1, "CFE")["b"], 4.0); // other locations unchanged
    }

    #[test]
    fn unknown_module_in_catchment_is_an_error() {
        let mut v: serde_json::Value = serde_json::from_str(BASE).unwrap();
        v["catchments"] = serde_json::json!({
            "cat-3": {"formulations": [{"name": "bmi_multi", "params": {"modules": [
                {"name": "bmi_c", "params": {"model_type_name": "TOPMODEL", "model_params": {"x": 1}}}
            ]}}]}
        });
        let c: RealizationConfig = serde_json::from_value(v).unwrap();
        assert!(c.modules_for("cat-3").is_err());
    }

    #[test]
    fn param_values_accepts_numbers_strings_and_arrays() {
        let p: ModuleParams = serde_json::from_value(serde_json::json!({
            "model_type_name": "NoahOWP",
            "model_params": {"a": 1.5, "b": "2.5", "c": [1.0, 2.0, 3.0, 4.0], "d": {"source": "hydrofabric"}}
        }))
        .unwrap();
        let (vals, skipped) = p.param_values();
        let m: HashMap<_, _> = vals.into_iter().collect();
        assert_eq!(m["a"], vec![1.5]);
        assert_eq!(m["b"], vec![2.5]);
        assert_eq!(m["c"], vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(skipped, vec!["d".to_string()]);
    }
}
