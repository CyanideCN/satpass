use chrono::{DateTime, Duration, Timelike, Utc};
use clap::Parser;
use satpass::{TcPassOptions, TleCatalog, dt_from_unix_seconds, find_tc_passes_from_bdeck_file};

#[derive(Parser, Debug)]
#[command(name = "satpass")]
#[command(about = "Compute satellite passes from b-deck tracks", long_about = None)]
struct Config {
    #[arg(value_name = "TLE_FILE")]
    tle_path: String,
    #[arg(value_name = "BDECK_FILE")]
    bdeck_path: String,
    #[arg(
        short = 's',
        long = "step-hours",
        default_value_t = 6.0,
        value_name = "hours"
    )]
    step_hours: f64,
    #[arg(
        short = 'i',
        long = "intensity",
        default_value_t = 100.0,
        value_name = "kt"
    )]
    intensity_thres: f64,
    #[arg(
        short = 'd',
        long = "distance",
        default_value_t = 1165.,
        value_name = "km"
    )]
    distance_thres: f64,
    #[arg(long = "aqua", default_value_t = false, value_name = "bool")]
    is_aqua: bool,
    #[arg(long = "terra", default_value_t = false, value_name = "bool")]
    is_terra: bool,
}

fn modis_name_from_time(scan_time: DateTime<Utc>, is_aqua: bool) -> String {
    let date_time = scan_time.format(".A%Y%j.%H%M").to_string();
    if is_aqua {
        format!("MYD021KM{}", date_time)
    } else {
        format!("MOD021KM{}", date_time)
    }
}

fn modis_name_fmt(scan_time: DateTime<Utc>, is_aqua: bool) -> String {
    // Round down to the most recent 5-minute boundary.
    let minute = scan_time.minute();
    let rounded_minute = (minute / 5) * 5;
    let base_time = scan_time
        .with_minute(rounded_minute)
        .and_then(|t| t.with_second(0))
        .expect("valid minute/second");

    // If we're within 30 seconds of a 5-minute boundary, also include the adjacent granule.
    let seconds_since_last_boundary = (minute % 5) * 60 + scan_time.second();
    let seconds_to_next_boundary = 300 - seconds_since_last_boundary;

    let mut names = vec![modis_name_from_time(base_time, is_aqua)];

    if seconds_since_last_boundary <= 30 {
        let adjacent_time = base_time - Duration::minutes(5);
        names.push(modis_name_from_time(adjacent_time, is_aqua));
    } else if seconds_to_next_boundary <= 30 {
        let adjacent_time = base_time + Duration::minutes(5);
        names.push(modis_name_from_time(adjacent_time, is_aqua));
    }

    names.join(" ")
}

fn main() {
    let config = Config::parse();
    let catalog = TleCatalog::from_file(&config.tle_path).unwrap();
    let options = TcPassOptions::new(
        config.step_hours,
        config.intensity_thres,
        config.distance_thres,
    );
    let all_passes = find_tc_passes_from_bdeck_file(&catalog, &config.bdeck_path, options).unwrap();

    for event in all_passes.iter() {
        let dt_cpa = dt_from_unix_seconds(event.cpa_time);
        let sat_file_name = if config.is_aqua {
            modis_name_fmt(dt_cpa, true)
        } else if config.is_terra {
            modis_name_fmt(dt_cpa, false)
        } else {
            String::new()
        };
        println!(
            "{} - Pos: ({:6.2},{:7.2})  Dist: {:4.0} km  Zenith: {:4.1} deg Wind: {:3.0} kt   {}",
            dt_cpa.format("%Y-%m-%d %H:%M:%S"),
            event.tc_lat,
            event.tc_lon,
            event.cpa_distance,
            event.sat_zenith,
            event.intensity,
            sat_file_name
        );
    }
}
