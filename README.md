# satpass

Compute satellite passes relative to tropical cyclone best-track (B-Deck) positions
using TLEs, then report closest-approach events that meet intensity and distance
thresholds.

## What it does
- Reads a TLE file (two-line element pairs) and selects the nearest-epoch TLE for
  each B-Deck time.
- Reads an ATCF-style B-Deck file and keeps 6-hourly points.
- For each time/position, finds satellite passes and reports closest-approach
  distance and satellite zenith angle.
- Optionally formats MODIS granule names for Aqua or Terra.

## Inputs
- TLE file: consecutive line1/line2 pairs (no name lines).
- B-Deck file: ATCF best-track format with timestamp, lat, lon, and intensity.
  Only 6-hourly entries are used.

## Build
```bash
cargo build --release
```

## Python
Build the Python wheel with maturin:
```bash
maturin build --release
```

Python inputs and outputs use `datetime`; the binding converts to Unix timestamps
internally.
```python
from datetime import datetime, timezone
import satpass

catalog = satpass.TleCatalog.from_file("tle.txt")
passes = catalog.get_passes(
    datetime(2013, 11, 7, 0, 0, tzinfo=timezone.utc),
    6 * 3600,
    130.0,
    10.0,
)

tc_passes = catalog.find_tc_passes("bwp312013.dat")
```

## Usage
```bash
satpass <TLE_FILE> <BDECK_FILE> [options]
```

Example:
```bash
satpass tle.txt bwp312013.dat --intensity 100 --distance 1165 --step-hours 6 --aqua
```

## Options
- `--step-hours <hours>`: time window for pass search (default: 6)
- `--intensity <kt>`: minimum B-Deck intensity to report (default: 100)
- `--distance <km>`: maximum closest-approach distance (default: 1165)
- `--aqua`: print Aqua MODIS granule names
- `--terra`: print Terra MODIS granule names


## Notes
- Longitude is handled in 0-360 degrees east (west longitudes are converted).
- If both `--aqua` and `--terra` are omitted, the MODIS name field is blank.
