# DB Sync Studio

基于 Tauri、React 和 Ant Design 的本地优先数据库结构与数据对比工具。

[English README](./README.md)

## 功能

- MySQL、PostgreSQL 和 SQLite 连接管理
- 结构同步对比：表、字段、注释、PostgreSQL 枚举类型
- 数据同步对比：支持多表
- 按新增 / 更新 / 删除 / 相同展示结果统计
- 按表和操作类型分组生成 SQL 预览
- SQL 行号和语法高亮
- 本地比较历史，支持同步类型、数据库类型、时间范围和内容搜索
- 中英文界面
- 浅色 / 深色主题

## 下载

从这里下载最新的 macOS、Windows 和 Linux 安装包：

[GitHub Releases](https://github.com/SShnoodles/db-sync-studio/releases)

## 开发

安装依赖：

```bash
pnpm install
```

启动前端：

```bash
pnpm dev
```

启动 Tauri 应用：

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

连接配置和比较历史保存在本地 SQLite 中，不会上传到远端服务。

macOS 默认路径：

```text
~/Library/Application Support/cc.ssnoodles.db-sync-studio/db-sync-studio.sqlite
```

## 当前限制

- 源库和目标库必须是同一种数据库类型。
- 数据同步依赖主键。
- 结构同步和数据同步都可在目标库执行选中的 SQL。
- 大表对比受当前行数读取限制影响。
