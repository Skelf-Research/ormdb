"""Django database backend for ORMDB."""

# The backend module must be importable from 'ormdb.django'
# Django expects to find 'base.DatabaseWrapper' in this package

from .lookups import (
    # Lookup classes
    VectorSearchLookup,
    GeoRadiusLookup,
    GeoBoxLookup,
    GeoPolygonLookup,
    GeoNearestLookup,
    TextMatchLookup,
    TextPhraseLookup,
    TextBooleanLookup,
    # Registration functions
    register_lookups,
    register_lookup_on_field,
    SEARCH_LOOKUPS,
)

__all__ = [
    # Lookup classes
    "VectorSearchLookup",
    "GeoRadiusLookup",
    "GeoBoxLookup",
    "GeoPolygonLookup",
    "GeoNearestLookup",
    "TextMatchLookup",
    "TextPhraseLookup",
    "TextBooleanLookup",
    # Registration functions
    "register_lookups",
    "register_lookup_on_field",
    "SEARCH_LOOKUPS",
]
