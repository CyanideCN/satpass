from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone

from . import _satpass


def _to_timestamp(value: datetime) -> float:
    return value.timestamp()


def _to_datetime(value: float) -> datetime:
    return datetime.fromtimestamp(value, timezone.utc)


@dataclass(frozen=True)
class SatPassEvent:
    cpa_time: datetime
    cpa_distance: float
    elevation: float

    @classmethod
    def from_native(cls, event: _satpass.SatPassEvent) -> SatPassEvent:
        return cls(
            cpa_time=_to_datetime(event.cpa_time),
            cpa_distance=event.cpa_distance,
            elevation=event.elevation,
        )


@dataclass(frozen=True)
class TCSatPassEvent:
    cpa_time: datetime
    cpa_distance: float
    sat_zenith: float
    intensity: float
    tc_lat: float
    tc_lon: float

    @classmethod
    def from_native(cls, event: _satpass.TCSatPassEvent) -> TCSatPassEvent:
        return cls(
            cpa_time=_to_datetime(event.cpa_time),
            cpa_distance=event.cpa_distance,
            sat_zenith=event.sat_zenith,
            intensity=event.intensity,
            tc_lat=event.tc_lat,
            tc_lon=event.tc_lon,
        )


class Orbital:
    def __init__(self, inner: _satpass.Orbital):
        self._inner = inner

    @classmethod
    def from_tle(cls, line1: str, line2: str) -> Orbital:
        return cls(_satpass.Orbital.from_tle(line1, line2))

    def get_passes(
        self,
        start_time: datetime,
        interval_seconds: float,
        longitude: float,
        latitude: float,
    ) -> list[SatPassEvent]:
        return [
            SatPassEvent.from_native(event)
            for event in self._inner.get_passes(
                _to_timestamp(start_time),
                interval_seconds,
                longitude,
                latitude,
            )
        ]


class TleCatalog:
    def __init__(self, inner: _satpass.TleCatalog):
        self._inner = inner

    @classmethod
    def from_file(cls, path: str) -> TleCatalog:
        return cls(_satpass.TleCatalog.from_file(path))

    @classmethod
    def from_tle(cls, line1: str, line2: str) -> TleCatalog:
        return cls(_satpass.TleCatalog.from_tle(line1, line2))

    def get_passes(
        self,
        start_time: datetime,
        interval_seconds: float,
        longitude: float,
        latitude: float,
    ) -> list[SatPassEvent]:
        return [
            SatPassEvent.from_native(event)
            for event in self._inner.get_passes(
                _to_timestamp(start_time),
                interval_seconds,
                longitude,
                latitude,
            )
        ]

    def find_tc_passes(
        self,
        bdeck_path: str,
        step_hours: float = 6.0,
        intensity_threshold: float = 100.0,
        distance_threshold: float = 1165.0,
    ) -> list[TCSatPassEvent]:
        return [
            TCSatPassEvent.from_native(event)
            for event in self._inner.find_tc_passes(
                bdeck_path,
                step_hours,
                intensity_threshold,
                distance_threshold,
            )
        ]


def find_tc_passes_from_files(
    tle_path: str,
    bdeck_path: str,
    step_hours: float = 6.0,
    intensity_threshold: float = 100.0,
    distance_threshold: float = 1165.0,
) -> list[TCSatPassEvent]:
    return [
        TCSatPassEvent.from_native(event)
        for event in _satpass.find_tc_passes_from_files(
            tle_path,
            bdeck_path,
            step_hours,
            intensity_threshold,
            distance_threshold,
        )
    ]


__all__ = [
    "Orbital",
    "SatPassEvent",
    "TCSatPassEvent",
    "TleCatalog",
    "find_tc_passes_from_files",
]
