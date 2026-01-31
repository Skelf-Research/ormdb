/**
 * Sequelize dialect for ORMDB.
 *
 * This module provides a Sequelize-compatible interface that works
 * with ORMDB's HTTP gateway.
 *
 * @example
 * ```typescript
 * import { OrmdbSequelize, DataTypes } from "@ormdb/client/sequelize";
 *
 * const sequelize = new OrmdbSequelize("http://localhost:8080");
 *
 * const User = sequelize.define("User", {
 *   name: DataTypes.STRING,
 *   email: DataTypes.STRING,
 *   age: DataTypes.INTEGER,
 * });
 *
 * // Query
 * const users = await User.findAll({
 *   where: { age: { [Op.gt]: 18 } },
 * });
 *
 * // Create
 * const user = await User.create({ name: "Alice", email: "alice@example.com" });
 * ```
 */

import { OrmdbClient } from "../client";
import type { OrmdbConfig, FilterExpr, OrderSpec } from "../types";

// ============================================================================
// Data Types
// ============================================================================

/** Sequelize-compatible data types */
export const DataTypes = {
  STRING: { type: "string" },
  TEXT: { type: "text" },
  INTEGER: { type: "int32" },
  BIGINT: { type: "int64" },
  FLOAT: { type: "float32" },
  DOUBLE: { type: "float64" },
  BOOLEAN: { type: "bool" },
  DATE: { type: "timestamp" },
  DATEONLY: { type: "date" },
  TIME: { type: "time" },
  UUID: { type: "uuid" },
  UUIDV4: { type: "uuid" },
  JSON: { type: "json" },
  JSONB: { type: "json" },
  BLOB: { type: "bytes" },
  ARRAY: (itemType: { type: string }) => ({
    type: `${itemType.type}[]`,
    isArray: true,
  }),
} as const;

// ============================================================================
// Operators
// ============================================================================

/** Sequelize operators */
export const Op = {
  eq: Symbol("eq"),
  ne: Symbol("ne"),
  is: Symbol("is"),
  not: Symbol("not"),
  or: Symbol("or"),
  and: Symbol("and"),
  gt: Symbol("gt"),
  gte: Symbol("gte"),
  lt: Symbol("lt"),
  lte: Symbol("lte"),
  between: Symbol("between"),
  notBetween: Symbol("notBetween"),
  in: Symbol("in"),
  notIn: Symbol("notIn"),
  like: Symbol("like"),
  notLike: Symbol("notLike"),
  iLike: Symbol("iLike"),
  notILike: Symbol("notILike"),
  startsWith: Symbol("startsWith"),
  endsWith: Symbol("endsWith"),
  substring: Symbol("substring"),
  regexp: Symbol("regexp"),
  notRegexp: Symbol("notRegexp"),
  iRegexp: Symbol("iRegexp"),
  notIRegexp: Symbol("notIRegexp"),
} as const;

/** Operator type */
export type OperatorSymbol = (typeof Op)[keyof typeof Op];

// ============================================================================
// Types
// ============================================================================

/** Model attribute definition */
export interface ModelAttributeColumnOptions {
  type: { type: string; isArray?: boolean };
  primaryKey?: boolean;
  autoIncrement?: boolean;
  allowNull?: boolean;
  unique?: boolean;
  defaultValue?: unknown;
  field?: string;
  references?: {
    model: string;
    key: string;
  };
}

/** Model attributes */
export type ModelAttributes = Record<
  string,
  ModelAttributeColumnOptions | { type: string }
>;

/** Model options */
export interface ModelOptions {
  tableName?: string;
  timestamps?: boolean;
  paranoid?: boolean;
  underscored?: boolean;
  freezeTableName?: boolean;
}

/** Where clause with operators */
export type WhereOptions = {
  [key: string]: unknown | { [op: symbol]: unknown };
} & {
  [Op.or]?: WhereOptions[];
  [Op.and]?: WhereOptions[];
};

/** Find options */
export interface FindOptions {
  where?: WhereOptions;
  attributes?: string[];
  include?: IncludeOptions[];
  order?: [string, "ASC" | "DESC"][];
  limit?: number;
  offset?: number;
  raw?: boolean;
}

