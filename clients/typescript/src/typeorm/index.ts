/**
 * TypeORM adapter for ORMDB.
 *
 * This module provides TypeORM-compatible data source and repository
 * implementations that work with ORMDB's HTTP gateway.
 *
 * @example
 * ```typescript
 * import { OrmdbDataSource } from "@ormdb/client/typeorm";
 * import { Entity, Column, PrimaryColumn } from "typeorm";
 *
 * @Entity()
 * class User {
 *   @PrimaryColumn()
 *   id: string;
 *
 *   @Column()
 *   name: string;
 *
 *   @Column()
 *   email: string;
 * }
 *
 * const dataSource = new OrmdbDataSource({
 *   url: "http://localhost:8080",
 *   entities: [User],
 * });
 *
 * await dataSource.initialize();
 *
 * const userRepo = dataSource.getRepository(User);
 * const users = await userRepo.find({ where: { name: "Alice" } });
 * ```
 */

import { OrmdbClient } from "../client";
import type { OrmdbConfig, FilterExpr, OrderSpec } from "../types";

// ============================================================================
// Types
// ============================================================================

/** Entity constructor type */
export type EntityClass<T = unknown> = new () => T;

/** Entity metadata stored via decorators */
export interface EntityMetadata {
  name: string;
  tableName: string;
  columns: ColumnMetadata[];
  relations: RelationMetadata[];
}

/** Column metadata */
export interface ColumnMetadata {
  propertyName: string;
  columnName: string;
  type: string;
  isPrimary: boolean;
  isNullable: boolean;
  isGenerated: boolean;
  default?: unknown;
}

/** Relation metadata */
export interface RelationMetadata {
  propertyName: string;
  relationType: "one-to-one" | "one-to-many" | "many-to-one" | "many-to-many";
  target: EntityClass | string;
  inverseSide?: string;
}

/** Find options */
export interface FindOptions<T = unknown> {
  where?: FindConditions<T> | FindConditions<T>[];
  order?: { [K in keyof T]?: "ASC" | "DESC" };
  skip?: number;
  take?: number;
  select?: (keyof T)[];
  relations?: string[];
}

/** Find conditions (where clause) */
export type FindConditions<T> = {
  [K in keyof T]?: T[K] | FindOperator<T[K]>;
};

/** Find operator */
export interface FindOperator<T> {
  type: string;
  value: T | T[];
  useParameter?: boolean;
  multipleParameters?: boolean;
}

/** Save options */
export interface SaveOptions {
  reload?: boolean;
}

/** Remove options */
export interface RemoveOptions {
  softRemove?: boolean;
}

// ============================================================================
// Decorators (for metadata collection)
// ============================================================================

// Global metadata storage
const entityMetadataMap = new Map<EntityClass, EntityMetadata>();

/**
 * Get or create entity metadata.
 */
function getOrCreateMetadata(target: EntityClass): EntityMetadata {
  if (!entityMetadataMap.has(target)) {
    entityMetadataMap.set(target, {
      name: target.name,
      tableName: target.name,
      columns: [],
      relations: [],
    });
  }
  return entityMetadataMap.get(target)!;
}

/**
 * Entity decorator.
 */
export function Entity(options?: { name?: string }): ClassDecorator {
  return function (target: Function) {
    const metadata = getOrCreateMetadata(target as EntityClass);
    if (options?.name) {
      metadata.tableName = options.name;
    }
  };
}

/**
 * Column decorator.
 */
export function Column(options?: {
  name?: string;
  type?: string;
  nullable?: boolean;
  default?: unknown;
}): PropertyDecorator {
  return function (target: Object, propertyKey: string | symbol) {
    const metadata = getOrCreateMetadata(target.constructor as EntityClass);
    metadata.columns.push({
      propertyName: String(propertyKey),
      columnName: options?.name ?? String(propertyKey),
      type: options?.type ?? "string",
      isPrimary: false,
      isNullable: options?.nullable ?? false,
      isGenerated: false,
      default: options?.default,
    });
  };
}

