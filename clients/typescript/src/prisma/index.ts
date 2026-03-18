/**
 * Prisma-compatible adapter for ORMDB.
 *
 * This module provides a Prisma-like API that translates operations
 * to ORMDB's HTTP gateway.
 *
 * @example
 * ```typescript
 * import { createPrismaClient } from "@ormdb/client/prisma";
 *
 * const prisma = createPrismaClient("http://localhost:8080");
 *
 * // Prisma-like operations
 * const users = await prisma.user.findMany({
 *   where: { status: "active" },
 *   orderBy: { name: "asc" },
 *   take: 10,
 * });
 *
 * const user = await prisma.user.create({
 *   data: { name: "Alice", email: "alice@example.com" },
 * });
 * ```
 */

import { OrmdbClient } from "../client";
import type {
  OrmdbConfig,
  FilterExpr,
  OrderSpec,
  RelationInclude,
  SchemaEntity,
} from "../types";

/** Prisma where clause */
export type WhereClause<T = Record<string, unknown>> = {
  [K in keyof T]?: T[K] | WhereOperator<T[K]>;
} & {
  AND?: WhereClause<T>[];
  OR?: WhereClause<T>[];
  NOT?: WhereClause<T>;
};

/** Prisma where operators */
export interface WhereOperator<T> {
  equals?: T;
  not?: T | WhereOperator<T>;
  in?: T[];
  notIn?: T[];
  lt?: T;
  lte?: T;
  gt?: T;
  gte?: T;
  contains?: string;
  startsWith?: string;
  endsWith?: string;
  mode?: "default" | "insensitive";
}

/** Prisma order by clause */
export type OrderByClause<T = Record<string, unknown>> = {
  [K in keyof T]?: "asc" | "desc";
};

/** Prisma select clause */
export type SelectClause<T = Record<string, unknown>> = {
  [K in keyof T]?: boolean;
};

/** Prisma include clause */
export type IncludeClause = Record<
  string,
  | boolean
  | {
      where?: WhereClause;
      orderBy?: OrderByClause;
      take?: number;
      skip?: number;
      include?: IncludeClause;
    }
>;

/** Find many options */
export interface FindManyOptions<T = Record<string, unknown>> {
  where?: WhereClause<T>;
  orderBy?: OrderByClause<T> | OrderByClause<T>[];
  take?: number;
  skip?: number;
  select?: SelectClause<T>;
  include?: IncludeClause;
  cursor?: { id: string };
  distinct?: (keyof T)[];
}

/** Find unique options */
export interface FindUniqueOptions<T = Record<string, unknown>> {
  where: { id: string } | Record<string, unknown>;
  select?: SelectClause<T>;
  include?: IncludeClause;
}

/** Find first options */
export interface FindFirstOptions<T = Record<string, unknown>>
  extends FindManyOptions<T> {}

/** Create options */
export interface CreateOptions<T = Record<string, unknown>> {
  data: Partial<T>;
  select?: SelectClause<T>;
  include?: IncludeClause;
}

/** Create many options */
export interface CreateManyOptions<T = Record<string, unknown>> {
  data: Partial<T>[];
  skipDuplicates?: boolean;
}

/** Update options */
export interface UpdateOptions<T = Record<string, unknown>> {
  where: { id: string } | Record<string, unknown>;
  data: Partial<T>;
  select?: SelectClause<T>;
  include?: IncludeClause;
}

/** Update many options */
export interface UpdateManyOptions<T = Record<string, unknown>> {
  where: WhereClause<T>;
  data: Partial<T>;
}

/** Upsert options */
export interface UpsertOptions<T = Record<string, unknown>> {
  where: { id: string } | Record<string, unknown>;
  create: Partial<T>;
  update: Partial<T>;
  select?: SelectClause<T>;
  include?: IncludeClause;
}

/** Delete options */
export interface DeleteOptions<T = Record<string, unknown>> {
  where: { id: string } | Record<string, unknown>;
  select?: SelectClause<T>;
  include?: IncludeClause;
}

/** Delete many options */
export interface DeleteManyOptions<T = Record<string, unknown>> {
  where: WhereClause<T>;
}

/** Count options */
export interface CountOptions<T = Record<string, unknown>> {
  where?: WhereClause<T>;
  cursor?: { id: string };
  take?: number;
  skip?: number;
}

/** Aggregate options */
export interface AggregateOptions<T = Record<string, unknown>> {
  where?: WhereClause<T>;
  _count?: boolean | { [K in keyof T]?: boolean };
  _avg?: { [K in keyof T]?: boolean };
  _sum?: { [K in keyof T]?: boolean };
  _min?: { [K in keyof T]?: boolean };
  _max?: { [K in keyof T]?: boolean };
}

