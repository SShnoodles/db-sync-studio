# DB Sync Studio

Local-first database schema and data comparison tool built with Tauri, React, and Ant Design.

[中文文档](./README.zh-CN.md)

## Features

- MySQL, PostgreSQL, and SQLite connection management
- Schema sync comparison for tables, columns, comments, and PostgreSQL enum types
- Data sync comparison across multiple tables
- Insert / Update / Delete / Same result summary
- SQL preview grouped by table and operation type
- Line numbers and syntax highlighting for generated SQL
- Local comparison history with sync type, database type, time range, and content search
- English / Chinese UI
- Light / dark theme

## Download

Download the latest macOS, Windows, and Linux builds from:

[GitHub Releases](https://github.com/SShnoodles/db-sync-studio/releases)

## Development

Install dependencies:

```bash
pnpm install
```

Start the frontend:

```bash
pnpm dev
```

Start the Tauri app:

```bash
pnpm tauri dev
```

Build the frontend:

```bash
pnpm build
```

Check Rust:

```bash
cd src-tauri
cargo check
```

## Local data

Connection settings and comparison history are stored locally in SQLite. They are not uploaded to any remote service.

Default macOS path:

```text
~/Library/Application Support/cc.ssnoodles.db-sync-studio/db-sync-studio.sqlite
```

## Current limits

- Source and target must use the same database type.
- Data sync requires primary keys.
- Schema and data sync can execute selected SQL on the target database.
- Large table comparison is limited by the current row fetch limit.
