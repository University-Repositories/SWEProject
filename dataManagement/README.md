# Data Management (`data_management`)

This crate manages the PostgreSQL database schema migrations, repositories (`UserRepo`, `SessionRepo`, `CatalogRepo`), dev fixtures, and catalog imports for Beat4Beat.

---

## Quick Reference Commands

All commands below are run from the **repository root**:

| Action | Command |
| :--- | :--- |
| **Start Database** | `docker compose up -d` |
| **Run Migrations** | `sqlx migrate run --source dataManagement/migrations` |
| **Seed Dev Fixture (CLI)** | `cargo run -p data_management --bin catalog_import -- --fixture` |
| **Seed Dev Fixture (Docker SQL)** | `docker compose exec -T db psql -U beat4beat -d beat4beat < dataManagement/fixtures/dev_fixture.sql` |
| **Run Catalog Import (100+ Songs)** | `cargo run -p data_management --bin catalog_import` |
| **Run Integration Tests** | `cargo test -p data_management --test game_db_test` |
| **Format Check** | `cargo fmt --all -- --check` |
| **Lint Check** | `cargo clippy --all-targets -- -D warnings` |

---

## 1. Migrations

Migrations are located in `migrations/`:
* `0001_users_and_sessions.sql`: Creates `users` and `sessions` tables with indexes and constraints.
* `0002_catalog.sql`: Enables `pg_trgm` extension and creates `recordings`, `credit_entities`, and `recording_credits` tables with trigram GIN indexes.

To apply migrations manually:
```bash
# From repo root:
sqlx migrate run --source dataManagement/migrations

# Or from inside dataManagement/:
sqlx migrate run --source migrations
```

---

## 2. Dev Fixture

A seed script with 12 interconnected songs, shared credits, and same-title edge cases (`"Hold On"` by different artists) to unblock service and UI development.

* **Via CLI:**
  ```bash
  cargo run -p data_management --bin catalog_import -- --fixture
  ```
* **Programmatically in Rust (inside services or tests):**
  ```rust
  data_management::seed_dev_fixture(&pool).await?;
  ```
* **Via Docker SQL:**
  ```bash
  docker compose exec -T db psql -U beat4beat -d beat4beat < dataManagement/fixtures/dev_fixture.sql
  ```

---

## 3. Catalog Import & Coverage Report

The `catalog_import` binary loads MusicBrainz recordings, validates credits, filters songs with $< 2$ distinct credit entities, and prints a coverage report.

* **Run with bundled offline dataset (100+ songs):**
  ```bash
  cargo run -p data_management --bin catalog_import
  ```
* **Run with a custom JSON dataset:**
  ```bash
  cargo run -p data_management --bin catalog_import -- path/to/dataset.json
  ```
* **Override minimum credit threshold:**
  ```bash
  MIN_CREDIT_COUNT=3 cargo run -p data_management --bin catalog_import
  ```

---

## 4. Repositories

* **`UserRepo`**:
  * `create(username, password_hash) -> Result<UserRow, RepoError>` (Returns `Conflict` on case-insensitive duplicate username)
  * `find_by_username(username) -> Result<Option<UserRow>, RepoError>` (Matches ignoring case)
  * `find_by_id(id) -> Result<Option<UserRow>, RepoError>`
* **`SessionRepo`**:
  * `create(user_id, token_hash, expires_at) -> Result<(), RepoError>`
  * `find_valid(token_hash, now) -> Result<Option<SessionRow>, RepoError>` (Expired sessions return `None`)
  * `delete(token_hash) -> Result<(), RepoError>` (Safe no-op if session is missing)
* **`CatalogRepo`**:
  * `search(query, limit) -> Result<Vec<RecordingSummaryRow>, RepoError>` (Ranked by trigram similarity, breaks ties by `year ASC`, then `id`)
  * `get_recording(id) -> Result<Option<RecordingDetailRow>, RepoError>` (Credits ordered by `role`, then `name`)
  * `get_credit_ids(id) -> Result<Vec<Uuid>, RepoError>` (Deduplicated credit entity IDs)

All repository methods also offer an `_with_executor` variant for use inside database transactions.

---

## 5. Running Tests

```bash
# Run all dataManagement tests:
cargo test -p data_management

# Run integration tests with console output:
cargo test -p data_management --test game_db_test -- --nocapture
```
