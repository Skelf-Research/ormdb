"""SQLAlchemy dialect for ORMDB."""

from .dialect import (
    OrmdbDialect,
    # Search expression types
    VectorSearch,
    GeoWithinRadius,
    GeoWithinBox,
    GeoWithinPolygon,
    GeoNearest,
    TextMatch,
    TextPhrase,
    TextBoolean,
    # Convenience functions
    vector_search,
    geo_within_radius,
    geo_within_box,
    geo_within_polygon,
    geo_nearest,
    text_match,
    text_phrase,
    text_boolean,
)

__all__ = [
    "OrmdbDialect",
    # Search expression types
    "VectorSearch",
    "GeoWithinRadius",
    "GeoWithinBox",
    "GeoWithinPolygon",
    "GeoNearest",
    "TextMatch",
    "TextPhrase",
    "TextBoolean",
    # Convenience functions
    "vector_search",
    "geo_within_radius",
    "geo_within_box",
    "geo_within_polygon",
    "geo_nearest",
    "text_match",
    "text_phrase",
    "text_boolean",
]