/** Include options */
export interface IncludeOptions {
  model: Model;
  as?: string;
  attributes?: string[];
  where?: WhereOptions;
  required?: boolean;
  limit?: number;
}

/** Create options */
export interface CreateOptions {
  fields?: string[];
  returning?: boolean;
}

/** Update options */
export interface UpdateOptions {
  where: WhereOptions;
  fields?: string[];
  returning?: boolean;
}

/** Destroy options */
export interface DestroyOptions {
  where?: WhereOptions;
  force?: boolean;
  limit?: number;
}

/** Count options */
export interface CountOptions {
  where?: WhereOptions;
  distinct?: boolean;
}

// ============================================================================
// Model Instance
// ============================================================================

/** Model instance representing a single record */
export class ModelInstance {
  private model: Model;
  private _values: Record<string, unknown>;
  private _previousDataValues: Record<string, unknown>;
  private _isNewRecord: boolean;

  constructor(
    model: Model,
    values: Record<string, unknown> = {},
    isNewRecord = true
  ) {
    this.model = model;
    this._values = { ...values };
    this._previousDataValues = { ...values };
    this._isNewRecord = isNewRecord;

    // Create property accessors for each attribute
    for (const key of Object.keys(model.rawAttributes)) {
      Object.defineProperty(this, key, {
        get: () => this._values[key],
        set: (value) => {
          this._values[key] = value;
        },
        enumerable: true,
      });
    }
  }

  get id(): string {
    return this._values.id as string;
  }

  get isNewRecord(): boolean {
    return this._isNewRecord;
  }

  get dataValues(): Record<string, unknown> {
    return { ...this._values };
  }

  async save(): Promise<this> {
    if (this._isNewRecord) {
      const result = await this.model.create(this._values);
      Object.assign(this._values, result.dataValues);
      this._isNewRecord = false;
    } else {
      await this.model.update(this._values, {
        where: { id: this._values.id },
      });
    }
    this._previousDataValues = { ...this._values };
    return this;
  }

  async destroy(): Promise<void> {
    if (!this._isNewRecord && this._values.id) {
      await this.model.destroy({
        where: { id: this._values.id },
      });
    }
  }

  async reload(): Promise<this> {
    if (this._values.id) {
      const instance = await this.model.findByPk(this._values.id as string);
      if (instance) {
        Object.assign(this._values, instance.dataValues);
        this._previousDataValues = { ...this._values };
      }
    }
    return this;
  }

  toJSON(): Record<string, unknown> {
    return { ...this._values };
  }

  get(key: string): unknown {
    return this._values[key];
  }

  set(key: string, value: unknown): void {
    this._values[key] = value;
  }

  changed(): string[] | false {
    const changed: string[] = [];
    for (const key of Object.keys(this._values)) {
      if (this._values[key] !== this._previousDataValues[key]) {
        changed.push(key);
      }
    }
    return changed.length > 0 ? changed : false;
  }
}

// ============================================================================
// Model Class
// ============================================================================

/** Model class representing a database table */
export class Model {
  private client: OrmdbClient;
  private _tableName: string;
  private _attributes: ModelAttributes;
  private _options: ModelOptions;

  constructor(
    client: OrmdbClient,
    tableName: string,
    attributes: ModelAttributes,
    options: ModelOptions = {}
  ) {
    this.client = client;
    this._tableName = options.tableName ?? tableName;
    this._attributes = attributes;
    this._options = options;
  }

  get tableName(): string {
    return this._tableName;
  }

  get rawAttributes(): ModelAttributes {
    return this._attributes;
  }

  /**
   * Build a new instance without saving.
   */
  build(values: Record<string, unknown> = {}): ModelInstance {
    return new ModelInstance(this, values, true);
  }

  /**
   * Create a new record.
   */
  async create(
    values: Record<string, unknown>,
    _options?: CreateOptions
  ): Promise<ModelInstance> {
    const result = await this.client.insert(this._tableName, values);
    const id = result.insertedIds[0];

    if (id) {
      const created = await this.client.findById(this._tableName, id);
      return new ModelInstance(this, created ?? { id, ...values }, false);
    }

    return new ModelInstance(this, values, false);
  }