/** Model delegate (Prisma-style model accessor) */
export interface ModelDelegate<T = Record<string, unknown>> {
  findMany(options?: FindManyOptions<T>): Promise<T[]>;
  findFirst(options?: FindFirstOptions<T>): Promise<T | null>;
  findUnique(options: FindUniqueOptions<T>): Promise<T | null>;
  findUniqueOrThrow(options: FindUniqueOptions<T>): Promise<T>;
  findFirstOrThrow(options?: FindFirstOptions<T>): Promise<T>;
  create(options: CreateOptions<T>): Promise<T>;
  createMany(options: CreateManyOptions<T>): Promise<{ count: number }>;
  update(options: UpdateOptions<T>): Promise<T>;
  updateMany(options: UpdateManyOptions<T>): Promise<{ count: number }>;
  upsert(options: UpsertOptions<T>): Promise<T>;
  delete(options: DeleteOptions<T>): Promise<T>;
  deleteMany(options?: DeleteManyOptions<T>): Promise<{ count: number }>;
  count(options?: CountOptions<T>): Promise<number>;
  aggregate(options: AggregateOptions<T>): Promise<Record<string, unknown>>;
}

/**
 * Prisma-compatible client for ORMDB.
 */
export class PrismaClient {
  private client: OrmdbClient;
  private models: Map<string, ModelDelegate> = new Map();
  private schema: SchemaEntity[] | null = null;

  constructor(config: OrmdbConfig | string) {
    this.client = new OrmdbClient(config);
  }

  /**
   * Connect to the database.
   */
  async $connect(): Promise<void> {
    await this.client.health();
    const schema = await this.client.getSchema();
    this.schema = schema.entities;
  }

  /**
   * Disconnect from the database.
   */
  async $disconnect(): Promise<void> {
    // HTTP client doesn't need explicit disconnect
  }

  /**
   * Execute a raw query.
   */
  async $queryRaw<T = unknown>(
    entity: string,
    filter?: FilterExpr
  ): Promise<T[]> {
    const result = await this.client.query(entity, { filter });
    return result.entities as T[];
  }

  /**
   * Execute a raw mutation.
   */
  async $executeRaw(
    operation: "insert" | "update" | "delete",
    entity: string,
    data: Record<string, unknown>,
    id?: string
  ): Promise<number> {
    let result;
    switch (operation) {
      case "insert":
        result = await this.client.insert(entity, data);
        break;
      case "update":
        if (!id) throw new Error("ID required for update");
        result = await this.client.update(entity, id, data);
        break;
      case "delete":
        if (!id) throw new Error("ID required for delete");
        result = await this.client.delete(entity, id);
        break;
    }
    return result.affected;
  }

  /**
   * Get a model delegate by name.
   */
  private getModelDelegate(entityName: string): ModelDelegate {
    const lowerName = entityName.toLowerCase();

    if (!this.models.has(lowerName)) {
      this.models.set(lowerName, this.createModelDelegate(entityName));
    }

    return this.models.get(lowerName)!;
  }