/**
 * Primary column decorator.
 */
export function PrimaryColumn(options?: {
  name?: string;
  type?: string;
}): PropertyDecorator {
  return function (target: Object, propertyKey: string | symbol) {
    const metadata = getOrCreateMetadata(target.constructor as EntityClass);
    metadata.columns.push({
      propertyName: String(propertyKey),
      columnName: options?.name ?? String(propertyKey),
      type: options?.type ?? "uuid",
      isPrimary: true,
      isNullable: false,
      isGenerated: false,
    });
  };
}

/**
 * Primary generated column decorator.
 */
export function PrimaryGeneratedColumn(type?: "uuid" | "increment"): PropertyDecorator {
  return function (target: Object, propertyKey: string | symbol) {
    const metadata = getOrCreateMetadata(target.constructor as EntityClass);
    metadata.columns.push({
      propertyName: String(propertyKey),
      columnName: String(propertyKey),
      type: type ?? "uuid",
      isPrimary: true,
      isNullable: false,
      isGenerated: true,
    });
  };
}

/**
 * One-to-one relation decorator.
 */
export function OneToOne<T>(
  typeFn: () => EntityClass<T>,
  inverseSide?: string | ((obj: T) => unknown)
): PropertyDecorator {
  return function (target: Object, propertyKey: string | symbol) {
    const metadata = getOrCreateMetadata(target.constructor as EntityClass);
    metadata.relations.push({
      propertyName: String(propertyKey),
      relationType: "one-to-one",
      target: typeFn(),
      inverseSide: typeof inverseSide === "string" ? inverseSide : undefined,
    });
  };
}

/**
 * One-to-many relation decorator.
 */
export function OneToMany<T>(
  typeFn: () => EntityClass<T>,
  inverseSide: string | ((obj: T) => unknown)
): PropertyDecorator {
  return function (target: Object, propertyKey: string | symbol) {
    const metadata = getOrCreateMetadata(target.constructor as EntityClass);
    metadata.relations.push({
      propertyName: String(propertyKey),
      relationType: "one-to-many",
      target: typeFn(),
      inverseSide: typeof inverseSide === "string" ? inverseSide : undefined,
    });
  };
}

/**
 * Many-to-one relation decorator.
 */
export function ManyToOne<T>(
  typeFn: () => EntityClass<T>,
  inverseSide?: string | ((obj: T) => unknown)
): PropertyDecorator {
  return function (target: Object, propertyKey: string | symbol) {
    const metadata = getOrCreateMetadata(target.constructor as EntityClass);
    metadata.relations.push({
      propertyName: String(propertyKey),
      relationType: "many-to-one",
      target: typeFn(),
      inverseSide: typeof inverseSide === "string" ? inverseSide : undefined,
    });
  };
}

/**
 * Many-to-many relation decorator.
 */
export function ManyToMany<T>(
  typeFn: () => EntityClass<T>,
  inverseSide?: string | ((obj: T) => unknown)
): PropertyDecorator {
  return function (target: Object, propertyKey: string | symbol) {
    const metadata = getOrCreateMetadata(target.constructor as EntityClass);
    metadata.relations.push({
      propertyName: String(propertyKey),
      relationType: "many-to-many",
      target: typeFn(),
      inverseSide: typeof inverseSide === "string" ? inverseSide : undefined,
    });
  };
}

/**
 * Join column decorator.
 */
export function JoinColumn(_options?: {
  name?: string;
  referencedColumnName?: string;
}): PropertyDecorator {
  return function (_target: Object, _propertyKey: string | symbol) {
    // JoinColumn metadata is used for schema generation
    // ORMDB handles joins automatically based on relations
  };
}

// ============================================================================
// Find Operators
// ============================================================================

export function Equal<T>(value: T): FindOperator<T> {
  return { type: "equal", value };
}

