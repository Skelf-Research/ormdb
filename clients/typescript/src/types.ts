/**
 * ORMDB TypeScript type definitions.
 */

/** Supported value types in ORMDB */
export type Value =
  | { Null: null }
  | { Bool: boolean }
  | { Int32: number }
  | { Int64: number }
  | { Float32: number }
  | { Float64: number }
  | { String: string }
  | { Bytes: number[] }
  | { Uuid: number[] }
  | { Timestamp: number }
  | { Date: string }
  | { Time: string }
  | { Json: unknown }
  | { BoolArray: boolean[] }
  | { Int32Array: number[] }
  | { Int64Array: number[] }
  | { Float32Array: number[] }
  | { Float64Array: number[] }
  | { StringArray: string[] };

/** Filter operators */
export type FilterOp =
  | "eq"
  | "ne"
  | "lt"
  | "le"
  | "gt"
  | "ge"
  | "like"
  | "ilike"
  | "in"
  | "not_in"
  | "is_null"
  | "is_not_null";

/** Filter expression */
export type FilterExpr =
  | { field: string; op: FilterOp; value?: unknown }
  | { and: FilterExpr[] }
  | { or: FilterExpr[] }
  | { not: FilterExpr };

/** Order direction */
export type OrderDirection = "asc" | "desc";

/** Order specification */
export interface OrderSpec {
  field: string;
  direction: OrderDirection;
}

/** Pagination options */
export interface Pagination {
  limit: number;
  offset: number;
}

/** Relation include specification */
export interface RelationInclude {
  relation: string;
  fields?: string[];
  filter?: FilterExpr;
  order_by?: OrderSpec[];
  limit?: number;
  includes?: RelationInclude[];
}

/** Graph query request */
export interface GraphQuery {
  root_entity: string;
  fields?: string[];
  filter?: { expression: FilterExpr };
  order_by?: OrderSpec[];
  pagination?: Pagination;
  includes?: RelationInclude[];
}

/** Entity row in query result */
export type EntityRow = Record<string, unknown>;

/** Entity block in query result */
export interface EntityBlock {
  entity_type: string;
  fields: string[];
  rows: EntityRow[];
}

/** Edge in query result */
export interface Edge {
  parent_type: string;
  parent_id: string;
  relation: string;
  child_type: string;
  child_id: string;
}

/** Edge block in query result */
export interface EdgeBlock {
  parent_type: string;
  relation: string;
  child_type: string;
  edges: Edge[];
}

/** Query result data */
export interface QueryResultData {
  entities: EntityBlock[];
  edges: EdgeBlock[];
}

/** Query result metadata */
export interface QueryResultMeta {
  total_entities: number;
  total_edges: number;
  has_more: boolean;
}

/** Query response */
export interface QueryResponse {
  data: QueryResultData;
  meta: QueryResultMeta;
}

/** Field value for mutations */
export interface FieldValue {
  field: string;
  value: Value;
}

/** Mutation request types */
export type MutationRequest =
  | { Insert: { entity: string; data: FieldValue[] } }
  | { Update: { entity: string; id: number[]; data: FieldValue[] } }
  | { Delete: { entity: string; id: number[] } }
  | { Upsert: { entity: string; id?: number[] | null; data: FieldValue[] } };

/** Mutation response */
export interface MutationResponse {
  success: boolean;
  affected: number;
  inserted_ids: string[];
}

/** Schema field definition */
export interface SchemaField {
  name: string;
  field_type: string;
  required: boolean;
  indexed: boolean;
  unique: boolean;
  default?: unknown;
}

/** Schema relation definition */
export interface SchemaRelation {
  name: string;
  target: string;
  type: "one_to_one" | "one_to_many" | "many_to_one" | "many_to_many";
  inverse?: string;
}

/** Schema entity definition */
export interface SchemaEntity {
  name: string;
  fields: SchemaField[];
  relations: SchemaRelation[];
}

/** Schema response */
export interface SchemaResponse {
  entities: SchemaEntity[];
  version: number;
}

/** Health response */
export interface HealthResponse {
  status: "healthy" | "degraded";
  version: string;
  ormdb_connected: boolean;
}

/** Replication role */
export type ReplicationRole = "primary" | "replica" | "standalone";

/** Replication status */
export interface ReplicationStatus {
  role: ReplicationRole;
  primary_addr?: string;
  current_lsn: number;
  lag_entries: number;
  lag_ms: number;
}

/** Change log entry */
export interface ChangeLogEntry {
  lsn: number;
  timestamp: number;
  entity_type: string;
  entity_id: string;
  change_type: "insert" | "update" | "delete";
  changed_fields: string[];
  schema_version: number;
}

/** Stream changes response */
export interface StreamChangesResponse {
  entries: ChangeLogEntry[];
  next_lsn: number;
  has_more: boolean;
}

/** Client configuration */
export interface OrmdbConfig {
  /** Base URL of the ORMDB gateway */
  baseUrl: string;
  /** Request timeout in milliseconds */
  timeout?: number;
  /** Custom headers to include in requests */
  headers?: Record<string, string>;
  /** Retry configuration */
  retry?: {
    maxRetries: number;
    retryDelay: number;
  };
}

/** Query options */
export interface QueryOptions<T = EntityRow> {
  fields?: (keyof T)[] | string[];
  filter?: FilterExpr;
  orderBy?: OrderSpec[];
  limit?: number;
  offset?: number;
  includes?: RelationInclude[];
}

/** Mutation result */
export interface MutationResult {
  success: boolean;
  affected: number;
  insertedIds: string[];
}

/** Query result */
export interface QueryResult<T = EntityRow> {
  entities: T[];
  edges: Edge[];
  totalEntities: number;
  totalEdges: number;
  hasMore: boolean;
}

/** Error codes */
export type ErrorCode =
  | "CONNECTION_ERROR"
  | "QUERY_ERROR"
  | "MUTATION_ERROR"
  | "VALIDATION_ERROR"
  | "SCHEMA_ERROR"
  | "TIMEOUT_ERROR"
  | "UNKNOWN_ERROR";

/** ORMDB error */
export class OrmdbError extends Error {
  constructor(
    message: string,
    public readonly code: ErrorCode,
    public readonly cause?: Error
  ) {
    super(message);
    this.name = "OrmdbError";
  }
}
