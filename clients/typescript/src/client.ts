/**
 * ORMDB HTTP client for TypeScript.
 */

import type {
  OrmdbConfig,
  QueryOptions,
  QueryResult,
  MutationResult,
  GraphQuery,
  QueryResponse,
  MutationRequest,
  MutationResponse,
  SchemaResponse,
  HealthResponse,
  ReplicationStatus,
  StreamChangesResponse,
  FilterExpr,
  Value,
  FieldValue,
  EntityRow,
} from "./types";
import { OrmdbError } from "./types";

/** Default configuration */
const DEFAULT_CONFIG: Required<Omit<OrmdbConfig, "baseUrl" | "headers">> & {
  headers: Record<string, string>;
} = {
  timeout: 30000,
  headers: {},
  retry: {
    maxRetries: 3,
    retryDelay: 1000,
  },
};

/**
 * ORMDB HTTP client.
 *
 * @example
 * ```typescript
 * const client = new OrmdbClient({ baseUrl: "http://localhost:8080" });
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
export class OrmdbClient {
  private readonly config: Required<OrmdbConfig>;

  constructor(config: OrmdbConfig | string) {
    if (typeof config === "string") {
      config = { baseUrl: config };
    }

    this.config = {
      ...DEFAULT_CONFIG,
      ...config,
      headers: { ...DEFAULT_CONFIG.headers, ...config.headers },
      retry: { ...DEFAULT_CONFIG.retry, ...config.retry },
    };

    // Remove trailing slash from base URL
    this.config.baseUrl = this.config.baseUrl.replace(/\/$/, "");
  }

  /**
   * Check gateway health.
   */
  async health(): Promise<HealthResponse> {
    return this.request<HealthResponse>("GET", "/health");
  }

  /**
   * Get database schema.
   */
  async getSchema(): Promise<SchemaResponse> {
    return this.request<SchemaResponse>("GET", "/schema");
  }

  /**
   * Execute a query.
   */
  async query<T extends EntityRow = EntityRow>(
    entity: string,
    options: QueryOptions<T> = {}
  ): Promise<QueryResult<T>> {
    const payload: GraphQuery = {
      root_entity: entity,
    };

    if (options.fields) {
      payload.fields = options.fields as string[];
    }
    if (options.filter) {
      payload.filter = { expression: options.filter };
    }
    if (options.orderBy) {
      payload.order_by = options.orderBy;
    }
    if (options.limit !== undefined || options.offset !== undefined) {
      payload.pagination = {
        limit: options.limit ?? 100,
        offset: options.offset ?? 0,
      };
    }
    if (options.includes) {
      payload.includes = options.includes;
    }

    const response = await this.request<QueryResponse>("POST", "/query", payload);

    // Flatten entity blocks
    const entities: T[] = [];
    for (const block of response.data.entities) {
      entities.push(...(block.rows as T[]));
    }

    // Flatten edge blocks
    const edges = response.data.edges.flatMap((block) => block.edges);

    return {
      entities,
      edges,
      totalEntities: response.meta.total_entities,
      totalEdges: response.meta.total_edges,
      hasMore: response.meta.has_more,
    };
  }

  /**
   * Find a single entity by filter.
   */
  async findFirst<T extends EntityRow = EntityRow>(
    entity: string,
    options: Omit<QueryOptions<T>, "limit"> = {}
  ): Promise<T | null> {
    const result = await this.query<T>(entity, { ...options, limit: 1 });
    return result.entities[0] ?? null;
  }

  /**
   * Find a single entity by ID.
   */
  async findById<T extends EntityRow = EntityRow>(
    entity: string,
    id: string,
    options: Pick<QueryOptions<T>, "fields" | "includes"> = {}
  ): Promise<T | null> {
    return this.findFirst<T>(entity, {
      ...options,
      filter: { field: "id", op: "eq", value: id },
    });
  }

  /**
   * Count entities matching a filter.
   */
  async count(entity: string, filter?: FilterExpr): Promise<number> {
    const result = await this.query(entity, { filter, limit: 0 });
    return result.totalEntities;
  }

  /**
   * Insert a new entity.
   */
  async insert(
    entity: string,
    data: Record<string, unknown>
  ): Promise<MutationResult> {
    const payload: MutationRequest = {
      Insert: {
        entity,
        data: this.convertToFieldValues(data),
      },
    };

    const response = await this.request<MutationResponse>(
      "POST",
      "/mutate",
      payload
    );

    return {
      success: response.success,
      affected: response.affected,
      insertedIds: response.inserted_ids,
    };
  }

  /**
   * Insert multiple entities.
   */
  async insertMany(
    entity: string,
    records: Record<string, unknown>[]
  ): Promise<MutationResult> {
    const results: MutationResult = {
      success: true,
      affected: 0,
      insertedIds: [],
    };

    // Execute inserts sequentially (batch API would be more efficient)
    for (const record of records) {
      const result = await this.insert(entity, record);
      results.affected += result.affected;
      results.insertedIds.push(...result.insertedIds);
      if (!result.success) {
        results.success = false;
      }
    }

    return results;
  }

  /**
   * Update an entity by ID.
   */
  async update(
    entity: string,
    id: string,
    data: Record<string, unknown>
  ): Promise<MutationResult> {
    const payload: MutationRequest = {
      Update: {
        entity,
        id: this.hexToUuid(id),
        data: this.convertToFieldValues(data),
      },
    };

    const response = await this.request<MutationResponse>(
      "POST",
      "/mutate",
      payload
    );

    return {
      success: response.success,
      affected: response.affected,
      insertedIds: response.inserted_ids,
    };
  }

  /**
   * Update multiple entities matching a filter.
   */
  async updateMany(
    entity: string,
    filter: FilterExpr,
    data: Record<string, unknown>
  ): Promise<MutationResult> {
    // First, find all matching entities
    const result = await this.query(entity, {
      filter,
      fields: ["id"],
    });

    const results: MutationResult = {
      success: true,
      affected: 0,
      insertedIds: [],
    };

    // Update each entity
    for (const row of result.entities) {
      const updateResult = await this.update(entity, row.id as string, data);
      results.affected += updateResult.affected;
      if (!updateResult.success) {
        results.success = false;
      }
    }

    return results;
  }

  /**
   * Delete an entity by ID.
   */
  async delete(entity: string, id: string): Promise<MutationResult> {
    const payload: MutationRequest = {
      Delete: {
        entity,
        id: this.hexToUuid(id),
      },
    };

    const response = await this.request<MutationResponse>(
      "POST",
      "/mutate",
      payload
    );

    return {
      success: response.success,
      affected: response.affected,
      insertedIds: response.inserted_ids,
    };
  }

  /**
   * Delete multiple entities matching a filter.
   */
  async deleteMany(entity: string, filter: FilterExpr): Promise<MutationResult> {
    // First, find all matching entities
    const result = await this.query(entity, {
      filter,
      fields: ["id"],
    });

    const results: MutationResult = {
      success: true,
      affected: 0,
      insertedIds: [],
    };

    // Delete each entity
    for (const row of result.entities) {
      const deleteResult = await this.delete(entity, row.id as string);
      results.affected += deleteResult.affected;
      if (!deleteResult.success) {
        results.success = false;
      }
    }

    return results;
  }

  /**
   * Upsert an entity (insert or update).
   */
  async upsert(
    entity: string,
    data: Record<string, unknown>,
    id?: string
  ): Promise<MutationResult> {
    const payload: MutationRequest = {
      Upsert: {
        entity,
        id: id ? this.hexToUuid(id) : null,
        data: this.convertToFieldValues(data),
      },
    };

    const response = await this.request<MutationResponse>(
      "POST",
      "/mutate",
      payload
    );

    return {
      success: response.success,
      affected: response.affected,
      insertedIds: response.inserted_ids,
    };
  }

  /**
   * Get replication status.
   */
  async getReplicationStatus(): Promise<ReplicationStatus> {
    const response = await this.request<{ data: ReplicationStatus }>(
      "GET",
      "/replication/status"
    );
    return response.data;
  }

  /**
   * Stream changes from the changelog.
   */
  async streamChanges(options: {
    fromLsn?: number;
    limit?: number;
    entities?: string[];
  } = {}): Promise<StreamChangesResponse> {
    const params = new URLSearchParams();
    params.set("from_lsn", String(options.fromLsn ?? 0));
    params.set("limit", String(options.limit ?? 1000));
    if (options.entities?.length) {
      params.set("entities", options.entities.join(","));
    }

    return this.request<StreamChangesResponse>(
      "GET",
      `/replication/changes?${params.toString()}`
    );
  }

  /**
   * Execute a raw HTTP request.
   */
  private async request<T>(
    method: "GET" | "POST",
    path: string,
    body?: unknown
  ): Promise<T> {
    const url = `${this.config.baseUrl}${path}`;
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      ...this.config.headers,
    };

    let lastError: Error | undefined;
    const maxRetries = this.config.retry.maxRetries;

    for (let attempt = 0; attempt <= maxRetries; attempt++) {
      try {
        const controller = new AbortController();
        const timeoutId = setTimeout(
          () => controller.abort(),
          this.config.timeout
        );

        const response = await fetch(url, {
          method,
          headers,
          body: body ? JSON.stringify(body) : undefined,
          signal: controller.signal,
        });

        clearTimeout(timeoutId);

        if (!response.ok) {
          const errorBody = await response.json().catch(() => ({}));
          throw new OrmdbError(
            errorBody.message || `HTTP ${response.status}: ${response.statusText}`,
            this.mapStatusToErrorCode(response.status)
          );
        }

        return (await response.json()) as T;
      } catch (error) {
        lastError = error as Error;

        // Don't retry on client errors
        if (error instanceof OrmdbError) {
          if (
            error.code === "VALIDATION_ERROR" ||
            error.code === "QUERY_ERROR" ||
            error.code === "MUTATION_ERROR"
          ) {
            throw error;
          }
        }

        // Retry on network/timeout errors
        if (attempt < maxRetries) {
          await this.sleep(this.config.retry.retryDelay * (attempt + 1));
          continue;
        }
      }
    }

    throw new OrmdbError(
      lastError?.message ?? "Request failed",
      "CONNECTION_ERROR",
      lastError
    );
  }

  /**
   * Convert a record to field values.
   */
  private convertToFieldValues(data: Record<string, unknown>): FieldValue[] {
    return Object.entries(data).map(([field, value]) => ({
      field,
      value: this.convertValue(value),
    }));
  }

  /**
   * Convert a JavaScript value to ORMDB Value format.
   */
  private convertValue(value: unknown): Value {
    if (value === null || value === undefined) {
      return { Null: null };
    }
    if (typeof value === "boolean") {
      return { Bool: value };
    }
    if (typeof value === "number") {
      if (Number.isInteger(value)) {
        if (value >= -2147483648 && value <= 2147483647) {
          return { Int32: value };
        }
        return { Int64: value };
      }
      return { Float64: value };
    }
    if (typeof value === "string") {
      return { String: value };
    }
    if (value instanceof Date) {
      return { Timestamp: value.getTime() };
    }
    if (value instanceof Uint8Array || Buffer.isBuffer(value)) {
      return { Bytes: Array.from(value) };
    }
    if (Array.isArray(value)) {
      if (value.length === 0) {
        return { StringArray: [] };
      }
      const first = value[0];
      if (typeof first === "boolean") {
        return { BoolArray: value as boolean[] };
      }
      if (typeof first === "number") {
        if (Number.isInteger(first)) {
          return { Int64Array: value as number[] };
        }
        return { Float64Array: value as number[] };
      }
      if (typeof first === "string") {
        return { StringArray: value as string[] };
      }
      return { StringArray: value.map(String) };
    }
    if (typeof value === "object") {
      return { Json: value };
    }
    return { String: String(value) };
  }

  /**
   * Convert hex string to UUID byte array.
   */
  private hexToUuid(hex: string): number[] {
    // Remove dashes if present
    hex = hex.replace(/-/g, "");
    if (hex.length !== 32) {
      throw new OrmdbError(
        `Invalid UUID hex string: ${hex}`,
        "VALIDATION_ERROR"
      );
    }
    const bytes: number[] = [];
    for (let i = 0; i < 32; i += 2) {
      bytes.push(parseInt(hex.slice(i, i + 2), 16));
    }
    return bytes;
  }

  /**
   * Map HTTP status to error code.
   */
  private mapStatusToErrorCode(status: number): ErrorCode {
    if (status === 400) return "VALIDATION_ERROR";
    if (status === 404) return "QUERY_ERROR";
    if (status === 409) return "MUTATION_ERROR";
    if (status === 500) return "UNKNOWN_ERROR";
    if (status === 503) return "CONNECTION_ERROR";
    if (status === 504) return "TIMEOUT_ERROR";
    return "UNKNOWN_ERROR";
  }

  /**
   * Sleep for a given duration.
   */
  private sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}

export default OrmdbClient;