export function Not<T>(value: T | FindOperator<T>): FindOperator<T> {
  const actualValue = isFindOperator(value) ? value.value : value;
  return { type: "not", value: actualValue as T };
}

export function LessThan<T>(value: T): FindOperator<T> {
  return { type: "lessThan", value };
}

export function LessThanOrEqual<T>(value: T): FindOperator<T> {
  return { type: "lessThanOrEqual", value };
}

export function MoreThan<T>(value: T): FindOperator<T> {
  return { type: "moreThan", value };
}

export function MoreThanOrEqual<T>(value: T): FindOperator<T> {
  return { type: "moreThanOrEqual", value };
}

export function Like(value: string): FindOperator<string> {
  return { type: "like", value };
}

export function ILike(value: string): FindOperator<string> {
  return { type: "ilike", value };
}

export function In<T>(values: T[]): FindOperator<T> {
  return { type: "in", value: values[0], multipleParameters: true };
}

export function IsNull(): FindOperator<null> {
  return { type: "isNull", value: null };
}

function isFindOperator<T>(value: unknown): value is FindOperator<T> {
  return value !== null && typeof value === "object" && "type" in value;
}

// ============================================================================
// Repository
// ============================================================================

/**
 * TypeORM-style repository for ORMDB.
 */
export class Repository<T extends object> {
  private client: OrmdbClient;
  private entityClass: EntityClass<T>;
  private metadata: EntityMetadata;

  constructor(client: OrmdbClient, entityClass: EntityClass<T>) {
    this.client = client;
    this.entityClass = entityClass;
    this.metadata = getOrCreateMetadata(entityClass);
  }

  /**
   * Get the entity's table name.
   */
  get tableName(): string {
    return this.metadata.tableName;
  }

  /**
   * Find all entities matching options.
   */
  async find(options?: FindOptions<T>): Promise<T[]> {
    const filter = options?.where
      ? this.convertWhereToFilter(options.where)
      : undefined;

    const orderBy = options?.order
      ? this.convertOrderBy(options.order)
      : undefined;

    const includes = options?.relations?.map((r) => ({ relation: r }));

    const result = await this.client.query(this.tableName, {
      filter,
      orderBy,
      limit: options?.take,
      offset: options?.skip,
      fields: options?.select as string[],
      includes,
    });

    return result.entities.map((row) => this.mapToEntity(row));
  }

  /**
   * Find one entity matching options.
   */
  async findOne(options: FindOptions<T>): Promise<T | null> {
    const results = await this.find({ ...options, take: 1 });
    return results[0] ?? null;
  }

  /**
   * Find one entity by ID.
   */
  async findOneBy(where: FindConditions<T>): Promise<T | null> {
    return this.findOne({ where });
  }

  /**
   * Find one entity or throw.
   */
  async findOneOrFail(options: FindOptions<T>): Promise<T> {
    const result = await this.findOne(options);
    if (!result) {
      throw new Error(`Entity not found: ${this.tableName}`);
    }
    return result;
  }

  /**
   * Find one entity by ID or throw.
   */
  async findOneByOrFail(where: FindConditions<T>): Promise<T> {
    return this.findOneOrFail({ where });
  }

  /**
   * Find entities by IDs.
   */
  async findByIds(ids: string[]): Promise<T[]> {
    const filter: FilterExpr = { field: "id", op: "in", value: ids };
    const result = await this.client.query(this.tableName, { filter });
    return result.entities.map((row) => this.mapToEntity(row));
  }

  /**
   * Count entities.
   */
  async count(options?: FindOptions<T>): Promise<number> {
    const filter = options?.where
      ? this.convertWhereToFilter(options.where)
      : undefined;
    return this.client.count(this.tableName, filter);
  }

