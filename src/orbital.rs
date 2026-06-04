use geographiclib_rs::{Geodesic, InverseGeodesic};
use predict_rs::observer::predict_observe_orbit;
use predict_rs::orbit::*;
use predict_rs::predict::{ObserverElements, PredictObservation, PredictObserver, PredictPosition};
use sgp4::{Constants, Elements};

use crate::tle;

const PASS_SCAN_STEP_SEC: f64 = 60.0;
const TIME_TOLERANCE_SEC: f64 = 0.001;

fn geodesic_distance(geod: &Geodesic, lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let s12: f64 = geod.inverse(lat1, lon1, lat2, lon2);
    s12 / 1000.0 // Convert meters to kilometers
}

fn observe_orbit(oe: &ObserverElements, time: f64) -> (PredictPosition, PredictObservation) {
    let orbit = predict_orbit(oe.elements, oe.constants, time).unwrap();
    let obs = predict_observe_orbit(oe.observer, &orbit);
    (orbit, obs)
}

fn elevation_deg(oe: &ObserverElements, time: f64) -> f64 {
    let (_, obs) = observe_orbit(oe, time);
    obs.elevation.to_degrees()
}

fn refine_horizon_crossing(oe: &ObserverElements, lower_time: f64, upper_time: f64) -> f64 {
    let mut lower_time = lower_time;
    let mut upper_time = upper_time;
    let mut lower_elevation = elevation_deg(oe, lower_time);
    let upper_elevation = elevation_deg(oe, upper_time);
    assert!(
        lower_elevation * upper_elevation <= 0.0,
        "horizon crossing must be bracketed"
    );

    while upper_time - lower_time > TIME_TOLERANCE_SEC {
        let mid_time = (lower_time + upper_time) / 2.0;
        let mid_elevation = elevation_deg(oe, mid_time);
        if lower_elevation * mid_elevation <= 0.0 {
            upper_time = mid_time;
        } else {
            lower_time = mid_time;
            lower_elevation = mid_elevation;
        }
    }

    (lower_time + upper_time) / 2.0
}

fn find_max_elevation(
    oe: &ObserverElements,
    lower_time: f64,
    upper_time: f64,
) -> (f64, f64, PredictPosition) {
    let gr = (5.0_f64.sqrt() - 1.0) / 2.0;
    let mut lower_time = lower_time;
    let mut upper_time = upper_time;
    let mut left_time = upper_time - gr * (upper_time - lower_time);
    let mut right_time = lower_time + gr * (upper_time - lower_time);
    let mut left_elevation = elevation_deg(oe, left_time);
    let mut right_elevation = elevation_deg(oe, right_time);

    while upper_time - lower_time > TIME_TOLERANCE_SEC {
        if left_elevation < right_elevation {
            lower_time = left_time;
            left_time = right_time;
            left_elevation = right_elevation;
            right_time = lower_time + gr * (upper_time - lower_time);
            right_elevation = elevation_deg(oe, right_time);
        } else {
            upper_time = right_time;
            right_time = left_time;
            right_elevation = left_elevation;
            left_time = upper_time - gr * (upper_time - lower_time);
            left_elevation = elevation_deg(oe, left_time);
        }
    }

    let max_elevation_time = (lower_time + upper_time) / 2.0;
    let (orbit, obs) = observe_orbit(oe, max_elevation_time);
    (obs.elevation.to_degrees(), max_elevation_time, orbit)
}

fn pass_windows(oe: &ObserverElements, start_utc: f64, stop_utc: f64) -> Vec<(f64, f64)> {
    let mut windows = Vec::new();
    let mut pending_rise = None;
    let mut prev_time = start_utc;
    let mut prev_elevation = elevation_deg(oe, prev_time);

    while prev_time < stop_utc || pending_rise.is_some() {
        let time = if prev_time < stop_utc {
            (prev_time + PASS_SCAN_STEP_SEC).min(stop_utc)
        } else {
            prev_time + PASS_SCAN_STEP_SEC
        };
        let elevation = elevation_deg(oe, time);

        if prev_elevation * elevation <= 0.0 && prev_elevation != elevation {
            let crossing_time = refine_horizon_crossing(oe, prev_time, time);
            if prev_elevation < elevation {
                if crossing_time <= stop_utc {
                    pending_rise = Some(crossing_time);
                }
            } else if let Some(rise_time) = pending_rise {
                windows.push((rise_time, crossing_time));
                pending_rise = None;
            }
        }

        prev_time = time;
        prev_elevation = elevation;
    }

    windows
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
    pub tc_lat: f64,
    pub tc_lon: f64,
}

pub struct Orbital {
    elements: Elements,
    constants: Constants,
}

impl Orbital {
    pub fn new(tle: &tle::Tle) -> Self {
        let elements = Elements::from_tle(None, tle.line1.as_bytes(), tle.line2.as_bytes())
            .expect("Failed to parse TLE");
        let constants = Constants::from_elements(&elements).unwrap();
        Self {
            elements,
            constants,
        }
    }

    pub fn get_passes(
        &self,
        start_utc: f64,
        interval_sec: f64,
        longitude: f64,
        latitude: f64,
    ) -> Vec<SatPassEvent> {
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
                altitude: 0., // meters
                min_elevation: 0.0,
            },
        };

        pass_windows(&oe, start_utc, start_utc + interval_sec)
            .iter()
            .map(|(aos_time, los_time)| {
                let (max_elev_deg, max_elev_time, orbit_at_cpa) =
                    find_max_elevation(&oe, *aos_time, *los_time);

                SatPassEvent {
                    cpa_time: max_elev_time,
                    cpa_distance: geodesic_distance(
                        &geod,
                        latitude,
                        longitude,
                        orbit_at_cpa.latitude.to_degrees(),
                        orbit_at_cpa.longitude.to_degrees(),
                    ),
                    elevation: max_elev_deg,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE1: &str = "1 33591U 09005A   21355.91138073  .00000074  00000+0  65091-4 0  9998";
    const LINE2: &str = "2 33591  99.1688  21.1338 0013414 329.8936  30.1462 14.12516400663123";

    #[test]
    fn pass_rising_in_last_minute_is_completed() {
        let elements = Elements::from_tle(None, LINE1.as_bytes(), LINE2.as_bytes()).unwrap();
        let constants = Constants::from_elements(&elements).unwrap();
        let observer = PredictObserver {
            name: "Observer".to_string(),
            latitude: 20.0_f64.to_radians(),
            longitude: 140.0_f64.to_radians(),
            altitude: 0.0,
            min_elevation: 0.0,
        };
        let oe = ObserverElements {
            elements: &elements,
            constants: &constants,
            observer: &observer,
        };
        let epoch = elements.datetime.and_utc().timestamp_micros() as f64 / 1_000_000.0;
        let (rise_time, _) = pass_windows(&oe, epoch, epoch + 86_400.0)[0];
        let start_time = rise_time - 3_590.0;
        let stop_time = start_time + 3_600.0;

        let windows = pass_windows(&oe, start_time, stop_time);

        assert_eq!(windows.len(), 1);
        assert!((windows[0].0 - rise_time).abs() < 0.01);
        assert!(windows[0].1 > stop_time);
    }
}
