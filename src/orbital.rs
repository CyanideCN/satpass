use core::f64::consts::PI;

use geographiclib_rs::{Geodesic, InverseGeodesic};
use sgp4::{Constants, Elements, MinutesSinceEpoch, Prediction};

use crate::tle;

const PASS_SCAN_STEP_SEC: f64 = 60.0;
const TIME_TOLERANCE_SEC: f64 = 0.001;
const SECONDS_PER_DAY: f64 = 86_400.0;
const JULIAN_UNIX_EPOCH: f64 = 2_440_587.5;
const EARTH_RADIUS_KM_WGS84: f64 = 6_378.137;
const FLATTENING_FACTOR: f64 = 3.352_810_664_747_48E-3;
const EARTH_ROTATIONS_PER_SIDEREAL_DAY: f64 = 1.002_737_909_34;

fn geodesic_distance(geod: &Geodesic, lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let s12: f64 = geod.inverse(lat1, lon1, lat2, lon2);
    s12 / 1000.0 // Convert meters to kilometers
}

fn asin_clamped(value: f64) -> f64 {
    value.clamp(-1.0, 1.0).asin()
}

fn gmst(time_utc: f64) -> f64 {
    let jd = time_utc / SECONDS_PER_DAY + JULIAN_UNIX_EPOCH;
    let ut = (jd + 0.5).fract();
    let jd0 = jd - ut;
    let tu = (jd0 - 2_451_545.0) / 36_525.0;
    let mut gmst = 24_110.548_41 + tu * (8_640_184.812_866 + tu * (0.093_104 - tu * 6.2E-6));
    gmst = (gmst + SECONDS_PER_DAY * EARTH_ROTATIONS_PER_SIDEREAL_DAY * ut) % SECONDS_PER_DAY;
    (2.0 * PI * gmst / SECONDS_PER_DAY).rem_euclid(2.0 * PI)
}