  /**
   * Bulk create records.
   */
  async bulkCreate(
    records: Record<string, unknown>[],
    _options?: CreateOptions
  ): Promise<ModelInstance[]> {
    const instances: ModelInstance[] = [];

    for (const record of records) {
      const instance = await this.create(record);
      instances.push(instance);
    }

    return instances;
  }

  /**
   * Find all records matching the options.
   */
  async findAll(options?: FindOptions): Promise<ModelInstance[]> {
    const filter = options?.where
      ? this.convertWhereToFilter(options.where)
      : undefined;
    const orderBy = options?.order
      ? this.convertOrderBy(options.order)
      : undefined;
    const includes = options?.include
      ? options.include.map((inc) => ({
          relation: inc.as ?? inc.model.tableName,
          fields: inc.attributes,
          filter: inc.where ? this.convertWhereToFilter(inc.where) : undefined,
          limit: inc.limit,
        }))
      : undefined;

    const result = await this.client.query(this._tableName, {
      filter,
      orderBy,
      limit: options?.limit,
      offset: options?.offset,
      fields: options?.attributes,
      includes,
    });

    return result.entities.map((row) => new ModelInstance(this, row, false));
  }

  /**
   * Find one record matching the options.
   */
  async findOne(options?: FindOptions): Promise<ModelInstance | null> {
    const results = await this.findAll({ ...options, limit: 1 });
    return results[0] ?? null;
  }

  /**
   * Find a record by primary key.
   */
  async findByPk(
    pk: string,
    options?: Omit<FindOptions, "where">
  ): Promise<ModelInstance | null> {
    const record = await this.client.findById(this._tableName, pk, {
      fields: options?.attributes,
      includes: options?.include?.map((inc) => ({
        relation: inc.as ?? inc.model.tableName,
        fields: inc.attributes,
      })),
    });

    if (record) {
      return new ModelInstance(this, record, false);
    }

    return null;
  }

  /**
   * Find or create a record.
   */
  async findOrCreate(options: {
    where: WhereOptions;
    defaults?: Record<string, unknown>;
  }): Promise<[ModelInstance, boolean]> {
    const existing = await this.findOne({ where: options.where });

    if (existing) {
      return [existing, false];
    }

    const instance = await this.create({
      ...options.defaults,
      ...this.whereToValues(options.where),
    });

    return [instance, true];
  }

  /**
   * Update records.
   */
  async update(
    values: Record<string, unknown>,
    options: UpdateOptions
  ): Promise<[number]> {
    const filter = this.convertWhereToFilter(options.where);
    const result = await this.client.updateMany(this._tableName, filter, values);
    return [result.affected];
  }

  /**
   * Delete records.
   */
  async destroy(options?: DestroyOptions): Promise<number> {
    if (!options?.where) {
      // Delete all - dangerous but Sequelize allows it
      const all = await this.findAll({ attributes: ["id"] });
      let count = 0;
      for (const instance of all) {
        await this.client.delete(this._tableName, instance.id);
        count++;
      }
      return count;
    }

    const filter = this.convertWhereToFilter(options.where);
    const result = await this.client.deleteMany(this._tableName, filter);
    return result.affected;
  }

  /**
   * Count records.
   */
  async count(options?: CountOptions): Promise<number> {
    const filter = options?.where
      ? this.convertWhereToFilter(options.where)
      : undefined;
    return this.client.count(this._tableName, filter);
  }

  /**
   * Find and count all records.
   */
  async findAndCountAll(
    options?: FindOptions
  ): Promise<{ rows: ModelInstance[]; count: number }> {
    const [rows, count] = await Promise.all([
      this.findAll(options),
      this.count({ where: options?.where }),
    ]);

    return { rows, count };
  }

  /**
   * Increment a field value.
   */
  async increment(
    field: string,
    options: { by?: number; where: WhereOptions }
  ): Promise<void> {
    const records = await this.findAll({
      where: options.where,
      attributes: ["id", field],
    });

    for (const record of records) {
      const currentValue = (record.get(field) as number) ?? 0;
      await this.client.update(this._tableName, record.id, {
        [field]: currentValue + (options.by ?? 1),
      });
    }
  }

