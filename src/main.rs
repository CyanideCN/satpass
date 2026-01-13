use chrono::{DateTime, Timelike, Utc};
use clap::Parser;
use rayon::prelude::*;

mod bdeck;
mod tle;
mod orbital;
use orbital::*;

fn dt_from_unix_seconds(t_utc: f64) -> DateTime<Utc> {
    let micros = (t_utc * 1_000_000.0).round() as i64;
    DateTime::<Utc>::from_timestamp_micros(micros).expect("timestamp out of range")
}

#[derive(Parser, Debug)]
#[command(name = "satpass")]
#[command(about = "Compute satellite passes from b-deck tracks", long_about = None)]
struct Config {
    #[arg(value_name = "TLE_FILE")]
    tle_path: String,
    #[arg(value_name = "BDECK_FILE")]
    bdeck_path: String,
    #[arg(short = 's', long = "step-hours", default_value_t = 6.0, value_name = "hours")]
    step_hours: f64,
    #[arg(short = 'i', long = "intensity", default_value_t = 100.0, value_name = "kt")]
    intensity_thres: f64,
    #[arg(short = 'd', long = "distance", default_value_t = 1165., value_name = "km")]
    distance_thres: f64,
    #[arg(long = "aqua", default_value_t = false, value_name = "bool")]
    is_aqua: bool,
    #[arg(long = "terra", default_value_t = false, value_name = "bool")]
    is_terra: bool,
}

fn modis_name_fmt(scan_time: DateTime<Utc>, is_aqua: bool) -> String {
    // Round to most recent 5 minute increment
    let minute = scan_time.minute();
    let rounded_minute = (minute / 5) * 5;
    let scan_time = scan_time
        .with_minute(rounded_minute)
        .unwrap();
    let date_time = scan_time.format(".A%Y%j.%H%M").to_string();
    if is_aqua {
        format!("MYD021KM{}", date_time)
    } else {
        format!("MOD021KM{}", date_time)
    }
}

fn main() {
    let config = Config::parse();
    if config.step_hours <= 0.0 {
        eprintln!("Error: --step-hours must be > 0");
        return;
    }
    if config.intensity_thres < 0.0 {
        eprintln!("Error: --intensity must be >= 0");
        return;
    }
    if config.distance_thres < 0.0 {
        eprintln!("Error: --distance must be >= 0");
        return;
    }
    let tle_manager = tle::TLEManager::from_file(&config.tle_path).unwrap();
    let orbitals: Vec<Orbital> = tle_manager
        .tles
        .iter()
        .map(Orbital::new)
        .collect();
    let bdeck = bdeck::BDeck::from_file(&config.bdeck_path).unwrap();
    // Loop over bdeck to find all passes
    let step_sec = config.step_hours * 3600.0;
    let intensity_thres = config.intensity_thres;
    let distance_thres = config.distance_thres;
    let all_passes: Vec<TCSatPassEvent> = (0..bdeck.time.len())
        .into_par_iter()
        .map(|i| {
            let mut acc = Vec::new();
            let time = bdeck.time[i];
            let lon = bdeck.longitude[i];
            let lat = bdeck.latitude[i];
            let Some(tle_index) = tle_manager.select_tle_index(time) else {
                return acc;
            };
            let orbital = &orbitals[tle_index];
            let pass_events = orbital.get_passes(time, step_sec, lon, lat);
            let mut interp_index = i;
            for pass_event in pass_events {
                let ptime = pass_event.cpa_time;
                if let Some((lat_i, lon_i, intens_i)) =
                    bdeck.interpolate_with_index(ptime, &mut interp_index)
                {
                    if intens_i < intensity_thres {
                        continue;
                    }
                    let pass_refined = orbital.get_passes(ptime - 1800.0, 3600.0, lon_i, lat_i);
                    for refined_event in pass_refined.iter() {
                        if refined_event.cpa_distance <= distance_thres {
                            acc.push(TCSatPassEvent {
                                cpa_time: refined_event.cpa_time,
                                cpa_distance: refined_event.cpa_distance,
                                sat_zenith: 90.0 - refined_event.elevation,
                                intensity: intens_i,
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
        .collect();

    for event in all_passes.iter() {
        let dt_cpa = dt_from_unix_seconds(event.cpa_time);
        let mut sat_file_name: String = " ".to_string();
        if config.is_aqua {
            sat_file_name = modis_name_fmt(dt_cpa, true);
        } else if config.is_terra {
            sat_file_name = modis_name_fmt(dt_cpa, false);
        }
        println!("{} - Distance: {:4.0} km  Zenith: {:4.1}° Intensity: {:3.0} kt   {}",
            dt_cpa.format("%Y-%m-%d %H:%M:%S"),
            event.cpa_distance,
            event.sat_zenith,
            event.intensity,
            sat_file_name);
    }
}
