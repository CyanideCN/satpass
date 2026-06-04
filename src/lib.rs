#![cfg_attr(feature = "python", allow(deprecated, unsafe_op_in_unsafe_fn))]

pub mod bdeck;
pub mod orbital;
#[cfg(feature = "python")]
mod python;
pub mod tle;

use chrono::{DateTime, Utc};
use rayon::prelude::*;

use bdeck::BDeck;
use orbital::{Orbital, TCSatPassEvent};
use tle::{Tle, TleManager};

pub fn dt_from_unix_seconds(t_utc: f64) -> DateTime<Utc> {
    let micros = (t_utc * 1_000_000.0).round() as i64;
    DateTime::<Utc>::from_timestamp_micros(micros).expect("timestamp out of range")
}

pub struct TleCatalog {
    manager: TleManager,
    orbitals: Vec<Orbital>,
}

impl TleCatalog {
    pub fn from_file(filepath: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let manager = TleManager::from_file(filepath)?;
        Ok(Self::from_manager(manager))
    }

    pub fn from_lines(line1: &str, line2: &str) -> Self {
        Self::from_manager(TleManager::from_tles(vec![Tle::from_lines(line1, line2)]))
    }

    pub fn from_manager(manager: TleManager) -> Self {
        let orbitals = manager.tles.iter().map(Orbital::new).collect();
        Self { manager, orbitals }
    }

    pub fn select_orbital(&self, target_time: f64) -> &Orbital {
        &self.orbitals[self.manager.select_tle_index(target_time)]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TcPassOptions {
    pub step_hours: f64,
    pub intensity_threshold: f64,
    pub distance_threshold: f64,
}

impl TcPassOptions {
    pub fn new(step_hours: f64, intensity_threshold: f64, distance_threshold: f64) -> Self {
        assert!(step_hours > 0.0, "step_hours must be > 0");
        assert!(
            intensity_threshold >= 0.0,
            "intensity_threshold must be >= 0"
        );
        assert!(distance_threshold >= 0.0, "distance_threshold must be >= 0");
        Self {
            step_hours,
            intensity_threshold,
            distance_threshold,
        }
    }
}

pub fn find_tc_passes(
    catalog: &TleCatalog,
    bdeck: &BDeck,
    options: TcPassOptions,
) -> Vec<TCSatPassEvent> {
    let step_sec = options.step_hours * 3600.0;
    (0..bdeck.time.len())
        .into_par_iter()
        .map(|i| {
            let mut acc = Vec::new();
            let time = bdeck.time[i];
            let lon = bdeck.longitude[i];
            let lat = bdeck.latitude[i];
            let orbital = catalog.select_orbital(time);
            let pass_events = orbital.get_passes(time, step_sec, lon, lat);
            let mut interp_index = i;
            for pass_event in pass_events {
                let ptime = pass_event.cpa_time;
                if let Some((lat_i, lon_i, intens_i)) =
                    bdeck.interpolate_with_index(ptime, &mut interp_index)
                {
                    if intens_i < options.intensity_threshold {
                        continue;
                    }
                    let pass_refined = orbital.get_passes(ptime - 1800.0, 3600.0, lon_i, lat_i);
                    for refined_event in pass_refined.iter() {
                        if refined_event.cpa_distance <= options.distance_threshold {
                            let mut interp_cpa_index = interp_index;
                            let (tc_lat, tc_lon, _) = bdeck
                                .interpolate_with_index(
                                    refined_event.cpa_time,
                                    &mut interp_cpa_index,
                                )
                                .expect("refined CPA time must be covered by BDeck");
                            acc.push(TCSatPassEvent {
                                cpa_time: refined_event.cpa_time,
                                cpa_distance: refined_event.cpa_distance,
                                sat_zenith: 90.0 - refined_event.elevation,
                                intensity: intens_i,
                                tc_lat,
                                tc_lon,
                            });
                        }
                    }
                }
            }
            acc
        })
        .collect::<Vec<_>>()
        .into_iter()
        .flatten()
        .collect()
}

pub fn find_tc_passes_from_bdeck_file(
    catalog: &TleCatalog,
    bdeck_path: &str,
    options: TcPassOptions,
) -> std::io::Result<Vec<TCSatPassEvent>> {
    let bdeck = BDeck::from_file(bdeck_path)?;
    Ok(find_tc_passes(catalog, &bdeck, options))
}
