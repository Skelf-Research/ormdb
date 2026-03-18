/**
 * Drizzle ORM adapter for ORMDB.
 *
 * This module provides Drizzle-compatible schema definitions and query
 * builders that work with ORMDB's HTTP gateway.
 *
 * @example
 * ```typescript
 * import { drizzle } from "@ormdb/client/drizzle";
 * import { ormdbTable, text, integer, boolean } from "@ormdb/client/drizzle";
 *
 * // Define schema
 * const users = ormdbTable("User", {
 *   id: text("id").primaryKey(),
 *   name: text("name").notNull(),
 *   email: text("email").notNull(),
 *   age: integer("age"),
 *   active: boolean("active").default(true),
 * });
 *
 * // Create client
 * const db = drizzle("http://localhost:8080");
 *
 * // Query
 * const activeUsers = await db.select().from(users).where(eq(users.active, true));
 *
 * // Insert
 * await db.insert(users).values({ name: "Alice", email: "alice@example.com" });
 * ```
 */

import { OrmdbClient } from "../client";
import type { OrmdbConfig, FilterExpr, OrderSpec } from "../types";

// ============================================================================
// Column Types
// ============================================================================

/** Column definition */
export interface Column<T = unknown> {
  name: string;
  dataType: string;
  notNull: boolean;
  hasDefault: boolean;
  defaultValue?: T;
  primaryKey: boolean;
  references?: { table: string; column: string };

  // Builder methods
  $type: T;
}

/** Column builder */
export class ColumnBuilder<T = unknown> implements Column<T> {
  name: string;
  dataType: string;
  notNull = false;
  hasDefault = false;
  defaultValue?: T;
  primaryKey = false;
  references?: { table: string; column: string };
  $type!: T;

  constructor(name: string, dataType: string) {
    this.name = name;
    this.dataType = dataType;
  }

  primaryKey(): this {
    this.primaryKey = true;
    this.notNull = true;
    return this;
  }

  notNull(): this {
    this.notNull = true;
    return this;
  }

  default(value: T): this {
    this.hasDefault = true;
    this.defaultValue = value;
    return this;
  }

  references(table: string, column: string): this {
    this.references = { table, column };
    return this;
  }
}

// Column type constructors
export function text(name: string): ColumnBuilder<string> {
  return new ColumnBuilder<string>(name, "string");
}

export function varchar(name: string, _config?: { length?: number }): ColumnBuilder<string> {
  return new ColumnBuilder<string>(name, "string");
}

export function integer(name: string): ColumnBuilder<number> {
  return new ColumnBuilder<number>(name, "int32");
}

export function bigint(name: string, _config?: { mode?: "number" | "bigint" }): ColumnBuilder<number | bigint> {
  return new ColumnBuilder<number | bigint>(name, "int64");
}

export function real(name: string): ColumnBuilder<number> {
  return new ColumnBuilder<number>(name, "float32");
}

export function doublePrecision(name: string): ColumnBuilder<number> {
  return new ColumnBuilder<number>(name, "float64");
}

export function boolean(name: string): ColumnBuilder<boolean> {
  return new ColumnBuilder<boolean>(name, "bool");
}

export function timestamp(name: string, _config?: { mode?: "string" | "date" }): ColumnBuilder<Date | string> {
  return new ColumnBuilder<Date | string>(name, "timestamp");
}

export function date(name: string): ColumnBuilder<string> {
  return new ColumnBuilder<string>(name, "date");
}

export function time(name: string): ColumnBuilder<string> {
  return new ColumnBuilder<string>(name, "time");
}

export function json<T = unknown>(name: string): ColumnBuilder<T> {
  return new ColumnBuilder<T>(name, "json");
}

export function uuid(name: string): ColumnBuilder<string> {
  return new ColumnBuilder<string>(name, "uuid");
}

export function blob(name: string): ColumnBuilder<Uint8Array> {
  return new ColumnBuilder<Uint8Array>(name, "bytes");
}

// ============================================================================
// Table Definition
// ============================================================================

/** Table schema */
export interface Table<TColumns extends Record<string, Column> = Record<string, Column>> {
  _: {
    name: string;
    columns: TColumns;
  };
}

/** Create an ORMDB table schema */
export function ormdbTable<TColumns extends Record<string, ColumnBuilder>>(
  name: string,
  columns: TColumns
): Table<TColumns> & TColumns {
  const table: Table<TColumns> = {
    _: { name, columns: columns as unknown as TColumns },
  };

  // Add columns as properties for query builder access
  return Object.assign(table, columns) as Table<TColumns> & TColumns;
}

// ============================================================================
// Query Operators
// ============================================================================

/** SQL expression */
export interface SQL {
  sql: string;
  params: unknown[];
}