  /**
   * Save an entity (insert or update).
   */
  async save(entity: Partial<T>, _options?: SaveOptions): Promise<T> {
    const data = this.entityToData(entity);
    const id = (entity as Record<string, unknown>).id as string | undefined;

    if (id) {
      // Update existing
      await this.client.update(this.tableName, id, data);
      const updated = await this.client.findById(this.tableName, id);
      return this.mapToEntity(updated ?? data);
    } else {
      // Insert new
      const result = await this.client.insert(this.tableName, data);
      const insertedId = result.insertedIds[0];
      if (insertedId) {
        const inserted = await this.client.findById(this.tableName, insertedId);
        return this.mapToEntity(inserted ?? { id: insertedId, ...data });
      }
      return this.mapToEntity(data);
    }
  }

  /**
   * Save multiple entities.
   */
  async saveMany(entities: Partial<T>[]): Promise<T[]> {
    const results: T[] = [];
    for (const entity of entities) {
      results.push(await this.save(entity));
    }
    return results;
  }

  /**
   * Insert a new entity.
   */
  async insert(entity: Partial<T>): Promise<{ identifiers: { id: string }[] }> {
    const data = this.entityToData(entity);
    const result = await this.client.insert(this.tableName, data);
    return {
      identifiers: result.insertedIds.map((id) => ({ id })),
    };
  }

  /**
   * Update entities.
   */
  async update(
    criteria: string | FindConditions<T>,
    partialEntity: Partial<T>
  ): Promise<{ affected: number }> {
    const data = this.entityToData(partialEntity);

    if (typeof criteria === "string") {
      const result = await this.client.update(this.tableName, criteria, data);
      return { affected: result.affected };
    }

    const filter = this.convertWhereToFilter(criteria);
    const result = await this.client.updateMany(this.tableName, filter, data);
    return { affected: result.affected };
  }

  /**
   * Delete an entity.
   */
  async delete(
    criteria: string | FindConditions<T>
  ): Promise<{ affected: number }> {
    if (typeof criteria === "string") {
      const result = await this.client.delete(this.tableName, criteria);
      return { affected: result.affected };
    }

    const filter = this.convertWhereToFilter(criteria);
    const result = await this.client.deleteMany(this.tableName, filter);
    return { affected: result.affected };
  }

  /**
   * Remove an entity instance.
   */
  async remove(entity: T, _options?: RemoveOptions): Promise<T> {
    const id = (entity as Record<string, unknown>).id as string;
    if (id) {
      await this.client.delete(this.tableName, id);
    }
    return entity;
  }

  /**
   * Remove multiple entity instances.
   */
  async removeMany(entities: T[]): Promise<T[]> {
    for (const entity of entities) {
      await this.remove(entity);
    }
    return entities;
  }

  /**
   * Create a query builder.
   */
  createQueryBuilder(alias?: string): QueryBuilder<T> {
    return new QueryBuilder(this.client, this.tableName, alias);
  }

  /**
   * Convert where conditions to ORMDB filter.
   */
  private convertWhereToFilter(
    where: FindConditions<T> | FindConditions<T>[]
  ): FilterExpr {
    if (Array.isArray(where)) {
      return { or: where.map((w) => this.convertSingleWhere(w)) };
    }
    return this.convertSingleWhere(where);
  }

  private convertSingleWhere(where: FindConditions<T>): FilterExpr {
    const conditions: FilterExpr[] = [];

    for (const [key, value] of Object.entries(where)) {
      if (value === undefined) continue;

      if (isFindOperator(value)) {
        conditions.push(this.convertOperator(key, value));
      } else if (value === null) {
        conditions.push({ field: key, op: "is_null" });
      } else {
        conditions.push({ field: key, op: "eq", value });
      }
    }

    if (conditions.length === 0) {
      return { and: [] };
    }
    if (conditions.length === 1) {
      return conditions[0];
    }
    return { and: conditions };
  }

