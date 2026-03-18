/**
 * Kysely dialect for ORMDB.
 *
 * This module provides a Kysely-compatible dialect that translates
 * queries to ORMDB's HTTP gateway.
 *
 * @example
 * ```typescript
 * import { Kysely } from "kysely";
 * import { OrmdbDialect } from "@ormdb/client/kysely";
 *
 * interface Database {
 *   User: {
 *     id: string;
 *     name: string;
 *     email: string;
 *     age: number;
 *   };
 *   Post: {
 *     id: string;
 *     title: string;
 *     userId: string;
 *   };
 * }
 *
 * const db = new Kysely<Database>({
 *   dialect: new OrmdbDialect({
 *     baseUrl: "http://localhost:8080",
 *   }),
 * });
 *
 * // Type-safe queries
 * const users = await db
 *   .selectFrom("User")
 *   .selectAll()
 *   .where("age", ">", 18)
 *   .orderBy("name", "asc")
 *   .execute();
 * ```
 */

import type {
  Dialect,
  DialectAdapter,
  Driver,
  DatabaseConnection,
  QueryCompiler,
  QueryResult,
  CompiledQuery,
  DatabaseIntrospector,
  DialectAdapterBase,
  Kysely,
  TableMetadata,
  SchemaMetadata,
} from "kysely";

import { OrmdbClient } from "../client";
import type { OrmdbConfig, FilterExpr, OrderSpec } from "../types";

// ============================================================================
// Dialect Configuration
// ============================================================================

/** ORMDB dialect configuration */
export interface OrmdbDialectConfig extends OrmdbConfig {}

// ============================================================================
// Dialect Implementation
// ============================================================================

/**
 * Kysely dialect for ORMDB.
 */
export class OrmdbDialect implements Dialect {
  private config: OrmdbDialectConfig;

  constructor(config: OrmdbDialectConfig | string) {
    this.config = typeof config === "string" ? { baseUrl: config } : config;
  }

  createDriver(): Driver {
    return new OrmdbDriver(this.config);
  }

  createQueryCompiler(): QueryCompiler {
    return new OrmdbQueryCompiler();
  }

  createAdapter(): DialectAdapter {
    return new OrmdbAdapter();
  }

  createIntrospector(db: Kysely<unknown>): DatabaseIntrospector {
    return new OrmdbIntrospector(db, this.config);
  }
}

// ============================================================================
// Driver Implementation
// ============================================================================

/**
 * Kysely driver for ORMDB.
 */
class OrmdbDriver implements Driver {
  private config: OrmdbDialectConfig;
  private client: OrmdbClient | null = null;

  constructor(config: OrmdbDialectConfig) {
    this.config = config;
  }

  async init(): Promise<void> {
    this.client = new OrmdbClient(this.config);
    await this.client.health();
  }

  async acquireConnection(): Promise<DatabaseConnection> {
    if (!this.client) {
      throw new Error("Driver not initialized");
    }
    return new OrmdbConnection(this.client);
  }

  async beginTransaction(_connection: DatabaseConnection): Promise<void> {
    // ORMDB doesn't support transactions yet
  }

  async commitTransaction(_connection: DatabaseConnection): Promise<void> {
    // ORMDB auto-commits
  }

  async rollbackTransaction(_connection: DatabaseConnection): Promise<void> {
    // ORMDB doesn't support rollback
  }

  async releaseConnection(_connection: DatabaseConnection): Promise<void> {
    // HTTP connections don't need explicit release
  }

  async destroy(): Promise<void> {
    this.client = null;
  }
}

// ============================================================================
// Connection Implementation
// ============================================================================

/**
 * Database connection for ORMDB.
 */
class OrmdbConnection implements DatabaseConnection {
  private client: OrmdbClient;

  constructor(client: OrmdbClient) {
    this.client = client;
  }

  async executeQuery<R>(compiledQuery: CompiledQuery): Promise<QueryResult<R>> {
    const { sql, parameters } = compiledQuery;

    // Parse the compiled query to determine operation type
    const parsed = this.parseQuery(sql, parameters as unknown[]);

    switch (parsed.type) {
      case "select":
        return this.executeSelect<R>(parsed);
      case "insert":
        return this.executeInsert<R>(parsed);
      case "update":
        return this.executeUpdate<R>(parsed);
      case "delete":
        return this.executeDelete<R>(parsed);
      default:
        throw new Error(`Unsupported query type: ${parsed.type}`);
    }
  }

