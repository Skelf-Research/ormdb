"""Custom Django lookups for ORMDB search operations.

These lookups provide Django ORM support for ORMDB's advanced search features:
- Vector similarity search (HNSW)
- Geographic search (R-tree)
- Full-text search (BM25)

Usage:
    from django.db import models
    from ormdb.django.lookups import register_lookups

    # Register lookups for your fields
    register_lookups()

    # Vector search
    Product.objects.filter(embedding__vector_search={'query_vector': [...], 'k': 10})

    # Geo search
    Restaurant.objects.filter(location__geo_radius={'lat': 37.7749, 'lon': -122.4194, 'radius_km': 5.0})

    # Text search
    Article.objects.filter(content__text_match={'query': 'rust programming'})
"""

from typing import Any

from django.db.models import Field, Lookup


# ============================================================================
# Vector Search Lookups
# ============================================================================


class VectorSearchLookup(Lookup):
    """Vector similarity search lookup using HNSW index.

    Usage:
        Product.objects.filter(
            embedding__vector_search={
                'query_vector': [0.1, 0.2, 0.3, ...],
                'k': 10,
                'max_distance': 0.5,  # optional
            }
        )
    """

    lookup_name = "vector_search"

    def as_sql(self, compiler: Any, connection: Any) -> tuple[str, list[Any]]:
        """Generate SQL for the lookup."""
        lhs, lhs_params = self.process_lhs(compiler, connection)
        rhs = self.rhs

        if not isinstance(rhs, dict):
            raise ValueError("vector_search lookup requires a dict with 'query_vector' and 'k'")

        query_vector = rhs.get("query_vector", [])
        k = rhs.get("k", 10)
        max_distance = rhs.get("max_distance")

        # Build ORMDB filter expression
        filter_expr = {
            "vector_nearest_neighbor": {
                "field": lhs,
                "query_vector": query_vector,
                "k": k,
            }
        }
        if max_distance is not None:
            filter_expr["vector_nearest_neighbor"]["max_distance"] = max_distance

        # Return placeholder for ORMDB to process
        return f"__ORMDB_FILTER__({filter_expr})", lhs_params


# ============================================================================
# Geographic Search Lookups
# ============================================================================


class GeoRadiusLookup(Lookup):
    """Geographic radius search lookup.

    Usage:
        Restaurant.objects.filter(
            location__geo_radius={
                'lat': 37.7749,
                'lon': -122.4194,
                'radius_km': 5.0,
            }
        )
    """

    lookup_name = "geo_radius"

    def as_sql(self, compiler: Any, connection: Any) -> tuple[str, list[Any]]:
        """Generate SQL for the lookup."""
        lhs, lhs_params = self.process_lhs(compiler, connection)
        rhs = self.rhs

        if not isinstance(rhs, dict):
            raise ValueError("geo_radius lookup requires a dict with 'lat', 'lon', and 'radius_km'")

        filter_expr = {
            "geo_within_radius": {
                "field": lhs,
                "center_lat": rhs.get("lat", 0),
                "center_lon": rhs.get("lon", 0),
                "radius_km": rhs.get("radius_km", 0),
            }
        }

        return f"__ORMDB_FILTER__({filter_expr})", lhs_params


class GeoBoxLookup(Lookup):
    """Geographic bounding box search lookup.

    Usage:
        Restaurant.objects.filter(
            location__geo_box={
                'min_lat': 37.7,
                'min_lon': -122.5,
                'max_lat': 37.85,
                'max_lon': -122.35,
            }
        )
    """

    lookup_name = "geo_box"

    def as_sql(self, compiler: Any, connection: Any) -> tuple[str, list[Any]]:
        """Generate SQL for the lookup."""
        lhs, lhs_params = self.process_lhs(compiler, connection)
        rhs = self.rhs

        if not isinstance(rhs, dict):
            raise ValueError("geo_box lookup requires a dict with min/max lat/lon")

        filter_expr = {
            "geo_within_box": {
                "field": lhs,
                "min_lat": rhs.get("min_lat", 0),
                "min_lon": rhs.get("min_lon", 0),
                "max_lat": rhs.get("max_lat", 0),
                "max_lon": rhs.get("max_lon", 0),
            }
        }

        return f"__ORMDB_FILTER__({filter_expr})", lhs_params


class GeoPolygonLookup(Lookup):
    """Geographic polygon containment search lookup.

    Usage:
        Restaurant.objects.filter(
            location__geo_polygon={
                'vertices': [(37.7, -122.5), (37.8, -122.5), (37.8, -122.4), (37.7, -122.4)],
            }
        )
    """

    lookup_name = "geo_polygon"

    def as_sql(self, compiler: Any, connection: Any) -> tuple[str, list[Any]]:
        """Generate SQL for the lookup."""
        lhs, lhs_params = self.process_lhs(compiler, connection)
        rhs = self.rhs

        if not isinstance(rhs, dict):
            raise ValueError("geo_polygon lookup requires a dict with 'vertices'")

        filter_expr = {
            "geo_within_polygon": {
                "field": lhs,
                "vertices": rhs.get("vertices", []),
            }
        }

        return f"__ORMDB_FILTER__({filter_expr})", lhs_params