/** Comparison result for where clauses */
export interface Condition {
  type: "comparison" | "and" | "or" | "not";
  field?: string;
  op?: string;
  value?: unknown;
  conditions?: Condition[];
}

// Comparison operators
export function eq<T>(column: Column<T>, value: T): Condition {
  return { type: "comparison", field: column.name, op: "eq", value };
}

export function ne<T>(column: Column<T>, value: T): Condition {
  return { type: "comparison", field: column.name, op: "ne", value };
}

export function lt<T>(column: Column<T>, value: T): Condition {
  return { type: "comparison", field: column.name, op: "lt", value };
}

export function lte<T>(column: Column<T>, value: T): Condition {
  return { type: "comparison", field: column.name, op: "le", value };
}

export function gt<T>(column: Column<T>, value: T): Condition {
  return { type: "comparison", field: column.name, op: "gt", value };
}

export function gte<T>(column: Column<T>, value: T): Condition {
  return { type: "comparison", field: column.name, op: "ge", value };
}

export function like(column: Column<string>, pattern: string): Condition {
  return { type: "comparison", field: column.name, op: "like", value: pattern };
}

export function ilike(column: Column<string>, pattern: string): Condition {
  return { type: "comparison", field: column.name, op: "ilike", value: pattern };
}

export function inArray<T>(column: Column<T>, values: T[]): Condition {
  return { type: "comparison", field: column.name, op: "in", value: values };
}

export function notInArray<T>(column: Column<T>, values: T[]): Condition {
  return { type: "comparison", field: column.name, op: "not_in", value: values };
}

export function isNull(column: Column): Condition {
  return { type: "comparison", field: column.name, op: "is_null" };
}

export function isNotNull(column: Column): Condition {
  return { type: "comparison", field: column.name, op: "is_not_null" };
}

// Logical operators
export function and(...conditions: Condition[]): Condition {
  return { type: "and", conditions };
}

export function or(...conditions: Condition[]): Condition {
  return { type: "or", conditions };
}

export function not(condition: Condition): Condition {
  return { type: "not", conditions: [condition] };
}

// Order direction
export function asc(column: Column): { column: Column; direction: "asc" } {
  return { column, direction: "asc" };
}

export function desc(column: Column): { column: Column; direction: "desc" } {
  return { column, direction: "desc" };
}

// ============================================================================
// Query Builders
// ============================================================================

/** Convert condition to ORMDB filter */
function conditionToFilter(condition: Condition): FilterExpr {
  switch (condition.type) {
    case "comparison":
      return {
        field: condition.field!,
        op: condition.op as FilterExpr["op"],
        value: condition.value,
      };
    case "and":
      return { and: condition.conditions!.map(conditionToFilter) };
    case "or":
      return { or: condition.conditions!.map(conditionToFilter) };
    case "not":
      return { not: conditionToFilter(condition.conditions![0]) };
  }
}

/** Select query builder */
export class SelectBuilder<T extends Table> {
  private client: OrmdbClient;
  private table: T;
  private selectedFields?: string[];
  private whereCondition?: Condition;
  private orderBySpecs: OrderSpec[] = [];
  private limitValue?: number;
  private offsetValue?: number;

  constructor(client: OrmdbClient, table: T) {
    this.client = client;
    this.table = table;
  }

  from<TTable extends Table>(table: TTable): SelectBuilder<TTable> {
    return new SelectBuilder(this.client, table);
  }

  where(condition: Condition): this {
    this.whereCondition = condition;
    return this;
  }

  orderBy(...specs: { column: Column; direction: "asc" | "desc" }[]): this {
    this.orderBySpecs = specs.map((s) => ({
      field: s.column.name,
      direction: s.direction,
    }));
    return this;
  }

  limit(n: number): this {
    this.limitValue = n;
    return this;
  }

  offset(n: number): this {
    this.offsetValue = n;
    return this;
  }

  async execute(): Promise<Record<string, unknown>[]> {
    const filter = this.whereCondition
      ? conditionToFilter(this.whereCondition)
      : undefined;

    const result = await this.client.query(this.table._.name, {
      fields: this.selectedFields,
      filter,
      orderBy: this.orderBySpecs.length > 0 ? this.orderBySpecs : undefined,
      limit: this.limitValue,
      offset: this.offsetValue,
    });

    return result.entities;
  }

  then<TResult1 = Record<string, unknown>[], TResult2 = never>(
    onfulfilled?: (value: Record<string, unknown>[]) => TResult1 | PromiseLike<TResult1>,
    onrejected?: (reason: unknown) => TResult2 | PromiseLike<TResult2>
  ): Promise<TResult1 | TResult2> {
    return this.execute().then(onfulfilled, onrejected);
  }
}

/** Insert query builder */
export class InsertBuilder<T extends Table> {
  private client: OrmdbClient;
  private table: T;
  private data: Record<string, unknown>[] = [];

