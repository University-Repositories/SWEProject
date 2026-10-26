# Data Management (`data_management`)

This crate manages the PostgreSQL database schema migrations, repositories (`UserRepo`, `SessionRepo`, `CatalogRepo`), and catalog import for Beat4Beat.

---

## 1. Migrations

Migrations are located in `migrations/`:
* `0001_users_and_sessions.sql`: Creates `users` and `sessions` tables.
* `0002_catalog.sql`: Enables `pg_trgm` extension and creates `recordings`, `credit_entities`, and `recording_credits` tables.

To apply migrations manually:
```bash
sqlx migrate run --source migrations
```

---

## 2. Repositories

* **`UserRepo`**:
  * `create(username, password_hash) -> Result<UserRow, RepoError>` (Returns `Conflict` on case-insensitive duplicate username)
  * `find_by_username(username) -> Result<Option<UserRow>, RepoError>` (Matches ignoring case)
  * `find_by_id(id) -> Result<Option<UserRow>, RepoError>`
* **`SessionRepo`**:
  * `create(user_id, token_hash, expires_at) -> Result<(), RepoError>`
  * `find_valid(token_hash, now) -> Result<Option<SessionRow>, RepoError>` (Expired sessions return `None`)
  * `delete(token_hash) -> Result<(), RepoError>` (Deleting missing session returns `Ok(())`)
* **`CatalogRepo`**:
  * `search(query, limit) -> Result<Vec<RecordingSummaryRow>, RepoError>` (Ranked by trigram similarity, breaks ties by `year ASC`, then `id`)
  * `get_recording(id) -> Result<Option<RecordingDetailRow>, RepoError>` (Includes credits ordered by `role`, then `name`)
  * `get_credit_ids(id) -> Result<Vec<Uuid>, RepoError>` (Distinct credit entity IDs)

All repository methods also offer an `_with_executor` variant for use inside transactions.

---

## 3. Catalog Import

To run the catalog import binary:
```bash
cargo run -p data_management --bin catalog_import
```

It filters recordings by `min_credit_count` (configurable via `MIN_CREDIT_COUNT`, default: 2), loads them into the database idempotently, and outputs a coverage report.

---

## 4. Running Tests

```bash
cargo test -p data_management --test game_db_test
```