  private async executeSelect<R>(query: ParsedQuery): Promise<QueryResult<R>> {
    const result = await this.client.query(query.table, {
      fields: query.fields,
      filter: query.filter,
      orderBy: query.orderBy,
      limit: query.limit,
      offset: query.offset,
    });

    return {
      rows: result.entities as R[],
    };
  }

  private async executeInsert<R>(query: ParsedQuery): Promise<QueryResult<R>> {
    if (!query.values) {
      throw new Error("INSERT requires values");
    }

    const result = await this.client.insert(query.table, query.values);

    return {
      insertId: result.insertedIds[0] ? BigInt(0) : undefined,
      numAffectedRows: BigInt(result.affected),
      rows: [] as R[],
    };
  }

  private async executeUpdate<R>(query: ParsedQuery): Promise<QueryResult<R>> {
    if (!query.values || !query.filter) {
      throw new Error("UPDATE requires values and WHERE clause");
    }

    const result = await this.client.updateMany(
      query.table,
      query.filter,
      query.values
    );

    return {
      numAffectedRows: BigInt(result.affected),
      rows: [] as R[],
    };
  }

  private async executeDelete<R>(query: ParsedQuery): Promise<QueryResult<R>> {
    if (!query.filter) {
      throw new Error("DELETE requires WHERE clause");
    }

    const result = await this.client.deleteMany(query.table, query.filter);

    return {
      numAffectedRows: BigInt(result.affected),
      rows: [] as R[],
    };
  }

  private parseQuery(sql: string, parameters: unknown[]): ParsedQuery {
    // Parse SQL-like query generated by Kysely
    const normalizedSql = sql.trim().toUpperCase();

    if (normalizedSql.startsWith("SELECT")) {
      return this.parseSelect(sql, parameters);
    } else if (normalizedSql.startsWith("INSERT")) {
      return this.parseInsert(sql, parameters);
    } else if (normalizedSql.startsWith("UPDATE")) {
      return this.parseUpdate(sql, parameters);
    } else if (normalizedSql.startsWith("DELETE")) {
      return this.parseDelete(sql, parameters);
    }

    throw new Error(`Cannot parse query: ${sql}`);
  }

