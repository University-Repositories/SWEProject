# SWEProject

A two player PVP game where players name songs related to each other by their artist or other details. First one to run out ideas loses.

## Repository layout

| Folder            | Contents                                                                   |
| ----------------- | -------------------------------------------------------------------------- |
| `gameEngine/`     | `game_engine` crate: game rules (link validation, credit use counting)     |
| `dataManagement/` | `data_management` crate: database migrations, repositories, catalog import |
| `authentication/` | `authentication` crate: accounts service                                   |
| `server/`         | `server` crate: web server and API                                         |
| `frontEnd/`       | React and TypeScript client (not set up yet)                               |

The root `Cargo.toml` makes the four Rust folders one Cargo workspace, so every `cargo` command below runs from the repo root.

## Prerequisites

- Git
- Rust 1.85 or newer, installed with [rustup](https://rustup.rs). On Windows, let the installer add the Visual Studio C++ build tools.
- Docker Desktop, or Docker Engine with the Compose plugin
- Node.js LTS (for the client, once `frontEnd/` is set up)

Check your Rust version with `rustc --version`.

## First-time setup

### 1. Clone the repo

```
git clone https://github.com/University-Repositories/SWEProject.git
cd SWEProject
```

### 2. Create your `.env` file

Create a file named `.env` at the repo root containing these lines. Git ignores it.

```
DATABASE_URL=postgres://beat4beat:beat4beat@localhost:5432/beat4beat
POSTGRES_USER=beat4beat
POSTGRES_PASSWORD=beat4beat
POSTGRES_DB=beat4beat
```

### 3. Start the database

```
docker compose up -d
```

The database user, password, and database name default to `beat4beat`. Database tables are defined in `dataManagement/migrations/0001_init.sql`.

### 4. Check that it accepts connections

```
docker compose exec db pg_isready -U beat4beat
```

The output should end with `accepting connections`.

### 5. Build and test everything

```
cargo build
cargo test
```

### 6. (Optional) Populate with 1000 Pop Songs

Populate the songs catalog from MusicBrainz:

```
cargo run -p data_management --example populate_pop
```

## Everyday commands

| Task                           | Command                                         |
| ------------------------------ | ----------------------------------------------- |
| Start the database             | `docker compose up -d`                          |
| Stop the database (keeps data) | `docker compose down`                           |
| Stop and delete all data       | `docker compose down -v`                        |
| Build                          | `cargo build`                                   |
| Run all tests                  | `cargo test`                                    |
| Run database tests             | `cargo test -p data_management --test game_db_test` |
| Populate songs (1000 pop)      | `cargo run -p data_management --example populate_pop` |
| Format code                    | `cargo fmt --all`                               |
| Lint                           | `cargo clippy --all-targets -- -D warnings`     |

Run the format and lint commands before you push.

## Workflow

- Branch from `main` and open a pull request. Nobody pushes to `main` directly.
- Every pull request needs one approving review and passing CI before it merges.
- Never commit `.env`.

## Troubleshooting

- Port 5432 already in use: another PostgreSQL is running on your machine. Stop it, then run `docker compose up -d` again.
- Windows build error mentioning `link.exe` or `link: extra operand`: the Visual Studio C++ build tools are missing. Add the "Desktop development with C++" workload in the Visual Studio Installer.