class GeoNearestLookup(Lookup):
    """Geographic k-nearest neighbor search lookup.

    Usage:
        Restaurant.objects.filter(
            location__geo_nearest={
                'lat': 37.7749,
                'lon': -122.4194,
                'k': 10,
            }
        )
    """

    lookup_name = "geo_nearest"

    def as_sql(self, compiler: Any, connection: Any) -> tuple[str, list[Any]]:
        """Generate SQL for the lookup."""
        lhs, lhs_params = self.process_lhs(compiler, connection)
        rhs = self.rhs

        if not isinstance(rhs, dict):
            raise ValueError("geo_nearest lookup requires a dict with 'lat', 'lon', and 'k'")

        filter_expr = {
            "geo_nearest_neighbor": {
                "field": lhs,
                "center_lat": rhs.get("lat", 0),
                "center_lon": rhs.get("lon", 0),
                "k": rhs.get("k", 10),
            }
        }

        return f"__ORMDB_FILTER__({filter_expr})", lhs_params


# ============================================================================
# Full-Text Search Lookups
# ============================================================================


class TextMatchLookup(Lookup):
    """Full-text search lookup with BM25 scoring.

    Usage:
        Article.objects.filter(
            content__text_match={
                'query': 'rust programming',
                'min_score': 0.5,  # optional
            }
        )

    Or with just a string:
        Article.objects.filter(content__text_match='rust programming')
    """

    lookup_name = "text_match"

    def as_sql(self, compiler: Any, connection: Any) -> tuple[str, list[Any]]:
        """Generate SQL for the lookup."""
        lhs, lhs_params = self.process_lhs(compiler, connection)
        rhs = self.rhs

        if isinstance(rhs, str):
            rhs = {"query": rhs}

        if not isinstance(rhs, dict):
            raise ValueError("text_match lookup requires a string or dict with 'query'")

        filter_expr: dict[str, Any] = {
            "text_match": {
                "field": lhs,
                "query": rhs.get("query", ""),
            }
        }
        if "min_score" in rhs:
            filter_expr["text_match"]["min_score"] = rhs["min_score"]

        return f"__ORMDB_FILTER__({filter_expr})", lhs_params


class TextPhraseLookup(Lookup):
    """Full-text phrase search lookup.

    Usage:
        Article.objects.filter(content__text_phrase='quick brown fox')
    """

    lookup_name = "text_phrase"

    def as_sql(self, compiler: Any, connection: Any) -> tuple[str, list[Any]]:
        """Generate SQL for the lookup."""
        lhs, lhs_params = self.process_lhs(compiler, connection)
        rhs = self.rhs

        if not isinstance(rhs, str):
            raise ValueError("text_phrase lookup requires a string")

        filter_expr = {
            "text_phrase": {
                "field": lhs,
                "phrase": rhs,
            }
        }

        return f"__ORMDB_FILTER__({filter_expr})", lhs_params


class TextBooleanLookup(Lookup):
    """Full-text boolean search lookup with must/should/must_not terms.

    Usage:
        Article.objects.filter(
            content__text_boolean={
                'must': ['rust'],
                'should': ['performance', 'safety'],
                'must_not': ['deprecated'],
            }
        )
    """

    lookup_name = "text_boolean"

    def as_sql(self, compiler: Any, connection: Any) -> tuple[str, list[Any]]:
        """Generate SQL for the lookup."""
        lhs, lhs_params = self.process_lhs(compiler, connection)
        rhs = self.rhs

        if not isinstance(rhs, dict):
            raise ValueError("text_boolean lookup requires a dict with must/should/must_not")

        filter_expr = {
            "text_boolean": {
                "field": lhs,
                "must": rhs.get("must", []),
                "should": rhs.get("should", []),
                "must_not": rhs.get("must_not", []),
            }
        }

        return f"__ORMDB_FILTER__({filter_expr})", lhs_params


# ============================================================================
# Lookup Registration
# ============================================================================

# All search lookups
SEARCH_LOOKUPS = [
    VectorSearchLookup,
    GeoRadiusLookup,
    GeoBoxLookup,
    GeoPolygonLookup,
    GeoNearestLookup,
    TextMatchLookup,
    TextPhraseLookup,
    TextBooleanLookup,
]


def register_lookups() -> None:
    """Register all search lookups on the base Field class.

    Call this function once during Django app initialization to enable
    search lookups on all fields.

    Example:
        # In your Django app's AppConfig.ready() method:
        from ormdb.django.lookups import register_lookups
        register_lookups()
    """
    for lookup_cls in SEARCH_LOOKUPS:
        Field.register_lookup(lookup_cls)


def register_lookup_on_field(field_class: type[Field], lookup_cls: type[Lookup]) -> None:
    """Register a specific lookup on a specific field type.

    Args:
        field_class: The Django field class to register the lookup on.
        lookup_cls: The lookup class to register.

    Example:
        from django.db.models import JSONField
        from ormdb.django.lookups import VectorSearchLookup, register_lookup_on_field

        register_lookup_on_field(JSONField, VectorSearchLookup)
    """
    field_class.register_lookup(lookup_cls)
