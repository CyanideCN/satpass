use geographiclib_rs::{Geodesic, InverseGeodesic};
use predict_rs::observer::*;
use predict_rs::orbit::*;
use predict_rs::predict::*;
use sgp4::{Constants, Elements};

use crate::tle;

fn geodesic_distance(
    geod: &Geodesic,
    lat1: f64,
    lon1: f64,
    lat2: f64,
    lon2: f64,
) -> f64 {
    let s12: f64 = geod.inverse(lat1, lon1, lat2, lon2);
    s12 / 1000.0 // Convert meters to kilometers
}

fn observe_orbit(oe: &ObserverElements, time: f64) -> (PredictPosition, PredictObservation) {
    let orbit = predict_orbit(oe.elements, oe.constants, time).unwrap();
    let obs = predict_observe_orbit(oe.observer, &orbit);
    (orbit, obs)
}

fn elevation_derivative(oe: &ObserverElements, time: f64) -> f64 {
    let (_, obs) = observe_orbit(oe, time);
    obs.elevation_rate
}

// Modified from predict_rs original function to return time of max elevation
fn find_max_elevation(
    oe: &ObserverElements,
    lower_time: f64,
    upper_time: f64,
) -> (f64, f64, PredictPosition) {
    let mut iteration = 0u32;
    let mut lower_time = lower_time;
    let mut upper_time = upper_time;
    let mut lower_deriv = elevation_derivative(oe, lower_time);
    let mut upper_deriv = elevation_derivative(oe, upper_time);
    let mut max_ele_time_candidate = (upper_time + lower_time) / 2.0;
    let (mut orbit, mut obs) = observe_orbit(oe, max_ele_time_candidate);
    while ((lower_time - upper_time).abs() > 1e-6) && (iteration < 10000) {
        // calculate derivatives for candidate
        let candidate_deriv = obs.elevation_rate;

        //check whether derivative has changed sign
        if candidate_deriv * lower_deriv < 0.0 {
            upper_time = max_ele_time_candidate;
            upper_deriv = candidate_deriv;
        } else if candidate_deriv * upper_deriv < 0.0 {
            lower_time = max_ele_time_candidate;
            lower_deriv = candidate_deriv;
        } else {
            break;
        }
        iteration += 1;
        max_ele_time_candidate = (upper_time + lower_time) / 2.0;
        (orbit, obs) = observe_orbit(oe, max_ele_time_candidate);
    }

    let max_elev = obs.elevation.to_degrees();
    (max_elev, max_ele_time_candidate, orbit)
}

