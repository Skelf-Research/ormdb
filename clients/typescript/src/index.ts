/**
 * ORMDB TypeScript Client
 *
 * @example
 * ```typescript
 * import { OrmdbClient } from "@ormdb/client";
 *
 * const client = new OrmdbClient("http://localhost:8080");
 *
 * // Query entities
 * const users = await client.query("User", {
 *   filter: { field: "status", op: "eq", value: "active" },
 *   limit: 10,
 * });
 *
 * // Insert entity
 * const result = await client.insert("User", {
 *   name: "Alice",
 *   email: "alice@example.com",
 * });
 * ```
 */

export { OrmdbClient, OrmdbClient as default } from "./client";

export type {
  // Configuration
  OrmdbConfig,

  // Query types
  QueryOptions,
  QueryResult,
  FilterExpr,
  FilterOp,
  OrderSpec,
  OrderDirection,
  Pagination,
  RelationInclude,
  GraphQuery,

  // Response types
  QueryResponse,
  QueryResultData,
  QueryResultMeta,
  EntityBlock,
  EntityRow,
  EdgeBlock,
  Edge,

  // Mutation types
  MutationResult,
  MutationRequest,
  MutationResponse,
  FieldValue,
  Value,

  // Schema types
  SchemaResponse,
  SchemaEntity,
  SchemaField,
  SchemaRelation,

  // Health types
  HealthResponse,

  // Replication types
  ReplicationStatus,
  ReplicationRole,
  ChangeLogEntry,
  StreamChangesResponse,

  // Error types
  ErrorCode,
} from "./types";

export { OrmdbError } from "./types";