  private convertOperator(field: string, op: FindOperator<unknown>): FilterExpr {
    switch (op.type) {
      case "equal":
        return { field, op: "eq", value: op.value };
      case "not":
        return { field, op: "ne", value: op.value };
      case "lessThan":
        return { field, op: "lt", value: op.value };
      case "lessThanOrEqual":
        return { field, op: "le", value: op.value };
      case "moreThan":
        return { field, op: "gt", value: op.value };
      case "moreThanOrEqual":
        return { field, op: "ge", value: op.value };
      case "like":
        return { field, op: "like", value: op.value };
      case "ilike":
        return { field, op: "ilike", value: op.value };
      case "in":
        return { field, op: "in", value: op.value };
      case "isNull":
        return { field, op: "is_null" };
      default:
        return { field, op: "eq", value: op.value };
    }
  }

  private convertOrderBy(
    order: { [K in keyof T]?: "ASC" | "DESC" }
  ): OrderSpec[] {
    return Object.entries(order).map(([field, direction]) => ({
      field,
      direction: (direction as string).toLowerCase() as "asc" | "desc",
    }));
  }

  private entityToData(entity: Partial<T>): Record<string, unknown> {
    const data: Record<string, unknown> = {};

    for (const [key, value] of Object.entries(entity as object)) {
      if (value !== undefined && key !== "id") {
        data[key] = value;
      }
    }

    return data;
  }

  private mapToEntity(row: Record<string, unknown>): T {
    const entity = new this.entityClass();
    Object.assign(entity, row);
    return entity;
  }
}

// ============================================================================
// Query Builder
// ============================================================================

/**
 * Simple query builder for TypeORM compatibility.
 */
export class QueryBuilder<T> {
  private client: OrmdbClient;
  private entityName: string;
  private alias: string;
  private whereConditions: FilterExpr[] = [];
  private orderBySpecs: OrderSpec[] = [];
  private limitValue?: number;
  private offsetValue?: number;
  private selectedFields?: string[];

  constructor(client: OrmdbClient, entityName: string, alias?: string) {
    this.client = client;
    this.entityName = entityName;
    this.alias = alias ?? entityName.toLowerCase();
  }

  select(fields: string[]): this {
    this.selectedFields = fields.map((f) =>
      f.startsWith(`${this.alias}.`) ? f.slice(this.alias.length + 1) : f
    );
    return this;
  }

  where(condition: string, parameters?: Record<string, unknown>): this {
    // Parse simple conditions like "user.name = :name"
    const filter = this.parseCondition(condition, parameters);
    if (filter) {
      this.whereConditions.push(filter);
    }
    return this;
  }

  andWhere(condition: string, parameters?: Record<string, unknown>): this {
    return this.where(condition, parameters);
  }

  orWhere(condition: string, parameters?: Record<string, unknown>): this {
    const filter = this.parseCondition(condition, parameters);
    if (filter && this.whereConditions.length > 0) {
      const existing = this.whereConditions.pop()!;
      this.whereConditions.push({ or: [existing, filter] });
    } else if (filter) {
      this.whereConditions.push(filter);
    }
    return this;
  }

  orderBy(field: string, order: "ASC" | "DESC" = "ASC"): this {
    const fieldName = field.startsWith(`${this.alias}.`)
      ? field.slice(this.alias.length + 1)
      : field;
    this.orderBySpecs.push({
      field: fieldName,
      direction: order.toLowerCase() as "asc" | "desc",
    });
    return this;
  }

  addOrderBy(field: string, order: "ASC" | "DESC" = "ASC"): this {
    return this.orderBy(field, order);
  }

  skip(count: number): this {
    this.offsetValue = count;
    return this;
  }

  take(count: number): this {
    this.limitValue = count;
    return this;
  }

  async getMany(): Promise<T[]> {
    const filter =
      this.whereConditions.length > 0
        ? this.whereConditions.length === 1
          ? this.whereConditions[0]
          : { and: this.whereConditions }
        : undefined;

    const result = await this.client.query(this.entityName, {
      filter,
      orderBy: this.orderBySpecs.length > 0 ? this.orderBySpecs : undefined,
      limit: this.limitValue,
      offset: this.offsetValue,
      fields: this.selectedFields,
    });

    return result.entities as T[];
  }

