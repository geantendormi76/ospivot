# Node / TypeScript Workspace

`node/` 是仓库中唯一的 Node.js / TypeScript 领域边界。

## 放什么

- `packages/`：可复用的 Agent Harness、扩展、集成和领域模块。
- `scripts/`：可长期复用的 Node 开发工具。一次性检查或临时测试必须放到 `tmp/`。
- `tests/`：Node 工作区级集成测试；单个 package 的测试放在对应 package 的 `tests/`。

## 不放什么

- React / Tailwind / Tauri UI：放 `apps/`。
- Rust 系统能力：放 `crates/`。
- Python 模型、数据、实验和评估：放 `python/`。

这层目录本身就是语言边界：看到 `node/`，默认使用 Node.js + TypeScript；
不需要把 Python、Rust 和 Node 代码混到同一个实现目录里。