fn build_passes(
    oe: &ObserverElements,
    start_utc: f64,
    stop_utc: f64,
    include_max_elevation: bool,
) -> Passes {
    let (_, obs) = observe_orbit(oe, start_utc);
    let satellite_el = obs.elevation.to_degrees();
    let mut currtime = start_utc;
    let mut passes = vec![];
    let min_elev_deg = oe.observer.min_elevation;
    let coarse_step_sec = 10.0;
    let fine_step_sec = 1.0;
    let band_deg = 5.0;

    if satellite_el.abs() >= min_elev_deg {
        // Already in a pass, find AOS by going backwards in time
        let (_, real_aos) = step_pass(oe, currtime, &StepPassDirection::NegativeDirection).unwrap();
        currtime = real_aos - 1.0;
    }
    'outer: loop {
        let mut pass = Pass {
            aos: None,
            los: None,
            satellite_position_at_aos: None,
            satellite_position_at_los: None,
            max_elevation: None,
        };
        // rough time step until AOS
        loop {
            if currtime >= stop_utc {
                break 'outer;
            }
            let (_, obs) = observe_orbit(oe, currtime);
            let satellite_el = obs.elevation.to_degrees();
            if satellite_el >= min_elev_deg && obs.elevation_rate > 0.0 {
                currtime -= fine_step_sec;
                let (satpos, observation, _) =
                    refine_obs_elevation(oe, currtime, &RefineMode::AOS).unwrap();
                pass.aos = Some(observation);
                pass.satellite_position_at_aos = Some(satpos);
                currtime += fine_step_sec;
                break;
            }
            let step = if (satellite_el - min_elev_deg).abs() > band_deg {
                coarse_step_sec
            } else {
                fine_step_sec
            };
            currtime += step;
        }
        if pass.aos.is_none() {
            println!("Shouldn't be here");
        }
        // now find LOS
        loop {
            let (_, obs) = observe_orbit(oe, currtime);
            let satellite_el = obs.elevation.to_degrees();
            if satellite_el <= min_elev_deg && obs.elevation_rate < 0.0 {
                currtime -= fine_step_sec;
                let (satpos, observation, _) =
                    refine_obs_elevation(oe, currtime, &RefineMode::LOS).unwrap();
                pass.los = Some(observation);
                pass.satellite_position_at_los = Some(satpos);
                currtime += fine_step_sec;
                break;
            }
            let step = if (satellite_el - min_elev_deg).abs() > band_deg {
                coarse_step_sec
            } else {
                fine_step_sec
            };
            currtime += step;
            if currtime >= stop_utc {
                break;
            }
        }
        if pass.aos.is_some() && pass.los.is_some() {
            if include_max_elevation {
                let (maxel_obs, _, _) = find_max_elevation(
                    oe,
                    pass.aos.as_ref().expect("already checked").time,
                    pass.los.as_ref().expect("already checked").time,
                );
                pass.max_elevation = Some(maxel_obs);
            }
        }
        passes.push(pass);
    }
    Passes { passes }
}

#[derive(Debug, Clone)]
pub struct SatPassEvent {
    pub cpa_time: f64,
    pub cpa_distance: f64,
    pub elevation: f64,
}

#[derive(Debug, Clone)]
pub struct TCSatPassEvent {
    pub cpa_time: f64,
    pub cpa_distance: f64,
    pub sat_zenith: f64,
    pub intensity: f64,
}

pub struct Orbital{
    elements: Elements,
    constants: Constants,
}

impl Orbital {
    pub fn new(tle: &tle::TLE) -> Self {
        let elements = Elements::from_tle(
            None,
            tle.line1.as_bytes(),
            tle.line2.as_bytes(),
        ).expect("Failed to parse TLE");
        let constants = Constants::from_elements(&elements).unwrap();
        Self {
            elements,
            constants,
        }
    }

    pub fn get_passes(&self, start_utc: f64, interval_sec: f64, longitude: f64, latitude: f64) -> Vec<SatPassEvent> {
        let geod = Geodesic::wgs84();
        let latitude_rad = latitude.to_radians();
        let longitude_rad = longitude.to_radians();
        let oe = ObserverElements {
            elements: &self.elements,
            constants: &self.constants,
            observer: &PredictObserver {
                name: "Observer".to_string(),
                latitude: latitude_rad,
                longitude: longitude_rad,
                altitude: 0.,                 // meters
                min_elevation: 0.0
            },
        };

        let passes = build_passes(&oe, start_utc, start_utc + interval_sec, false);
        let mut pass_events: Vec<SatPassEvent> = Vec::with_capacity(passes.passes.len());

        for pass in passes.passes.iter() {
            // Filter out passes without AOS or LOS
            if pass.aos.is_none() || pass.los.is_none() {
                continue;
            }
            let aos = pass.aos.as_ref().expect("Missing AOS");
            let los = pass.los.as_ref().expect("Missing LOS");

            let (max_elev_deg, max_elev_time, orbit_at_cpa) = find_max_elevation(&oe, aos.time, los.time);
            // let obs_at_cpa = predict_observe_orbit(&oe.observer, &orbit_at_cpa);

            pass_events.push(SatPassEvent {
                cpa_time: max_elev_time,
                cpa_distance: geodesic_distance(
                    &geod,
                    latitude,
                    longitude,
                    orbit_at_cpa.latitude.to_degrees(),
                    orbit_at_cpa.longitude.to_degrees(),
                ),
                elevation: max_elev_deg,
            });
        }

        pass_events
    }
}