  async getOne(): Promise<T | null> {
    const results = await this.take(1).getMany();
    return results[0] ?? null;
  }

  async getCount(): Promise<number> {
    const filter =
      this.whereConditions.length > 0
        ? this.whereConditions.length === 1
          ? this.whereConditions[0]
          : { and: this.whereConditions }
        : undefined;

    return this.client.count(this.entityName, filter);
  }

  private parseCondition(
    condition: string,
    parameters?: Record<string, unknown>
  ): FilterExpr | null {
    // Parse patterns like "user.name = :name" or "user.age > :age"
    const match = condition.match(
      /(\w+)\.(\w+)\s*(=|!=|<>|<|>|<=|>=|LIKE|ILIKE|IN)\s*:(\w+)/i
    );

    if (!match || !parameters) return null;

    const [, , field, operator, paramName] = match;
    const value = parameters[paramName];

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
      op: opMap[operator.toUpperCase()] ?? "eq",
      value,
    };
  }
}

// ============================================================================
// Data Source
// ============================================================================

/** Data source options */
export interface OrmdbDataSourceOptions {
  url: string;
  timeout?: number;
  entities?: EntityClass[];
  synchronize?: boolean;
  logging?: boolean;
}

/**
 * TypeORM-compatible data source for ORMDB.
 */
export class OrmdbDataSource {
  private client: OrmdbClient;
  private options: OrmdbDataSourceOptions;
  private repositories = new Map<EntityClass, Repository<object>>();
  private _isInitialized = false;

  constructor(options: OrmdbDataSourceOptions) {
    this.options = options;
    this.client = new OrmdbClient({
      baseUrl: options.url,
      timeout: options.timeout,
    });
  }

  /**
   * Check if the data source is initialized.
   */
  get isInitialized(): boolean {
    return this._isInitialized;
  }

  /**
   * Initialize the data source.
   */
  async initialize(): Promise<this> {
    // Verify connection
    await this.client.health();

    // Register entities
    if (this.options.entities) {
      for (const entity of this.options.entities) {
        getOrCreateMetadata(entity);
      }
    }

    this._isInitialized = true;
    return this;
  }

  /**
   * Destroy the data source.
   */
  async destroy(): Promise<void> {
    this._isInitialized = false;
    this.repositories.clear();
  }

  /**
   * Get a repository for an entity.
   */
  getRepository<T extends object>(entityClass: EntityClass<T>): Repository<T> {
    if (!this.repositories.has(entityClass)) {
      this.repositories.set(
        entityClass,
        new Repository(this.client, entityClass)
      );
    }
    return this.repositories.get(entityClass) as Repository<T>;
  }

  /**
   * Create a query builder.
   */
  createQueryBuilder<T>(
    entityClass: EntityClass<T>,
    alias?: string
  ): QueryBuilder<T> {
    const metadata = getOrCreateMetadata(entityClass);
    return new QueryBuilder(this.client, metadata.tableName, alias);
  }

  /**
   * Run a transaction (limited support - executes sequentially).
   */
  async transaction<T>(
    runInTransaction: (entityManager: EntityManager) => Promise<T>
  ): Promise<T> {
    const manager = new EntityManager(this);
    return runInTransaction(manager);
  }
}

/**
 * Entity manager for transaction-like operations.
 */
export class EntityManager {
  constructor(private dataSource: OrmdbDataSource) {}

  getRepository<T extends object>(entityClass: EntityClass<T>): Repository<T> {
    return this.dataSource.getRepository(entityClass);
  }

  async save<T extends object>(entity: T): Promise<T> {
    const entityClass = entity.constructor as EntityClass<T>;
    const repo = this.getRepository(entityClass);
    return repo.save(entity);
  }

  async remove<T extends object>(entity: T): Promise<T> {
    const entityClass = entity.constructor as EntityClass<T>;
    const repo = this.getRepository(entityClass);
    return repo.remove(entity);
  }
}

export default OrmdbDataSource;
