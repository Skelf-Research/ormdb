"""Database features for ORMDB Django backend."""

from django.db.backends.base.features import BaseDatabaseFeatures


class DatabaseFeatures(BaseDatabaseFeatures):
    """Database features supported by ORMDB."""

    # General features
    allows_group_by_selected_pks = True
    can_use_chunked_reads = True
    can_return_columns_from_insert = False
    can_return_rows_from_bulk_insert = False
    has_bulk_insert = True
    has_native_uuid_field = True
    has_native_duration_field = False
    has_real_datatype = True
    has_json_field = True
    supports_json_field_contains = True

    # Transaction features (ORMDB auto-commits each operation)
    atomic_transactions = False
    autocommits_when_autocommit_is_off = True
    can_rollback_ddl = False
    supports_atomic_references_rename = False
    supports_transactions = False
    uses_savepoints = False

    # Schema/DDL features
    can_alter_table_rename_column = False
    supports_column_check_constraints = False
    supports_expression_indexes = False
    supports_index_column_ordering = False
    supports_partial_indexes = False
    supports_table_check_constraints = False
    supports_tablespaces = False

    # Introspection
    can_introspect_autofield = True
    can_introspect_big_integer_field = True
    can_introspect_binary_field = True
    can_introspect_decimal_field = True
    can_introspect_duration_field = False
    can_introspect_ip_address_field = False
    can_introspect_positive_integer_field = True
    can_introspect_small_integer_field = True
    can_introspect_time_field = True
    can_introspect_foreign_keys = True

    # Query features
    supports_aggregate_filter_clause = False
    supports_boolean_expr_in_select_clause = True
    supports_date_lookup_using_string = True
    supports_group_by_selected_pks_on_model = True
    supports_paramstyle_pyformat = True
    supports_select_for_update = False
    supports_select_related_with_limit = True
    supports_sequence_reset = False
    supports_slicing_ordering_in_compound = True
    supports_subqueries_in_group_by = True

    # NULL handling
    interprets_empty_strings_as_nulls = False
    nulls_order_largest = True

    # Limits
    max_query_params = None  # No specific limit

    # Primary keys
    allows_auto_pk = True
    related_fields_match_type = True

    # Misc
    closed_cursor_error_class = Exception
    empty_fetchmany_value: list = []
    requires_literal_defaults = False
    supports_default_in_lead_lag = True
    supports_expression_defaults = False
    supports_ignore_conflicts = True
    supports_update_conflicts = True
    supports_update_conflicts_with_target = False