  /**
   * Create a model delegate for an entity.
   */
  private createModelDelegate(entityName: string): ModelDelegate {
    const client = this.client;
    const self = this;

    return {
      async findMany(options = {}) {
        const filter = options.where
          ? self.convertWhereClause(options.where)
          : undefined;
        const orderBy = options.orderBy
          ? self.convertOrderBy(options.orderBy)
          : undefined;
        const fields = options.select
          ? self.convertSelect(options.select)
          : undefined;
        const includes = options.include
          ? self.convertInclude(options.include)
          : undefined;

        const result = await client.query(entityName, {
          filter,
          orderBy,
          limit: options.take,
          offset: options.skip,
          fields,
          includes,
        });

        return result.entities;
      },

      async findFirst(options = {}) {
        const results = await this.findMany({ ...options, take: 1 });
        return results[0] ?? null;
      },

      async findUnique(options) {
        const where = options.where;
        const id = "id" in where ? where.id : null;

        if (id) {
          return client.findById(entityName, id as string, {
            fields: options.select
              ? self.convertSelect(options.select)
              : undefined,
            includes: options.include
              ? self.convertInclude(options.include)
              : undefined,
          });
        }

        // For other unique fields, use findFirst
        return this.findFirst({
          where: where as WhereClause,
          select: options.select,
          include: options.include,
        });
      },

      async findUniqueOrThrow(options) {
        const result = await this.findUnique(options);
        if (!result) {
          throw new Error(`Record not found in ${entityName}`);
        }
        return result;
      },

      async findFirstOrThrow(options = {}) {
        const result = await this.findFirst(options);
        if (!result) {
          throw new Error(`Record not found in ${entityName}`);
        }
        return result;
      },

      async create(options) {
        const result = await client.insert(
          entityName,
          options.data as Record<string, unknown>
        );
        const id = result.insertedIds[0];

        // Fetch the created record
        if (id) {
          const record = await client.findById(entityName, id, {
            fields: options.select
              ? self.convertSelect(options.select)
              : undefined,
            includes: options.include
              ? self.convertInclude(options.include)
              : undefined,
          });
          return record ?? { id, ...options.data };
        }

        return { ...options.data } as Record<string, unknown>;
      },

      async createMany(options) {
        const result = await client.insertMany(
          entityName,
          options.data as Record<string, unknown>[]
        );
        return { count: result.affected };
      },

      async update(options) {
        const where = options.where;
        const id = "id" in where ? (where.id as string) : null;

        if (!id) {
          // Find by unique field first
          const record = await this.findFirst({
            where: where as WhereClause,
          });
          if (!record) {
            throw new Error(`Record not found in ${entityName}`);
          }
          return this.update({
            ...options,
            where: { id: (record as Record<string, unknown>).id as string },
          });
        }

        await client.update(
          entityName,
          id,
          options.data as Record<string, unknown>
        );

        // Fetch the updated record
        const record = await client.findById(entityName, id, {
          fields: options.select
            ? self.convertSelect(options.select)
            : undefined,
          includes: options.include
            ? self.convertInclude(options.include)
            : undefined,
        });

        return record ?? { id, ...options.data };
      },

      async updateMany(options) {
        const filter = self.convertWhereClause(options.where);
        const result = await client.updateMany(
          entityName,
          filter,
          options.data as Record<string, unknown>
        );
        return { count: result.affected };
      },

      async upsert(options) {
        const where = options.where;
        const id = "id" in where ? (where.id as string) : null;

        if (id) {
          const existing = await client.findById(entityName, id);
          if (existing) {
            return this.update({
              where: { id },
              data: options.update,
              select: options.select,
              include: options.include,
            });
          }
        }

        return this.create({
          data: options.create,
          select: options.select,
          include: options.include,
        });
      },

      async delete(options) {
        const where = options.where;
        const id = "id" in where ? (where.id as string) : null;

        if (!id) {
          const record = await this.findFirst({
            where: where as WhereClause,
          });
          if (!record) {
            throw new Error(`Record not found in ${entityName}`);
          }
          return this.delete({
            ...options,
            where: { id: (record as Record<string, unknown>).id as string },
          });
        }

        // Fetch before deleting
        const record = await client.findById(entityName, id, {
          fields: options.select
            ? self.convertSelect(options.select)
            : undefined,
          includes: options.include
            ? self.convertInclude(options.include)
            : undefined,
        });

        await client.delete(entityName, id);

        return record ?? { id };
      },

      async deleteMany(options = {}) {
        if (!options.where) {
          // Delete all - dangerous but Prisma allows it
          const all = await this.findMany({ select: { id: true } });
          let count = 0;
          for (const record of all) {
            await client.delete(
              entityName,
              (record as Record<string, unknown>).id as string
            );
            count++;
          }
          return { count };
        }

        const filter = self.convertWhereClause(options.where);
        const result = await client.deleteMany(entityName, filter);
        return { count: result.affected };
      },

      async count(options = {}) {
        const filter = options.where
          ? self.convertWhereClause(options.where)
          : undefined;
        return client.count(entityName, filter);
      },

      async aggregate(options) {
        // Basic aggregate implementation
        const filter = options.where
          ? self.convertWhereClause(options.where)
          : undefined;

        const result: Record<string, unknown> = {};

        if (options._count) {
          result._count = await client.count(entityName, filter);
        }

        // Other aggregations would require server-side support
        // For now, return what we can

        return result;
      },
    };
  }