  /**
   * Decrement a field value.
   */
  async decrement(
    field: string,
    options: { by?: number; where: WhereOptions }
  ): Promise<void> {
    return this.increment(field, {
      ...options,
      by: -(options.by ?? 1),
    });
  }

  /**
   * Convert Sequelize where clause to ORMDB filter.
   */
  private convertWhereToFilter(where: WhereOptions): FilterExpr {
    const conditions: FilterExpr[] = [];

    for (const [key, value] of Object.entries(where)) {
      // Handle Op.or
      if (key === Op.or.toString() || (typeof key === "symbol" && key === Op.or)) {
        const orValue = where[Op.or] as WhereOptions[];
        if (orValue) {
          conditions.push({
            or: orValue.map((w) => this.convertWhereToFilter(w)),
          });
        }
        continue;
      }

      // Handle Op.and
      if (key === Op.and.toString() || (typeof key === "symbol" && key === Op.and)) {
        const andValue = where[Op.and] as WhereOptions[];
        if (andValue) {
          conditions.push({
            and: andValue.map((w) => this.convertWhereToFilter(w)),
          });
        }
        continue;
      }

      if (value === null || value === undefined) {
        conditions.push({ field: key, op: "is_null" });
        continue;
      }

      if (typeof value === "object" && !Array.isArray(value)) {
        // Check for operator symbols
        const opEntries = Object.entries(value as Record<symbol, unknown>);
        for (const [opKey, opValue] of opEntries) {
          const filter = this.convertOperator(key, opKey, opValue);
          if (filter) {
            conditions.push(filter);
          }
        }

        // Also check symbol keys
        for (const sym of Object.getOwnPropertySymbols(value as object)) {
          const opValue = (value as Record<symbol, unknown>)[sym];
          const filter = this.convertSymbolOperator(key, sym, opValue);
          if (filter) {
            conditions.push(filter);
          }
        }
        continue;
      }

      // Simple equality
      conditions.push({ field: key, op: "eq", value });
    }

    if (conditions.length === 0) {
      return { and: [] };
    }
    if (conditions.length === 1) {
      return conditions[0];
    }
    return { and: conditions };
  }

  private convertOperator(
    field: string,
    op: string,
    value: unknown
  ): FilterExpr | null {
    // Handle string representations of operators
    const opMap: Record<string, FilterExpr["op"]> = {
      eq: "eq",
      ne: "ne",
      gt: "gt",
      gte: "ge",
      lt: "lt",
      lte: "le",
      like: "like",
      iLike: "ilike",
      in: "in",
      notIn: "not_in",
    };

    const ormdbOp = opMap[op];
    if (ormdbOp) {
      return { field, op: ormdbOp, value };
    }

    // Handle special operators
    if (op === "startsWith") {
      return { field, op: "like", value: `${value}%` };
    }
    if (op === "endsWith") {
      return { field, op: "like", value: `%${value}` };
    }
    if (op === "substring") {
      return { field, op: "like", value: `%${value}%` };
    }
    if (op === "is") {
      if (value === null) {
        return { field, op: "is_null" };
      }
    }
    if (op === "not") {
      if (value === null) {
        return { field, op: "is_not_null" };
      }
      return { field, op: "ne", value };
    }

    return null;
  }

  private convertSymbolOperator(
    field: string,
    sym: symbol,
    value: unknown
  ): FilterExpr | null {
    // Map symbols to operations
    if (sym === Op.eq) return { field, op: "eq", value };
    if (sym === Op.ne) return { field, op: "ne", value };
    if (sym === Op.gt) return { field, op: "gt", value };
    if (sym === Op.gte) return { field, op: "ge", value };
    if (sym === Op.lt) return { field, op: "lt", value };
    if (sym === Op.lte) return { field, op: "le", value };
    if (sym === Op.like) return { field, op: "like", value };
    if (sym === Op.iLike) return { field, op: "ilike", value };
    if (sym === Op.in) return { field, op: "in", value };
    if (sym === Op.notIn) return { field, op: "not_in", value };
    if (sym === Op.is) {
      if (value === null) return { field, op: "is_null" };
    }
    if (sym === Op.not) {
      if (value === null) return { field, op: "is_not_null" };
      return { field, op: "ne", value };
    }
    if (sym === Op.startsWith) {
      return { field, op: "like", value: `${value}%` };
    }
    if (sym === Op.endsWith) {
      return { field, op: "like", value: `%${value}` };
    }
    if (sym === Op.substring) {
      return { field, op: "like", value: `%${value}%` };
    }
    if (sym === Op.between && Array.isArray(value) && value.length === 2) {
      return {
        and: [
          { field, op: "ge", value: value[0] },
          { field, op: "le", value: value[1] },
        ],
      };
    }

    return null;
  }