  private parseSelect(sql: string, parameters: unknown[]): ParsedQuery {
    const result: ParsedQuery = { type: "select", table: "" };

    // Extract table name from FROM clause
    const fromMatch = sql.match(/FROM\s+["']?(\w+)["']?/i);
    if (fromMatch) {
      result.table = fromMatch[1];
    }

    // Extract fields from SELECT clause
    const selectMatch = sql.match(/SELECT\s+(.+?)\s+FROM/i);
    if (selectMatch && selectMatch[1] !== "*") {
      result.fields = selectMatch[1]
        .split(",")
        .map((f) => f.trim().replace(/["']/g, ""))
        .filter((f) => f !== "*");
    }

    // Extract WHERE clause
    const whereMatch = sql.match(/WHERE\s+(.+?)(?:\s+ORDER|\s+LIMIT|\s*$)/i);
    if (whereMatch) {
      result.filter = this.parseWhere(whereMatch[1], parameters);
    }

    // Extract ORDER BY clause
    const orderMatch = sql.match(/ORDER BY\s+(.+?)(?:\s+LIMIT|\s*$)/i);
    if (orderMatch) {
      result.orderBy = this.parseOrderBy(orderMatch[1]);
    }

    // Extract LIMIT and OFFSET
    const limitMatch = sql.match(/LIMIT\s+(\$\d+|\d+)/i);
    if (limitMatch) {
      const limitVal = limitMatch[1].startsWith("$")
        ? parameters[parseInt(limitMatch[1].slice(1)) - 1]
        : parseInt(limitMatch[1]);
      result.limit = limitVal as number;
    }

    const offsetMatch = sql.match(/OFFSET\s+(\$\d+|\d+)/i);
    if (offsetMatch) {
      const offsetVal = offsetMatch[1].startsWith("$")
        ? parameters[parseInt(offsetMatch[1].slice(1)) - 1]
        : parseInt(offsetMatch[1]);
      result.offset = offsetVal as number;
    }

    return result;
  }

  private parseInsert(sql: string, parameters: unknown[]): ParsedQuery {
    const result: ParsedQuery = { type: "insert", table: "" };

    // Extract table name
    const intoMatch = sql.match(/INSERT INTO\s+["']?(\w+)["']?/i);
    if (intoMatch) {
      result.table = intoMatch[1];
    }

    // Extract columns
    const columnsMatch = sql.match(/\(([^)]+)\)\s+VALUES/i);
    if (columnsMatch) {
      const columns = columnsMatch[1]
        .split(",")
        .map((c) => c.trim().replace(/["']/g, ""));

      // Map parameters to columns
      result.values = {};
      columns.forEach((col, i) => {
        if (parameters[i] !== undefined) {
          result.values![col] = parameters[i];
        }
      });
    }

    return result;
  }

  private parseUpdate(sql: string, parameters: unknown[]): ParsedQuery {
    const result: ParsedQuery = { type: "update", table: "" };

    // Extract table name
    const tableMatch = sql.match(/UPDATE\s+["']?(\w+)["']?/i);
    if (tableMatch) {
      result.table = tableMatch[1];
    }

    // Extract SET clause
    const setMatch = sql.match(/SET\s+(.+?)\s+WHERE/i);
    if (setMatch) {
      result.values = {};
      const assignments = setMatch[1].split(",");
      let paramIndex = 0;

      for (const assignment of assignments) {
        const [col] = assignment.split("=").map((s) => s.trim());
        const cleanCol = col.replace(/["']/g, "");
        result.values[cleanCol] = parameters[paramIndex++];
      }
    }

    // Extract WHERE clause
    const whereMatch = sql.match(/WHERE\s+(.+)$/i);
    if (whereMatch) {
      const remainingParams = parameters.slice(
        Object.keys(result.values || {}).length
      );
      result.filter = this.parseWhere(whereMatch[1], remainingParams);
    }

    return result;
  }

  private parseDelete(sql: string, parameters: unknown[]): ParsedQuery {
    const result: ParsedQuery = { type: "delete", table: "" };

    // Extract table name
    const fromMatch = sql.match(/DELETE FROM\s+["']?(\w+)["']?/i);
    if (fromMatch) {
      result.table = fromMatch[1];
    }

    // Extract WHERE clause
    const whereMatch = sql.match(/WHERE\s+(.+)$/i);
    if (whereMatch) {
      result.filter = this.parseWhere(whereMatch[1], parameters);
    }

    return result;
  }

  private parseWhere(whereClause: string, parameters: unknown[]): FilterExpr {
    // Simple parser for WHERE clauses
    // Handles: field = $1, field > $1, field IN ($1, $2), AND, OR

    const conditions: FilterExpr[] = [];
    let paramIndex = 0;

    // Split by AND (simple approach, doesn't handle nested OR)
    const parts = whereClause.split(/\s+AND\s+/i);

    for (const part of parts) {
      // Handle OR
      if (/\s+OR\s+/i.test(part)) {
        const orParts = part.split(/\s+OR\s+/i);
        const orConditions = orParts.map((p) => {
          const condition = this.parseSingleCondition(p, parameters, paramIndex);
          paramIndex += this.countParams(p);
          return condition;
        });
        conditions.push({ or: orConditions });
      } else {
        const condition = this.parseSingleCondition(part, parameters, paramIndex);
        paramIndex += this.countParams(part);
        conditions.push(condition);
      }
    }

    if (conditions.length === 1) {
      return conditions[0];
    }
    return { and: conditions };
  }

  private parseSingleCondition(
    condition: string,
    parameters: unknown[],
    paramOffset: number
  ): FilterExpr {
    // Parse: field op $n or field op value
    const match = condition.match(
      /["']?(\w+)["']?\s*(=|!=|<>|<|>|<=|>=|LIKE|ILIKE|IN|IS NULL|IS NOT NULL)\s*(.+)?/i
    );

    if (!match) {
      return { and: [] }; // Empty condition
    }

    const [, field, op, valueStr] = match;
    const normalizedOp = op.toUpperCase();

    if (normalizedOp === "IS NULL") {
      return { field, op: "is_null" };
    }

    if (normalizedOp === "IS NOT NULL") {
      return { field, op: "is_not_null" };
    }

    // Get value from parameters or parse literal
    let value: unknown;
    if (valueStr?.startsWith("$")) {
      const paramNum = parseInt(valueStr.slice(1)) - 1;
      value = parameters[paramNum];
    } else if (normalizedOp === "IN") {
      // Parse IN clause
      const inMatch = valueStr?.match(/\((.+)\)/);
      if (inMatch) {
        const inValues = inMatch[1].split(",").map((v) => {
          v = v.trim();
          if (v.startsWith("$")) {
            return parameters[parseInt(v.slice(1)) - 1];
          }
          return this.parseLiteral(v);
        });
        value = inValues;
      }
    } else {
      value = valueStr ? this.parseLiteral(valueStr.trim()) : parameters[paramOffset];
    }

    const opMap: Record<string, FilterExpr["op"]> = {
      "=": "eq",
      "!=": "ne",
      "<>": "ne",
      "<": "lt",
      ">": "gt",
      "<=": "le",
      ">=": "ge",
      LIKE: "like",
      ILIKE: "ilike",
      IN: "in",
    };

    return {
      field,
      op: opMap[normalizedOp] ?? "eq",
      value,
    };
  }

  private parseLiteral(value: string): unknown {
    // Remove quotes
    if (
      (value.startsWith("'") && value.endsWith("'")) ||
      (value.startsWith('"') && value.endsWith('"'))
    ) {
      return value.slice(1, -1);
    }

    // Parse numbers
    if (/^-?\d+$/.test(value)) {
      return parseInt(value);
    }
    if (/^-?\d+\.\d+$/.test(value)) {
      return parseFloat(value);
    }

    // Parse booleans
    if (value.toUpperCase() === "TRUE") return true;
    if (value.toUpperCase() === "FALSE") return false;
    if (value.toUpperCase() === "NULL") return null;

    return value;
  }

  private parseOrderBy(orderClause: string): OrderSpec[] {
    const specs: OrderSpec[] = [];
    const parts = orderClause.split(",");

    for (const part of parts) {
      const match = part.trim().match(/["']?(\w+)["']?\s*(ASC|DESC)?/i);
      if (match) {
        specs.push({
          field: match[1],
          direction: (match[2]?.toLowerCase() ?? "asc") as "asc" | "desc",
        });
      }
    }

    return specs;
  }

  private countParams(str: string): number {
    const matches = str.match(/\$\d+/g);
    return matches?.length ?? 0;
  }

  async *streamQuery<R>(
    _compiledQuery: CompiledQuery,
    _chunkSize?: number
  ): AsyncIterableIterator<QueryResult<R>> {
    throw new Error("Streaming not supported");
  }
}

/** Parsed query structure */
interface ParsedQuery {
  type: "select" | "insert" | "update" | "delete";
  table: string;
  fields?: string[];
  filter?: FilterExpr;
  orderBy?: OrderSpec[];
  limit?: number;
  offset?: number;
  values?: Record<string, unknown>;
}

// ============================================================================
// Query Compiler
// ============================================================================

/**
 * Query compiler for ORMDB.
 * Uses standard SQL-like syntax that our connection can parse.
 */
class OrmdbQueryCompiler implements QueryCompiler {
  compileQuery(node: unknown): CompiledQuery {
    // Kysely will call this with its internal query nodes
    // We'll compile to a SQL-like format that our connection understands
    const sql = this.compileNode(node);
    const parameters = this.extractParameters(node);

    return {
      sql,
      parameters,
      query: node,
    };
  }

  private compileNode(node: unknown): string {
    // Type guard for node structure
    const n = node as Record<string, unknown>;

    if (!n || typeof n !== "object") {
      return "";
    }

    // Handle different node types based on Kysely's internal structure
    const kind = n.kind as string;

    switch (kind) {
      case "SelectQueryNode":
        return this.compileSelect(n);
      case "InsertQueryNode":
        return this.compileInsert(n);
      case "UpdateQueryNode":
        return this.compileUpdate(n);
      case "DeleteQueryNode":
        return this.compileDelete(n);
      default:
        // Fallback: try to reconstruct from node structure
        return this.compileGeneric(n);
    }
  }

  private compileSelect(node: Record<string, unknown>): string {
    let sql = "SELECT ";

    // Compile selections
    const selections = node.selections as unknown[];
    if (selections?.length) {
      sql += selections.map((s) => this.compileSelection(s)).join(", ");
    } else {
      sql += "*";
    }

    // Compile FROM
    const from = node.from as Record<string, unknown>;
    if (from?.froms) {
      const tables = from.froms as unknown[];
      sql += " FROM " + tables.map((t) => this.compileTable(t)).join(", ");
    }

    // Compile WHERE
    const where = node.where as Record<string, unknown>;
    if (where?.where) {
      sql += " WHERE " + this.compileWhere(where.where);
    }

    // Compile ORDER BY
    const orderBy = node.orderBy as Record<string, unknown>;
    if (orderBy?.items) {
      const items = orderBy.items as unknown[];
      sql +=
        " ORDER BY " + items.map((i) => this.compileOrderByItem(i)).join(", ");
    }

    // Compile LIMIT
    const limit = node.limit as Record<string, unknown>;
    if (limit?.limit) {
      sql += " LIMIT " + this.compileValue(limit.limit);
    }

    // Compile OFFSET
    const offset = node.offset as Record<string, unknown>;
    if (offset?.offset) {
      sql += " OFFSET " + this.compileValue(offset.offset);
    }

    return sql;
  }

  private compileInsert(node: Record<string, unknown>): string {
    let sql = "INSERT INTO ";

    // Table
    const into = node.into as Record<string, unknown>;
    if (into?.table) {
      sql += this.compileTable(into.table);
    }

    // Columns and values
    const columns = node.columns as unknown[];
    const values = node.values as unknown[];

    if (columns?.length) {
      sql += " (" + columns.map((c) => this.compileColumn(c)).join(", ") + ")";
    }

    if (values?.length) {
      const valueList = values[0] as Record<string, unknown>;
      if (valueList?.values) {
        const vals = valueList.values as unknown[];
        sql += " VALUES (" + vals.map((v) => this.compileValue(v)).join(", ") + ")";
      }
    }

    return sql;
  }

  private compileUpdate(node: Record<string, unknown>): string {
    let sql = "UPDATE ";

    // Table
    const table = node.table as Record<string, unknown>;
    if (table?.table) {
      sql += this.compileTable(table.table);
    }

    // SET clause
    const updates = node.updates as unknown[];
    if (updates?.length) {
      sql +=
        " SET " + updates.map((u) => this.compileUpdateItem(u)).join(", ");
    }

    // WHERE
    const where = node.where as Record<string, unknown>;
    if (where?.where) {
      sql += " WHERE " + this.compileWhere(where.where);
    }

    return sql;
  }

  private compileDelete(node: Record<string, unknown>): string {
    let sql = "DELETE FROM ";

    // Table
    const from = node.from as Record<string, unknown>;
    if (from?.froms) {
      const tables = from.froms as unknown[];
      sql += tables.map((t) => this.compileTable(t)).join(", ");
    }

    // WHERE
    const where = node.where as Record<string, unknown>;
    if (where?.where) {
      sql += " WHERE " + this.compileWhere(where.where);
    }

    return sql;
  }

  private compileGeneric(node: Record<string, unknown>): string {
    // Fallback for unknown node types
    return JSON.stringify(node);
  }

  private compileSelection(sel: unknown): string {
    const s = sel as Record<string, unknown>;
    if (s.selection) {
      return this.compileColumn(s.selection);
    }
    return "*";
  }

  private compileTable(table: unknown): string {
    const t = table as Record<string, unknown>;
    if (t.table?.identifier?.name) {
      return `"${(t.table as Record<string, unknown>).identifier as Record<string, string>}"`;
    }
    if (t.identifier?.name) {
      return `"${(t.identifier as Record<string, string>).name}"`;
    }
    if (typeof t === "string") {
      return `"${t}"`;
    }
    return String(t);
  }

  private compileColumn(col: unknown): string {
    const c = col as Record<string, unknown>;
    if (c.column?.name) {
      return `"${(c.column as Record<string, string>).name}"`;
    }
    if (c.name) {
      return `"${c.name}"`;
    }
    return String(col);
  }

  private compileWhere(where: unknown): string {
    const w = where as Record<string, unknown>;
    const kind = w.kind as string;

    switch (kind) {
      case "BinaryOperationNode":
        return this.compileBinaryOp(w);
      case "AndNode":
        return this.compileAnd(w);
      case "OrNode":
        return this.compileOr(w);
      default:
        return this.compileValue(where);
    }
  }

  private compileBinaryOp(node: Record<string, unknown>): string {
    const left = this.compileValue(node.leftOperand);
    const op = (node.operator as Record<string, string>)?.operator ?? "=";
    const right = this.compileValue(node.rightOperand);
    return `${left} ${op} ${right}`;
  }

  private compileAnd(node: Record<string, unknown>): string {
    const left = this.compileWhere(node.left);
    const right = this.compileWhere(node.right);
    return `(${left} AND ${right})`;
  }

  private compileOr(node: Record<string, unknown>): string {
    const left = this.compileWhere(node.left);
    const right = this.compileWhere(node.right);
    return `(${left} OR ${right})`;
  }

  private compileOrderByItem(item: unknown): string {
    const i = item as Record<string, unknown>;
    const col = this.compileColumn(i.orderBy);
    const dir = (i.direction as string)?.toUpperCase() ?? "ASC";
    return `${col} ${dir}`;
  }

  private compileUpdateItem(item: unknown): string {
    const i = item as Record<string, unknown>;
    const col = this.compileColumn(i.column);
    const val = this.compileValue(i.value);
    return `${col} = ${val}`;
  }

  private compileValue(val: unknown): string {
    if (val === null || val === undefined) {
      return "NULL";
    }

    const v = val as Record<string, unknown>;
    const kind = v.kind as string;

    switch (kind) {
      case "ValueNode":
        return `$${v.parameterIndex ?? 1}`;
      case "ColumnNode":
        return this.compileColumn(v);
      case "ReferenceNode":
        return this.compileColumn(v.column);
      default:
        if (typeof val === "string") {
          return `'${val}'`;
        }
        if (typeof val === "number" || typeof val === "boolean") {
          return String(val);
        }
        return String(val);
    }
  }

  private extractParameters(node: unknown): unknown[] {
    const params: unknown[] = [];
    this.collectParameters(node, params);
    return params;
  }

  private collectParameters(node: unknown, params: unknown[]): void {
    if (!node || typeof node !== "object") return;

    const n = node as Record<string, unknown>;

    if (n.kind === "ValueNode" && "value" in n) {
      params.push(n.value);
    }

    for (const value of Object.values(n)) {
      if (Array.isArray(value)) {
        for (const item of value) {
          this.collectParameters(item, params);
        }
      } else if (value && typeof value === "object") {
        this.collectParameters(value, params);
      }
    }
  }
}

// ============================================================================
// Adapter
// ============================================================================

/**
 * Dialect adapter for ORMDB.
 */
class OrmdbAdapter implements DialectAdapterBase {
  get supportsTransactionalDdl(): boolean {
    return false;
  }

  get supportsReturning(): boolean {
    return false;
  }

  async acquireMigrationLock(_db: Kysely<unknown>): Promise<void> {
    // No-op
  }

  async releaseMigrationLock(_db: Kysely<unknown>): Promise<void> {
    // No-op
  }
}

// ============================================================================
// Introspector
// ============================================================================

/**
 * Database introspector for ORMDB.
 */
class OrmdbIntrospector implements DatabaseIntrospector {
  private db: Kysely<unknown>;
  private config: OrmdbDialectConfig;

  constructor(db: Kysely<unknown>, config: OrmdbDialectConfig) {
    this.db = db;
    this.config = config;
  }

  async getSchemas(): Promise<SchemaMetadata[]> {
    return [{ name: "public" }];
  }

  async getTables(_options?: { withInternalKyselyTables?: boolean }): Promise<TableMetadata[]> {
    const client = new OrmdbClient(this.config);
    const schema = await client.getSchema();

    return schema.entities.map((entity) => ({
      name: entity.name,
      schema: "public",
      isView: false,
      columns: entity.fields.map((field) => ({
        name: field.name,
        dataType: field.field_type,
        isNullable: !field.required,
        isAutoIncrementing: field.name === "id",
        hasDefaultValue: field.default !== undefined,
      })),
    }));
  }

  async getMetadata(_options?: { withInternalKyselyTables?: boolean }): Promise<{
    tables: TableMetadata[];
  }> {
    const tables = await this.getTables(_options);
    return { tables };
  }
}

// ============================================================================
// Exports
// ============================================================================

export default OrmdbDialect;