  /**
   * Convert Prisma where clause to ORMDB filter.
   */
  private convertWhereClause(where: WhereClause): FilterExpr {
    const conditions: FilterExpr[] = [];

    for (const [key, value] of Object.entries(where)) {
      if (key === "AND" && Array.isArray(value)) {
        conditions.push({
          and: value.map((w) => this.convertWhereClause(w)),
        });
        continue;
      }

      if (key === "OR" && Array.isArray(value)) {
        conditions.push({
          or: value.map((w) => this.convertWhereClause(w)),
        });
        continue;
      }

      if (key === "NOT" && value) {
        conditions.push({
          not: this.convertWhereClause(value as WhereClause),
        });
        continue;
      }

      if (value === null || value === undefined) {
        conditions.push({ field: key, op: "is_null" });
        continue;
      }

      if (typeof value === "object" && !Array.isArray(value)) {
        // Operator object
        const ops = value as WhereOperator<unknown>;

        if ("equals" in ops) {
          conditions.push({ field: key, op: "eq", value: ops.equals });
        }
        if ("not" in ops) {
          if (typeof ops.not === "object" && ops.not !== null) {
            conditions.push({
              not: this.convertWhereClause({ [key]: ops.not }),
            });
          } else {
            conditions.push({ field: key, op: "ne", value: ops.not });
          }
        }
        if ("in" in ops) {
          conditions.push({ field: key, op: "in", value: ops.in });
        }
        if ("notIn" in ops) {
          conditions.push({ field: key, op: "not_in", value: ops.notIn });
        }
        if ("lt" in ops) {
          conditions.push({ field: key, op: "lt", value: ops.lt });
        }
        if ("lte" in ops) {
          conditions.push({ field: key, op: "le", value: ops.lte });
        }
        if ("gt" in ops) {
          conditions.push({ field: key, op: "gt", value: ops.gt });
        }
        if ("gte" in ops) {
          conditions.push({ field: key, op: "ge", value: ops.gte });
        }
        if ("contains" in ops) {
          const pattern = `%${ops.contains}%`;
          conditions.push({
            field: key,
            op: ops.mode === "insensitive" ? "ilike" : "like",
            value: pattern,
          });
        }
        if ("startsWith" in ops) {
          const pattern = `${ops.startsWith}%`;
          conditions.push({
            field: key,
            op: ops.mode === "insensitive" ? "ilike" : "like",
            value: pattern,
          });
        }
        if ("endsWith" in ops) {
          const pattern = `%${ops.endsWith}`;
          conditions.push({
            field: key,
            op: ops.mode === "insensitive" ? "ilike" : "like",
            value: pattern,
          });
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

  /**
   * Convert Prisma orderBy to ORMDB order spec.
   */
  private convertOrderBy(
    orderBy: OrderByClause | OrderByClause[]
  ): OrderSpec[] {
    const specs = Array.isArray(orderBy) ? orderBy : [orderBy];
    const result: OrderSpec[] = [];

    for (const spec of specs) {
      for (const [field, direction] of Object.entries(spec)) {
        result.push({ field, direction: direction as "asc" | "desc" });
      }
    }

    return result;
  }

  /**
   * Convert Prisma select to field list.
   */
  private convertSelect(select: SelectClause): string[] {
    return Object.entries(select)
      .filter(([, include]) => include)
      .map(([field]) => field);
  }

  /**
   * Convert Prisma include to ORMDB relation includes.
   */
  private convertInclude(include: IncludeClause): RelationInclude[] {
    const result: RelationInclude[] = [];

    for (const [relation, options] of Object.entries(include)) {
      if (options === false) continue;

      const inc: RelationInclude = { relation };

      if (typeof options === "object") {
        if (options.where) {
          inc.filter = this.convertWhereClause(options.where);
        }
        if (options.orderBy) {
          inc.order_by = this.convertOrderBy(options.orderBy);
        }
        if (options.take !== undefined) {
          inc.limit = options.take;
        }
        if (options.include) {
          inc.includes = this.convertInclude(options.include);
        }
      }

      result.push(inc);
    }

    return result;
  }
}

/**
 * Create a Prisma-compatible client with dynamic model access.
 */
export function createPrismaClient(
  config: OrmdbConfig | string
): PrismaClient & Record<string, ModelDelegate> {
  const client = new PrismaClient(config);

  // Create a proxy to allow dynamic model access (e.g., prisma.user)
  return new Proxy(client, {
    get(target, prop: string) {
      // Return existing properties
      if (prop in target) {
        return (target as Record<string, unknown>)[prop];
      }

      // Return model delegate for entity names
      if (typeof prop === "string" && !prop.startsWith("$")) {
        // Convert camelCase to PascalCase for entity name
        const entityName = prop.charAt(0).toUpperCase() + prop.slice(1);
        return (target as unknown as { getModelDelegate: (name: string) => ModelDelegate })["getModelDelegate"](entityName);
      }

      return undefined;
    },
  }) as PrismaClient & Record<string, ModelDelegate>;
}

export default createPrismaClient;
