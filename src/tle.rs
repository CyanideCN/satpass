use chrono::{NaiveDate, Duration};

fn tle_epoch_to_timestamp(tle_epoch: &str) -> f64 {
    let year: i32 = tle_epoch[0..2].parse().unwrap();
    let year_full = if year < 57 { 2000 + year } else { 1900 + year };
    let day_of_year: f64 = tle_epoch[2..].parse().unwrap();

    let naive_date = NaiveDate::from_yo_opt(year_full, day_of_year.floor() as u32).unwrap();
    let seconds_in_day = ((day_of_year - day_of_year.floor()) * 86400.0).round() as u32;
    let naive_datetime = naive_date.and_hms_opt(0, 0, 0).unwrap()
        .checked_add_signed(Duration::seconds(seconds_in_day as i64)).unwrap();

    let datetime_utc = naive_datetime.and_utc();
    datetime_utc.timestamp() as f64
}

pub struct TLE {
    pub line1: String,
    pub line2: String,
    epoch_timestamp: f64,
}

pub struct TLEManager {
    pub tles: Vec<TLE>,
}

impl TLEManager {
    pub fn from_file(filepath: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(filepath)?;
        let mut tles = Vec::new();
        let mut lines = content.lines();
        while let (Some(line1), Some(line2)) = (lines.next(), lines.next()) {
            if line1.len() >= 32 {
                let tle_epoch = &line1[18..32];
                let epoch_timestamp = tle_epoch_to_timestamp(tle_epoch);
                tles.push(TLE {
                    line1: line1.to_string(),
                    line2: line2.to_string(),
                    epoch_timestamp,
                });
            }
        }
        tles.sort_by(|a, b| {
            a.epoch_timestamp
                .partial_cmp(&b.epoch_timestamp)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(TLEManager { tles })
    }

    pub fn select_tle_index(&self, target_time: f64) -> Option<usize> {
        if self.tles.is_empty() {
            return None;
        }

        match self.tles.binary_search_by(|tle| {
            if tle.epoch_timestamp < target_time {
                std::cmp::Ordering::Less
            } else if tle.epoch_timestamp > target_time {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        }) {
            Ok(index) => Some(index),
            Err(insert_index) => {
                if insert_index == 0 {
                    return Some(0);
                }
                if insert_index >= self.tles.len() {
                    return Some(self.tles.len() - 1);
                }

                let before = insert_index - 1;
                let after = insert_index;
                if (self.tles[before].epoch_timestamp - target_time).abs()
                    <= (self.tles[after].epoch_timestamp - target_time).abs()
                {
                    Some(before)
                } else {
                    Some(after)
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
