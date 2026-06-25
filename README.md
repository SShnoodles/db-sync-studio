# DB Sync Studio

本地数据库同步工具，基于 Tauri + React + Ant Design。

## 功能

- MySQL 连接管理：本地保存、测试连接
- 结构同步：选择源库/目标库，勾选数据表，比较表和字段差异
- 数据同步：多表比较，按 Insert / Update / Delete / Same 展示结果
- SQL 预览：按表、操作类型分组，带行号和高亮
- 历史记录：保存结构同步和数据同步记录，支持类型和时间筛选
- 设置：中英文切换、浅色/深色主题

## 开发

```bash
pnpm install
pnpm dev
```

启动 Tauri：

```bash
pnpm tauri dev
```

构建前端：

```bash
pnpm build
```

检查 Rust：

```bash
cd src-tauri
cargo check
```

## 本地数据

连接配置和比较历史保存在应用本地 SQLite 中，不上传远端。

macOS 默认路径：

```text
~/Library/Application Support/cc.ssnoodles.db-sync-studio/db-sync-studio.sqlite
```

## 当前限制

- 当前仅支持 MySQL
- 数据同步依赖主键比较
- SQL 只生成预览，不直接执行