fn vec3_sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn vec3_length(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn observer_position(theta: f64, observer: &Observer) -> [f64; 3] {
    let c = 1.0
        / (1.0 + FLATTENING_FACTOR * (FLATTENING_FACTOR - 2.0) * observer.sin_lat.powi(2)).sqrt();
    let sq = (1.0 - FLATTENING_FACTOR).powi(2) * c;
    let achcp = EARTH_RADIUS_KM_WGS84 * c * observer.cos_lat;
    [
        achcp * theta.cos(),
        achcp * theta.sin(),
        EARTH_RADIUS_KM_WGS84 * sq * observer.sin_lat,
    ]
}

fn satellite_lat_lon(theta: f64, position: [f64; 3]) -> (f64, f64) {
    let lon = (position[1].atan2(position[0]) - theta).rem_euclid(2.0 * PI);
    let r = (position[0] * position[0] + position[1] * position[1]).sqrt();
    let e2 = FLATTENING_FACTOR * (2.0 - FLATTENING_FACTOR);
    let mut lat = position[2].atan2(r);
    loop {
        let phi = lat;
        let c = 1.0 / (1.0 - e2 * phi.sin().powi(2)).sqrt();
        lat = (position[2] + EARTH_RADIUS_KM_WGS84 * c * e2 * phi.sin()).atan2(r);
        if (lat - phi).abs() < 1E-10 {
            break;
        }
    }
    if lat > PI / 2.0 {
        lat -= 2.0 * PI;
    }
    (lat.to_degrees(), lon.to_degrees())
}

struct Observer {
    lon_rad: f64,
    sin_lat: f64,
    cos_lat: f64,
}

impl Observer {
    fn new(latitude: f64, longitude: f64) -> Self {
        let lat_rad = latitude.to_radians();
        let lon_rad = longitude.to_radians();
        Self {
            lon_rad,
            sin_lat: lat_rad.sin(),
            cos_lat: lat_rad.cos(),
        }
    }
}

struct OrbitSample {
    elevation_deg: f64,
    sat_lat_deg: f64,
    sat_lon_deg: f64,
}

fn elevation_deg(orbital: &Orbital, observer: &Observer, time: f64) -> f64 {
    orbital.sample(observer, time).elevation_deg
}

fn refine_horizon_crossing(
    orbital: &Orbital,
    observer: &Observer,
    lower_time: f64,
    upper_time: f64,
) -> f64 {
    let mut lower_time = lower_time;
    let mut upper_time = upper_time;
    let mut lower_elevation = elevation_deg(orbital, observer, lower_time);
    let upper_elevation = elevation_deg(orbital, observer, upper_time);
    assert!(
        lower_elevation * upper_elevation <= 0.0,
        "horizon crossing must be bracketed"
    );

    while upper_time - lower_time > TIME_TOLERANCE_SEC {
        let mid_time = (lower_time + upper_time) / 2.0;
        let mid_elevation = elevation_deg(orbital, observer, mid_time);
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
    orbital: &Orbital,
    observer: &Observer,
    lower_time: f64,
    upper_time: f64,
) -> (f64, f64, OrbitSample) {
    let gr = (5.0_f64.sqrt() - 1.0) / 2.0;
    let mut lower_time = lower_time;
    let mut upper_time = upper_time;
    let mut left_time = upper_time - gr * (upper_time - lower_time);
    let mut right_time = lower_time + gr * (upper_time - lower_time);
    let mut left_elevation = elevation_deg(orbital, observer, left_time);
    let mut right_elevation = elevation_deg(orbital, observer, right_time);

    while upper_time - lower_time > TIME_TOLERANCE_SEC {
        if left_elevation < right_elevation {
            lower_time = left_time;
            left_time = right_time;
            left_elevation = right_elevation;
            right_time = lower_time + gr * (upper_time - lower_time);
            right_elevation = elevation_deg(orbital, observer, right_time);
        } else {
            upper_time = right_time;
            right_time = left_time;
            right_elevation = left_elevation;
            left_time = upper_time - gr * (upper_time - lower_time);
            left_elevation = elevation_deg(orbital, observer, left_time);
        }
    }

    let max_elevation_time = (lower_time + upper_time) / 2.0;
    let sample = orbital.sample(observer, max_elevation_time);
    (sample.elevation_deg, max_elevation_time, sample)
}

fn pass_windows(
    orbital: &Orbital,
    observer: &Observer,
    start_utc: f64,
    stop_utc: f64,
) -> Vec<(f64, f64)> {
    let mut windows = Vec::new();
    let mut pending_rise = None;
    let mut prev_time = start_utc;
    let mut prev_elevation = elevation_deg(orbital, observer, prev_time);

    while prev_time < stop_utc || pending_rise.is_some() {
        let time = if prev_time < stop_utc {
            (prev_time + PASS_SCAN_STEP_SEC).min(stop_utc)
        } else {
            prev_time + PASS_SCAN_STEP_SEC
        };
        let elevation = elevation_deg(orbital, observer, time);

        if prev_elevation * elevation <= 0.0 && prev_elevation != elevation {
            let crossing_time = refine_horizon_crossing(orbital, observer, prev_time, time);
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
    epoch_utc: f64,
    constants: Constants,
}

impl Orbital {
    pub fn new(tle: &tle::Tle) -> Self {
        let elements = Elements::from_tle(None, tle.line1.as_bytes(), tle.line2.as_bytes())
            .expect("Failed to parse TLE");
        let epoch_utc = elements.datetime.and_utc().timestamp_micros() as f64 / 1_000_000.0;
        let constants = Constants::from_elements(&elements).unwrap();
        Self {
            epoch_utc,
            constants,
        }
    }

    fn propagate(&self, time_utc: f64) -> Prediction {
        self.constants
            .propagate(MinutesSinceEpoch((time_utc - self.epoch_utc) / 60.0))
            .unwrap()
    }

    fn sample(&self, observer: &Observer, time_utc: f64) -> OrbitSample {
        let prediction = self.propagate(time_utc);
        let theta = gmst(time_utc);
        let observer_theta = (theta + observer.lon_rad).rem_euclid(2.0 * PI);
        let observer_pos = observer_position(observer_theta, observer);
        let range = vec3_sub(prediction.position, observer_pos);
        let range_length = vec3_length(range);

        let sin_theta = observer_theta.sin();
        let cos_theta = observer_theta.cos();
        let top_z = observer.cos_lat * cos_theta * range[0]
            + observer.cos_lat * sin_theta * range[1]
            + observer.sin_lat * range[2];
        let elevation = asin_clamped(top_z / range_length);
        let (sat_lat_deg, sat_lon_deg) = satellite_lat_lon(theta, prediction.position);

        OrbitSample {
            elevation_deg: elevation.to_degrees(),
            sat_lat_deg,
            sat_lon_deg,
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
        let observer = Observer::new(latitude, longitude);

        pass_windows(self, &observer, start_utc, start_utc + interval_sec)
            .iter()
            .map(|(aos_time, los_time)| {
                let (max_elev_deg, max_elev_time, sample_at_cpa) =
                    find_max_elevation(self, &observer, *aos_time, *los_time);

                SatPassEvent {
                    cpa_time: max_elev_time,
                    cpa_distance: geodesic_distance(
                        &geod,
                        latitude,
                        longitude,
                        sample_at_cpa.sat_lat_deg,
                        sample_at_cpa.sat_lon_deg,
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
        let tle = tle::Tle::from_lines(LINE1, LINE2);
        let orbital = Orbital::new(&tle);
        let observer = Observer::new(20.0, 140.0);
        let (rise_time, _) = pass_windows(
            &orbital,
            &observer,
            orbital.epoch_utc,
            orbital.epoch_utc + 86_400.0,
        )[0];
        let start_time = rise_time - 3_590.0;
        let stop_time = start_time + 3_600.0;

        let windows = pass_windows(&orbital, &observer, start_time, stop_time);

        assert_eq!(windows.len(), 1);
        assert!((windows[0].0 - rise_time).abs() < 0.01);
        assert!(windows[0].1 > stop_time);
    }
}