  private convertOrderBy(order: [string, "ASC" | "DESC"][]): OrderSpec[] {
    return order.map(([field, direction]) => ({
      field,
      direction: direction.toLowerCase() as "asc" | "desc",
    }));
  }

  private whereToValues(where: WhereOptions): Record<string, unknown> {
    const values: Record<string, unknown> = {};

    for (const [key, value] of Object.entries(where)) {
      if (
        typeof key === "string" &&
        !key.startsWith("Symbol") &&
        value !== null &&
        typeof value !== "object"
      ) {
        values[key] = value;
      }
    }

    return values;
  }
}

// ============================================================================
// Sequelize Instance
// ============================================================================

/** Sequelize configuration options */
export interface OrmdbSequelizeOptions extends OrmdbConfig {
  logging?: boolean | ((sql: string) => void);
}

/**
 * Sequelize-compatible instance for ORMDB.
 */
export class OrmdbSequelize {
  private client: OrmdbClient;
  private models: Map<string, Model> = new Map();
  private options: OrmdbSequelizeOptions;

  constructor(config: OrmdbSequelizeOptions | string) {
    this.options =
      typeof config === "string" ? { baseUrl: config } : config;
    this.client = new OrmdbClient(this.options);
  }

  /**
   * Test the connection.
   */
  async authenticate(): Promise<void> {
    await this.client.health();
  }

  /**
   * Close the connection.
   */
  async close(): Promise<void> {
    // HTTP client doesn't need explicit close
  }

  /**
   * Define a model.
   */
  define(
    modelName: string,
    attributes: ModelAttributes,
    options?: ModelOptions
  ): Model {
    const model = new Model(this.client, modelName, attributes, options);
    this.models.set(modelName, model);
    return model;
  }

  /**
   * Get a defined model.
   */
  model(modelName: string): Model {
    const model = this.models.get(modelName);
    if (!model) {
      throw new Error(`Model ${modelName} is not defined`);
    }
    return model;
  }

  /**
   * Check if a model is defined.
   */
  isDefined(modelName: string): boolean {
    return this.models.has(modelName);
  }

  /**
   * Get all defined models.
   */
  get models(): Map<string, Model> {
    return this.models;
  }

  /**
   * Sync models with database (no-op for ORMDB - schema is managed separately).
   */
  async sync(_options?: { force?: boolean; alter?: boolean }): Promise<void> {
    // ORMDB manages schema through its own system
    // This is a no-op for compatibility
  }

  /**
   * Drop all tables (dangerous!).
   */
  async drop(_options?: { cascade?: boolean }): Promise<void> {
    // No-op - ORMDB manages schema separately
  }

  /**
   * Run a raw query (limited support).
   */
  async query(sql: string, _options?: { replacements?: Record<string, unknown> }): Promise<unknown[]> {
    // Very limited SQL support - mainly for compatibility
    const match = sql.match(/SELECT .+ FROM ["']?(\w+)["']?/i);
    if (match) {
      const result = await this.client.query(match[1]);
      return result.entities;
    }
    throw new Error("Raw queries are not fully supported");
  }

  /**
   * Begin a transaction (limited support).
   */
  async transaction<T>(
    callback: (t: Transaction) => Promise<T>
  ): Promise<T> {
    const t = new Transaction();
    try {
      const result = await callback(t);
      await t.commit();
      return result;
    } catch (error) {
      await t.rollback();
      throw error;
    }
  }
}

/**
 * Transaction placeholder (limited support).
 */
export class Transaction {
  async commit(): Promise<void> {
    // ORMDB auto-commits
  }

  async rollback(): Promise<void> {
    // ORMDB doesn't support rollback
  }
}

export default OrmdbSequelize;
