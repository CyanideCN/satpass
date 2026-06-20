use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::orbital::{Orbital, SatPassEvent, TCSatPassEvent};
use crate::{TcPassOptions, TleCatalog, find_tc_passes_from_bdeck_file, find_tc_passes_from_track};

fn py_value_error(error: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(error.to_string())
}

#[pyclass(name = "SatPassEvent")]
pub struct PySatPassEvent {
    #[pyo3(get)]
    cpa_time: f64,
    #[pyo3(get)]
    cpa_distance: f64,
    #[pyo3(get)]
    elevation: f64,
}

impl PySatPassEvent {
    fn from_event(event: &SatPassEvent) -> Self {
        Self {
            cpa_time: event.cpa_time,
            cpa_distance: event.cpa_distance,
            elevation: event.elevation,
        }
    }
}

#[pymethods]
impl PySatPassEvent {
    fn __repr__(&self) -> String {
        format!(
            "SatPassEvent(cpa_distance={}, elevation={})",
            self.cpa_distance, self.elevation
        )
    }
}

#[pyclass(name = "TCSatPassEvent")]
pub struct PyTCSatPassEvent {
    #[pyo3(get)]
    cpa_time: f64,
    #[pyo3(get)]
    cpa_distance: f64,
    #[pyo3(get)]
    sat_zenith: f64,
    #[pyo3(get)]
    intensity: f64,
    #[pyo3(get)]
    tc_lat: f64,
    #[pyo3(get)]
    tc_lon: f64,
}

impl PyTCSatPassEvent {
    fn from_event(event: &TCSatPassEvent) -> Self {
        Self {
            cpa_time: event.cpa_time,
            cpa_distance: event.cpa_distance,
            sat_zenith: event.sat_zenith,
            intensity: event.intensity,
            tc_lat: event.tc_lat,
            tc_lon: event.tc_lon,
        }
    }
}

#[pymethods]
impl PyTCSatPassEvent {
    fn __repr__(&self) -> String {
        format!(
            "TCSatPassEvent(cpa_distance={}, sat_zenith={}, intensity={}, tc_lat={}, tc_lon={})",
            self.cpa_distance, self.sat_zenith, self.intensity, self.tc_lat, self.tc_lon
        )
    }
}

#[pyclass(name = "Orbital")]
pub struct PyOrbital {
    orbital: Orbital,
}

#[pymethods]
impl PyOrbital {
    #[staticmethod]
    fn from_tle(line1: &str, line2: &str) -> Self {
        let tle = crate::tle::Tle::from_lines(line1, line2);
        Self {
            orbital: Orbital::new(&tle),
        }
    }

    #[pyo3(signature = (start_time, interval_seconds, longitude, latitude))]
    fn get_passes(
        &self,
        start_time: f64,
        interval_seconds: f64,
        longitude: f64,
        latitude: f64,
    ) -> Vec<PySatPassEvent> {
        self.orbital
            .get_passes(start_time, interval_seconds, longitude, latitude)
            .iter()
            .map(PySatPassEvent::from_event)
            .collect()
    }
}

#[pyclass(name = "TleCatalog")]
pub struct PyTleCatalog {
    catalog: TleCatalog,
}

#[pymethods]
impl PyTleCatalog {
    #[staticmethod]
    fn from_file(path: &str) -> PyResult<Self> {
        Ok(Self {
            catalog: TleCatalog::from_file(path).map_err(py_value_error)?,
        })
    }

    #[staticmethod]
    fn from_tle(line1: &str, line2: &str) -> Self {
        Self {
            catalog: TleCatalog::from_lines(line1, line2),
        }
    }

    #[pyo3(signature = (start_time, interval_seconds, longitude, latitude))]
    fn get_passes(
        &self,
        start_time: f64,
        interval_seconds: f64,
        longitude: f64,
        latitude: f64,
    ) -> Vec<PySatPassEvent> {
        self.catalog
            .select_orbital(start_time)
            .get_passes(start_time, interval_seconds, longitude, latitude)
            .iter()
            .map(PySatPassEvent::from_event)
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (time, longitude, latitude, intensity, step_hours = 6.0, intensity_threshold = 100.0, distance_threshold = 1165.0))]
    fn find_tc_passes(
        &self,
        time: Vec<f64>,
        longitude: Vec<f64>,
        latitude: Vec<f64>,
        intensity: Vec<f64>,
        step_hours: f64,
        intensity_threshold: f64,
        distance_threshold: f64,
    ) -> Vec<PyTCSatPassEvent> {
        let options = TcPassOptions::new(step_hours, intensity_threshold, distance_threshold);
        find_tc_passes_from_track(&self.catalog, time, longitude, latitude, intensity, options)
            .iter()
            .map(PyTCSatPassEvent::from_event)
            .collect()
    }

    #[pyo3(signature = (bdeck_path, step_hours = 6.0, intensity_threshold = 100.0, distance_threshold = 1165.0))]
    fn find_tc_passes_from_bdeck(
        &self,
        bdeck_path: &str,
        step_hours: f64,
        intensity_threshold: f64,
        distance_threshold: f64,
    ) -> PyResult<Vec<PyTCSatPassEvent>> {
        let options = TcPassOptions::new(step_hours, intensity_threshold, distance_threshold);
        Ok(
            find_tc_passes_from_bdeck_file(&self.catalog, bdeck_path, options)
                .map_err(py_value_error)?
                .iter()
                .map(PyTCSatPassEvent::from_event)
                .collect(),
        )
    }
}

#[pyfunction]
#[pyo3(signature = (tle_path, bdeck_path, step_hours = 6.0, intensity_threshold = 100.0, distance_threshold = 1165.0))]
fn find_tc_passes_from_files(
    tle_path: &str,
    bdeck_path: &str,
    step_hours: f64,
    intensity_threshold: f64,
    distance_threshold: f64,
) -> PyResult<Vec<PyTCSatPassEvent>> {
    let catalog = TleCatalog::from_file(tle_path).map_err(py_value_error)?;
    let options = TcPassOptions::new(step_hours, intensity_threshold, distance_threshold);
    Ok(
        find_tc_passes_from_bdeck_file(&catalog, bdeck_path, options)
            .map_err(py_value_error)?
            .iter()
            .map(PyTCSatPassEvent::from_event)
            .collect(),
    )
}

#[pymodule]
fn _satpass(_py: Python<'_>, module: &PyModule) -> PyResult<()> {
    module.add_class::<PyOrbital>()?;
    module.add_class::<PyTleCatalog>()?;
    module.add_class::<PySatPassEvent>()?;
    module.add_class::<PyTCSatPassEvent>()?;
    module.add_function(wrap_pyfunction!(find_tc_passes_from_files, module)?)?;
    Ok(())
}
