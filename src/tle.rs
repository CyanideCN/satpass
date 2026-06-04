use chrono::{Duration, NaiveDate};

fn tle_epoch_to_timestamp(tle_epoch: &str) -> f64 {
    let year: i32 = tle_epoch[0..2].parse().unwrap();
    let year_full = if year < 57 { 2000 + year } else { 1900 + year };
    let day_of_year: f64 = tle_epoch[2..].parse().unwrap();

    let naive_date = NaiveDate::from_yo_opt(year_full, day_of_year.floor() as u32).unwrap();
    let seconds_in_day = ((day_of_year - day_of_year.floor()) * 86400.0).round() as u32;
    let naive_datetime = naive_date
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .checked_add_signed(Duration::seconds(seconds_in_day as i64))
        .unwrap();

    let datetime_utc = naive_datetime.and_utc();
    datetime_utc.timestamp() as f64
}

pub struct Tle {
    pub line1: String,
    pub line2: String,
    epoch_timestamp: f64,
}

impl Tle {
    pub fn from_lines(line1: &str, line2: &str) -> Self {
        let tle_epoch = &line1[18..32];
        let epoch_timestamp = tle_epoch_to_timestamp(tle_epoch);
        Self {
            line1: line1.to_string(),
            line2: line2.to_string(),
            epoch_timestamp,
        }
    }
}

pub struct TleManager {
    pub tles: Vec<Tle>,
}

impl TleManager {
    pub fn from_tles(mut tles: Vec<Tle>) -> Self {
        assert!(
            !tles.is_empty(),
            "TLE catalog must contain at least one TLE"
        );
        tles.sort_by(|a, b| a.epoch_timestamp.total_cmp(&b.epoch_timestamp));
        TleManager { tles }
    }

    pub fn from_file(filepath: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(filepath)?;
        let lines: Vec<_> = content.lines().collect();
        assert!(
            lines.len() % 2 == 0,
            "TLE file must contain complete line pairs"
        );

        let tles = lines
            .chunks_exact(2)
            .map(|pair| {
                let line1 = pair[0];
                let line2 = pair[1];
                Tle::from_lines(line1, line2)
            })
            .collect::<Vec<_>>();
        assert!(!tles.is_empty(), "TLE file must contain at least one TLE");
        Ok(Self::from_tles(tles))
    }

    pub fn select_tle_index(&self, target_time: f64) -> usize {
        match self
            .tles
            .binary_search_by(|tle| tle.epoch_timestamp.total_cmp(&target_time))
        {
            Ok(index) => index,
            Err(0) => 0,
            Err(insert_index) if insert_index == self.tles.len() => self.tles.len() - 1,
            Err(insert_index) => {
                let before = insert_index - 1;
                let after = insert_index;
                if (self.tles[before].epoch_timestamp - target_time).abs()
                    <= (self.tles[after].epoch_timestamp - target_time).abs()
                {
                    before
                } else {
                    after
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tle_epoch_to_timestamp() {
        let tle_epoch = "23045.5";
        let timestamp = tle_epoch_to_timestamp(tle_epoch);
        assert_eq!(timestamp, 1676376000.0);
    }
}