  constructor(client: OrmdbClient, table: T) {
    this.client = client;
    this.table = table;
  }

  values(data: Record<string, unknown> | Record<string, unknown>[]): this {
    this.data = Array.isArray(data) ? data : [data];
    return this;
  }

  async returning(): Promise<Record<string, unknown>[]> {
    const results: Record<string, unknown>[] = [];

    for (const row of this.data) {
      const result = await this.client.insert(this.table._.name, row);
      if (result.insertedIds[0]) {
        const inserted = await this.client.findById(
          this.table._.name,
          result.insertedIds[0]
        );
        if (inserted) results.push(inserted);
      }
    }

    return results;
  }

  async execute(): Promise<{ rowCount: number }> {
    let rowCount = 0;

    for (const row of this.data) {
      const result = await this.client.insert(this.table._.name, row);
      rowCount += result.affected;
    }

    return { rowCount };
  }

  then<TResult1 = { rowCount: number }, TResult2 = never>(
    onfulfilled?: (value: { rowCount: number }) => TResult1 | PromiseLike<TResult1>,
    onrejected?: (reason: unknown) => TResult2 | PromiseLike<TResult2>
  ): Promise<TResult1 | TResult2> {
    return this.execute().then(onfulfilled, onrejected);
  }
}

/** Update query builder */
export class UpdateBuilder<T extends Table> {
  private client: OrmdbClient;
  private table: T;
  private data?: Record<string, unknown>;
  private whereCondition?: Condition;

  constructor(client: OrmdbClient, table: T) {
    this.client = client;
    this.table = table;
  }

  set(data: Record<string, unknown>): this {
    this.data = data;
    return this;
  }

  where(condition: Condition): this {
    this.whereCondition = condition;
    return this;
  }

  async execute(): Promise<{ rowCount: number }> {
    if (!this.data) {
      throw new Error("No data to update");
    }

    const filter = this.whereCondition
      ? conditionToFilter(this.whereCondition)
      : undefined;

    if (!filter) {
      throw new Error("WHERE clause is required for UPDATE");
    }

    const result = await this.client.updateMany(
      this.table._.name,
      filter,
      this.data
    );

    return { rowCount: result.affected };
  }

  then<TResult1 = { rowCount: number }, TResult2 = never>(
    onfulfilled?: (value: { rowCount: number }) => TResult1 | PromiseLike<TResult1>,
    onrejected?: (reason: unknown) => TResult2 | PromiseLike<TResult2>
  ): Promise<TResult1 | TResult2> {
    return this.execute().then(onfulfilled, onrejected);
  }
}

/** Delete query builder */
export class DeleteBuilder<T extends Table> {
  private client: OrmdbClient;
  private table: T;
  private whereCondition?: Condition;

  constructor(client: OrmdbClient, table: T) {
    this.client = client;
    this.table = table;
  }

  where(condition: Condition): this {
    this.whereCondition = condition;
    return this;
  }

  async execute(): Promise<{ rowCount: number }> {
    const filter = this.whereCondition
      ? conditionToFilter(this.whereCondition)
      : undefined;

    if (!filter) {
      throw new Error("WHERE clause is required for DELETE");
    }

    const result = await this.client.deleteMany(this.table._.name, filter);

    return { rowCount: result.affected };
  }

  then<TResult1 = { rowCount: number }, TResult2 = never>(
    onfulfilled?: (value: { rowCount: number }) => TResult1 | PromiseLike<TResult1>,
    onrejected?: (reason: unknown) => TResult2 | PromiseLike<TResult2>
  ): Promise<TResult1 | TResult2> {
    return this.execute().then(onfulfilled, onrejected);
  }
}

// ============================================================================
// Drizzle Client
// ============================================================================

/** Drizzle-compatible database client */
export interface DrizzleClient {
  select(fields?: Record<string, Column>): SelectBuilder<Table>;
  insert<T extends Table>(table: T): InsertBuilder<T>;
  update<T extends Table>(table: T): UpdateBuilder<T>;
  delete<T extends Table>(table: T): DeleteBuilder<T>;
}

/**
 * Create a Drizzle-compatible client for ORMDB.
 */
export function drizzle(config: OrmdbConfig | string): DrizzleClient {
  const client = new OrmdbClient(config);

  // Create a dummy table for initial select
  const dummyTable: Table = { _: { name: "", columns: {} } };

  return {
    select(_fields?: Record<string, Column>) {
      return new SelectBuilder(client, dummyTable);
    },

    insert<T extends Table>(table: T) {
      return new InsertBuilder(client, table);
    },

    update<T extends Table>(table: T) {
      return new UpdateBuilder(client, table);
    },

    delete<T extends Table>(table: T) {
      return new DeleteBuilder(client, table);
    },
  };
}

export default drizzle;
