use chrono::NaiveDateTime;
use std::fs::read_to_string;
use std::io;

pub struct BDeck {
    pub time: Vec<f64>,
    pub intensity: Vec<f64>,
    pub latitude: Vec<f64>,
    pub longitude: Vec<f64>,
}

impl BDeck {
    pub fn from_file(filepath: &str) -> io::Result<Self> {
        let mut time = Vec::new();
        let mut intensity = Vec::new();
        let mut latitude = Vec::new();
        let mut longitude = Vec::new();

        let file = read_to_string(filepath)?;
        let mut last_time = "";
        for line in file.lines() {
            let line_time = &line[8..18];
            let line_hour = &line_time[8..10];
            if line_hour.parse::<i32>().unwrap() % 6 != 0 {
                continue;
            }
            if last_time == line_time {
                continue;
            }
            last_time = line_time;
            let timestamp = NaiveDateTime::parse_from_str(&format!("{line_time}00"), "%Y%m%d%H%M")
                .unwrap()
                .and_utc()
                .timestamp() as f64;
            let wind_field = if line.len() < 52 {
                &line[line.len() - 4..]
            } else {
                &line[48..51]
            };
            let wind = wind_field.trim().parse::<i32>().unwrap();
            let wind = if wind == 999 { 0 } else { wind };
            let lat_str = &line[35..39];
            let mut lat = lat_str[..3].trim().parse::<f64>().unwrap() / 10.0;
            if &lat_str[3..4] == "S" {
                lat *= -1.0;
            }
            let lon_str = &line[41..46];
            let mut lon = lon_str[..4].trim().parse::<f64>().unwrap() / 10.0;
            if &lon_str[4..5] == "W" {
                lon = 360.0 - lon;
            }
            time.push(timestamp);
            intensity.push(wind as f64);
            latitude.push(lat);
            longitude.push(lon);
        }

        Ok(BDeck {
            time,
            intensity,
            latitude,
            longitude,
        })
    }

    pub fn interpolate_with_index(
        &self,
        query_time: f64,
        index: &mut usize,
    ) -> Option<(f64, f64, f64)> {
        if self.time.is_empty() {
            return None;
        }
        if query_time < self.time[0] || query_time > *self.time.last().unwrap() {
            return None;
        }

        let mut i = (*index).min(self.time.len() - 1);
        if query_time < self.time[i] {
            match self.time.binary_search_by(|t| t.total_cmp(&query_time)) {
                Ok(found) => {
                    *index = found;
                    return Some((
                        self.latitude[found],
                        self.longitude[found],
                        self.intensity[found],
                    ));
                }
                Err(found) => {
                    if found == 0 || found >= self.time.len() {
                        return None;
                    }
                    i = found - 1;
                }
            }
        } else {
            while i + 1 < self.time.len() && self.time[i + 1] < query_time {
                i += 1;
            }
        }

        if self.time[i] == query_time {
            *index = i;
            return Some((self.latitude[i], self.longitude[i], self.intensity[i]));
        }
        if i + 1 < self.time.len() && self.time[i + 1] == query_time {
            *index = i + 1;
            return Some((
                self.latitude[i + 1],
                self.longitude[i + 1],
                self.intensity[i + 1],
            ));
        }
        if i + 1 >= self.time.len() {
            return None;
        }

        let t0 = self.time[i];
        let t1 = self.time[i + 1];
        let factor = (query_time - t0) / (t1 - t0);

        let lat = self.latitude[i] + factor * (self.latitude[i + 1] - self.latitude[i]);
        let lon = self.longitude[i] + factor * (self.longitude[i + 1] - self.longitude[i]);
        let inten = self.intensity[i] + factor * (self.intensity[i + 1] - self.intensity[i]);

        *index = i;
        Some((lat, lon, inten))
    }
}
