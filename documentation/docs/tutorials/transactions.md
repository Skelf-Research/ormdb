# Transactions

Learn how to use ACID transactions for data integrity.

## What Are Transactions?

A transaction groups multiple operations so they either all succeed or all fail. ORMDB transactions provide:

- **Atomicity** - All operations succeed or none do
- **Consistency** - Constraints are always enforced
- **Isolation** - Concurrent transactions don't interfere
- **Durability** - Committed changes survive crashes

## Basic Transaction

=== "Rust"

    ```rust
    use ormdb_core::storage::Transaction;

    // Start a transaction
    let mut tx = storage.begin_transaction()?;

    // Perform operations
    tx.insert("User", user_id, &user_data)?;
    tx.insert("Profile", profile_id, &profile_data)?;

    // Commit the transaction
    tx.commit()?;
    ```

=== "TypeScript"

    ```typescript
    // Start a transaction
    const tx = await client.beginTransaction();

    try {
      // Perform operations
      const userResult = await tx.insert("User", {
        name: "Alice",
        email: "alice@example.com",
      });

      await tx.insert("Profile", {
        user_id: userResult.insertedIds[0],
        bio: "Hello!",
      });

      // Commit
      await tx.commit();
    } catch (error) {
      // Rollback on error
      await tx.rollback();
      throw error;
    }
    ```

=== "Python"

    ```python
    # Using context manager (auto-rollback on error)
    with client.transaction() as tx:
        user_result = tx.insert("User", {
            "name": "Alice",
            "email": "alice@example.com",
        })

        tx.insert("Profile", {
            "user_id": user_result.inserted_ids[0],
            "bio": "Hello!",
        })
        # Commits automatically at end of block

    # Manual transaction
    tx = client.begin_transaction()
    try:
        tx.insert("User", {...})
        tx.commit()
    except:
        tx.rollback()
        raise
    ```

## Use Cases

### Transfer Between Accounts

=== "Rust"

    ```rust
    let mut tx = storage.begin_transaction()?;

    // Debit from source
    let source = tx.get("Account", source_id)?;
    let new_source_balance = source.balance - amount;
    tx.update("Account", source_id, &Account { balance: new_source_balance })?;

    // Credit to destination
    let dest = tx.get("Account", dest_id)?;
    let new_dest_balance = dest.balance + amount;
    tx.update("Account", dest_id, &Account { balance: new_dest_balance })?;

    tx.commit()?;
    // Both updates succeed or neither does
    ```

=== "TypeScript"

    ```typescript
    const tx = await client.beginTransaction();

    try {
      // Debit from source
      const source = await tx.findById("Account", sourceId);
      await tx.update("Account", sourceId, {
        balance: source.balance - amount,
      });

      // Credit to destination
      const dest = await tx.findById("Account", destId);
      await tx.update("Account", destId, {
        balance: dest.balance + amount,
      });

      await tx.commit();
    } catch (error) {
      await tx.rollback();
      throw error;
    }
    ```

### Create Related Entities

=== "Rust"

    ```rust
    let mut tx = storage.begin_transaction()?;

    // Create user
    let user_id = StorageEngine::generate_id();
    tx.insert("User", user_id, &user)?;

    // Create their profile
    tx.insert("Profile", profile_id, &Profile { user_id, ... })?;

    // Create default settings
    tx.insert("Settings", settings_id, &Settings { user_id, ... })?;

    tx.commit()?;
    ```

=== "TypeScript"

    ```typescript
    const tx = await client.beginTransaction();

    try {
      const userResult = await tx.insert("User", { name: "Alice" });
      const userId = userResult.insertedIds[0];

      await tx.insert("Profile", { user_id: userId, bio: "" });
      await tx.insert("Settings", { user_id: userId, theme: "light" });

      await tx.commit();
    } catch (error) {
      await tx.rollback();
      throw error;
    }
    ```

## Optimistic Concurrency

ORMDB uses optimistic concurrency control with version checks:

=== "Rust"

    ```rust
    let mut tx = storage.begin_transaction()?;

    // Read with version
    let (version, user) = tx.get_with_version("User", user_id)?;

    // Update with expected version
    let result = tx.update_if_version("User", user_id, version, &updated_user);

    match result {
        Ok(_) => tx.commit()?,
        Err(Error::TransactionConflict { .. }) => {
            // Another transaction modified this record
            // Retry with fresh data
        }
    }
    ```

=== "TypeScript"

    ```typescript
    const tx = await client.beginTransaction();

    try {
      // Read current state
      const user = await tx.findById("User", userId);

      // Update with optimistic locking
      await tx.update("User", userId, { ...updates }, {
        expectedVersion: user._version,
      });

      await tx.commit();
    } catch (error) {
      if (error.code === "VERSION_CONFLICT") {
        // Retry with fresh data
      }
      await tx.rollback();
      throw error;
    }
    ```

## Transaction Isolation

ORMDB provides snapshot isolation:

- Transactions see a consistent snapshot of data
- Writes from other transactions are not visible until commit
- Write conflicts are detected at commit time

=== "Rust"

    ```rust
    // Transaction 1
    let mut tx1 = storage.begin_transaction()?;
    let user = tx1.get("User", user_id)?;  // Reads version 1

    // Transaction 2 commits an update
    let mut tx2 = storage.begin_transaction()?;
    tx2.update("User", user_id, &updated)?;
    tx2.commit()?;  // Now at version 2

    // Transaction 1 still sees version 1
    let user_again = tx1.get("User", user_id)?;  // Still version 1

    // Conflict on commit if tx1 tries to update
    tx1.update("User", user_id, &my_update)?;
    tx1.commit()?;  // Error: TransactionConflict
    ```

## Error Handling

=== "Rust"

    ```rust
    use ormdb_core::error::Error;

    let mut tx = storage.begin_transaction()?;

    match tx.commit() {
        Ok(_) => println!("Transaction committed"),
        Err(Error::TransactionConflict { entity_id, expected, actual }) => {
            println!("Conflict: expected version {}, found {}", expected, actual);
        }
        Err(Error::ConstraintViolation(e)) => {
            println!("Constraint violated: {:?}", e);
        }
        Err(e) => {
            println!("Transaction failed: {}", e);
        }
    }
    ```

=== "TypeScript"

    ```typescript
    const tx = await client.beginTransaction();

    try {
      await tx.insert("User", data);
      await tx.commit();
    } catch (error) {
      await tx.rollback();

      if (error.code === "VERSION_CONFLICT") {
        console.log("Concurrent modification detected");
      } else if (error.code === "CONSTRAINT_VIOLATION") {
        console.log("Constraint violated:", error.message);
      } else {
        throw error;
      }
    }
    ```

## Best Practices

1. **Keep transactions short** - Long transactions increase conflict probability
2. **Don't hold transactions across I/O** - Complete quickly, don't wait for user input
3. **Handle conflicts gracefully** - Retry or report to user
4. **Use appropriate isolation** - Understand what your transaction sees
5. **Prefer upsert over read-then-write** - Reduces conflict window

## Next Steps

- **[Mutations](mutations.md)** - Individual mutations
- **[Error Handling](../reference/errors.md)** - Transaction error types
