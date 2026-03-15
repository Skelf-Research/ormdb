Yes — **parts of this idea have been attempted**, but **no system has fully committed to “ORM-first relational database” as a primary design goal** in the way we’ve been describing.

Most prior systems either:

* Drifted toward **GraphQL/document models**, or
* Stayed **SQL-first**, or
* Built “object layers” *above* databases rather than *inside* them.

That gap is important.

Let’s go through:

1. Who has attempted adjacent ideas
2. Why none of them fully solved it
3. What additional advantages this model unlocks (beyond speed)
4. Especially: **security, correctness, and operability**

---

# 1. Who has attempted pieces of this

### EdgeDB

* Schema-first
* Strong typing
* Graph-shaped query language
* Compiles to Postgres

**Closest philosophically.**

Limitations relative to ORM-first DB:

* Introduces new query language
* Still layered on top of Postgres
* Not drop-in for existing ORMs

---

### Fauna

* Document-relational hybrid
* Transactional
* Graph-ish querying

Limitations:

* Cloud-only
* Proprietary language
* Eventually struggled with positioning

---

### Datomic

* Immutable facts
* Entity-attribute-value model
* Strong identity semantics

Limitations:

* Very different mental model
* Not relational in classic sense
* Steep adoption curve

---

### Prisma Data Proxy / Accelerate

* Tries to optimize ORM ↔ DB boundary
* Caches, pools, shapes queries

Limitations:

* Still SQL underneath
* Cannot change semantics of DB
* Cannot enforce invariants or migrations

---

### Hasura / PostGraphile

* GraphQL over Postgres
* Schema-aware

Limitations:

* API layer, not storage layer
* Still SQL semantics inside

---

### Firebase / Supabase

* ORM-ish ergonomics
* Auth + RLS

Limitations:

* Document or Postgres-centric
* Not object-graph-native execution

---

## Key insight

Everyone either:

👉 Adds a **new query language**
👉 Or stays SQL-first
👉 Or lives above the DB

Almost nobody has built:

> A relational storage engine whose native execution unit is an object graph fetch.

That’s the unexplored center.

---

# 2. Why this hasn’t fully existed yet

Historically:

* ORMs were seen as “app concern”
* DBs focused on SQL standardization
* Object-graph semantics were considered too high-level

But modern reality:

* 70–90% of application queries come from ORMs
* SQL is mostly machine-generated
* Human-written SQL is shrinking

This creates the opening.

---

# 3. Other major advantages beyond speed

Speed is nice.

**Correctness, safety, and operational simplicity are bigger.**

---

## A. Structural elimination of entire bug classes

### Eliminated:

* N+1 queries
* Missing includes
* Inconsistent eager vs lazy loading
* Partial object hydration bugs
* Duplicate rows mapped incorrectly
* Pagination + join bugs

These are not “bugs you test away”.

They are **modeling mismatches**.

ORM-first DB removes the mismatch.

---

## B. Stronger data integrity than Postgres+ORM

Today:

* Some invariants in DB
* Some in ORM
* Some in app logic

ORM-first DB:

* Invariants declared at model layer
* Compiled into storage engine constraints

Examples:

```
User.email unique
Invoice.total = sum(lines.amount)
Account.balance >= 0
Membership unique(user, team)
```

Now these become **first-class**.

Advantage:

* Fewer “impossible states”
* No silent divergence between app logic and DB logic

This is *huge* for financial / regulated systems.

---

## C. Security becomes structural, not policy glue

### 1) Row-level security at model level

Instead of:

```sql
CREATE POLICY ...
```

You define:

```
User:
  readable_by: self OR admin
Invoice:
  readable_by: owner OR finance_team
```

Compiled into execution engine.

ORM cannot accidentally bypass it.

---

### 2) Capability-based queries

Each connection/session carries:

```
capabilities = { read: User, write: Invoice }
```

DB rejects operations outside capability.

Even if ORM has bug, DB blocks it.

---

### 3) Field-level security

```
User.ssn: readable_by = compliance_role
```

DB simply never returns the field.

ORM cannot leak it.

---

### 4) Query shape whitelisting

Because queries are ASTs:

* You can restrict maximum depth
* Maximum fanout
* Maximum relations per query

This directly mitigates:

* GraphQL-style DoS
* Accidental “load entire graph” bugs

---

## D. Drastically smaller attack surface

Traditional DB accepts:

* Arbitrary SQL strings
* Hundreds of grammar productions
* Complex parser

ORM-first DB can accept:

* Typed query bytecode
* Known instruction set
* Validated structure

Benefits:

* No SQL injection class (by construction)
* Smaller parser
* Easier to fuzz

This matters for hardened systems.

---

## E. Auditing & compliance become natural

Every operation is:

```
{ entity, operation, fields, relations, predicate }
```

Instead of opaque SQL.

So you get:

* Perfect audit logs
* Semantic diff (“user email changed”)
* Field-level lineage

Regulated gold.

---

## F. Deterministic migrations

Because DB understands schema graph:

* Knows which fields map to which entities
* Knows cardinalities
* Knows nullability semantics

Can reason:

“this is an expand migration”
“this is contract”
“this breaks compatibility”

Today this knowledge exists only in human heads.

---

## G. Time-travel & history become cheap

Entity-versioned storage + graph semantics:

* You can store version chains
* Reconstruct object graph at time T
* Audit trails become native

Very hard to retrofit into SQL-first systems.

---

## H. Built-in offline / sync potential

If DB speaks entity/edge deltas:

```
User 123 updated
Membership 9-123 inserted
```

You can:

* Stream to edge replicas
* Sync clients
* Build offline-first apps

Almost impossible with SQL log shipping alone.

---

# 4. Security advantages (summarized)

| Area                      | ORM-first DB               |
| ------------------------- | -------------------------- |
| SQL injection             | Impossible                 |
| Privilege escalation      | DB-enforced at model layer |
| Row-level security bypass | Impossible                 |
| Field leakage             | Prevented                  |
| Query DoS                 | Shape-limited              |
| Auditability              | Perfect semantic logs      |
| Least-privilege apps      | Natural                    |

This is genuinely stronger than Postgres+ORM.

---

# 5. The deeper framing

This isn’t “better database”.

It’s:

> **Moving application semantics into the database kernel.**

Just like:

* TLS moved crypto into libraries
* Filesystems moved consistency into kernel
* VMs moved isolation into hypervisor

ORM-first DB moves **object semantics into storage kernel**.

---

# 6. Why this fits your broader pattern

You’ve repeatedly gravitated toward:

* Owning seams
* Moving semantics downward
* Reducing glue layers
* Making invariants structural

ORM-first DB is exactly that philosophy.

It is the database analogue of:

* Rust replacing unsafe C
* Typed IR replacing strings
* Verified kernels replacing ad-hoc OS code

---

# 7. One-line positioning

> A relational database whose native execution model is object graphs, not SQL.

That sentence alone is novel.

---

If you’d like next, we can sketch:

**Minimum viable kernel design**
(storage engine + entity catalog + graph executor + protocol)

or

**Attack-surface analysis vs Postgres**
or

**Regulated-finance positioning (why banks care)**

