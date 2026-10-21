# Hermes 交接：Pi Rust 内核

## 用户要求

在当前 Pi fork 上降低内存、迁移完整 Rust 内核，同时兼容原来的 Pi 插件。
用户已选定兼容优先、保留 Node；Node-API 同进程连接 Rust，不改写现有插件。
步骤与验收条件见 [TODO.md](TODO.md)。用户要求本地 Codex 停止开发，由 wayamac 上的 Hermes 接手。

仓库：<https://github.com/Wang-Yang-source/Mio>

交接分支：`codex/rust-kernel-handoff`。这是未完成的原型交接，不是可用的 Rust 后端。
先读根目录 `AGENTS.md`、`README.md` 和本文件；遵守项目检查及版本管理要求，不自动提交或发布后续功能。

## 当前文件与实际行为

- `TODO.md`：完整迁移步骤和未完成验收。
- `native/`：独立 Cargo workspace。`kernel` 是无依赖回合调度状态机；`node` 是 napi-rs 类绑定，协议版本 1。
- `packages/agent/src/loop-driver.ts`：动作/反馈类型和同步驱动接口。
- `packages/agent/src/agent.ts`、`types.ts`：传递实验性的 `loopDriverFactory` 参数。

**关键限制：`agent-loop.ts` 尚未使用这个参数；没有原生模块加载器、后端选择、打包脚本或闭环运行。默认和实际执行均仍为原来的 TypeScript。**

Rust 原型只保存调度状态，不保存消息正文，也不调用模型或工具。尚未验证 JS 与原生模块的真实加载、异常/取消桥接和释放语义。内存基准、兼容矩阵、差分测试均未实现。

本地旧项目位于 `Mio-Backup/Mio`，是独立仓库，仅供参考，未随本次交接提交。
不从备份目录加载依赖或凭据；无需依赖备份即可继续当前 Pi 工作。

## 已执行验证

环境：Linux，Node 26.8.2、npm 11.19.1、Rust/Cargo 1.98.1。

- `npm ci --ignore-scripts`：成功；已有 autoevals 包管理器 engine 警告、依赖弃用提示、3 项 high 漏洞，未自动升级依赖。
- `cargo fmt --manifest-path native/Cargo.toml --all --check`：通过。
- `cargo check --manifest-path native/Cargo.toml --all-targets --locked`：通过。
- `cargo test --manifest-path native/Cargo.toml --locked`：纯内核 4 项通过；不包括 Node 加载或 Agent 闭环。
- `cargo clippy --manifest-path native/Cargo.toml --all-targets --locked -- -D warnings`：通过。
- 在 `packages/agent` 执行 `node ../../node_modules/vitest/dist/cli.js --run test/agent.test.ts test/agent-loop.test.ts`：78 项通过，验证的是现有 TypeScript 行为。

首次 Agent 测试因缺少生成的模型 JSON 而失败。随后按 `nix/model-catalog.json` 中固定 revision 下载目录，验证 SHA-256，再运行 `packages/ai/scripts/hydrate-model-catalog.ts`，复跑通过。未调用真实模型或写入模型生成源码。

原工作目录的 `npm run check` 被备份中 `.cache/codex-src/package.json` 的非固定 prettier 版本阻断。验证副本只包含本次源码快照，不包含旧备份。固定目录还缺少 `stream.test.ts` 引用的 NVIDIA 模型，导致一次类型检查失败。下载最新公开目录并核验 SHA-256 后，验证副本的 **`npm run check` 全部通过**（格式、依赖、入口、安装锁、类型、浏览器 smoke）。不要为通过检查修改旧备份。

通过检查使用的模型目录 revision：`sha256-ce2b35e558368354315ceab5a0bc1d50e53d405e8f261ea019a491591bb0a758`。可从 `https://pi.dev/api/models/revisions/<revision>?types=chat,image,classifier` 下载，核验散列后执行 `node packages/ai/scripts/hydrate-model-catalog.ts <下载文件>`。这些 JSON/类型数据是忽略文件；本次未改动 `nix/model-catalog.json` 或模型生成源码。

## 接手顺序

1. 拉取交接分支，确认文件和提交；将旧备份放在检查范围之外。
2. 优先建立原版内存/行为基准；检查下列已有 Rust 项目，确认可借鉴点和兼容缺口。
3. 审核当前原型。Agent/SDK 会共享和修改活跃消息对象，不能直接 JSON 往返整个上下文。
4. 若继续当前架构，先完成显式可选的原生加载与调度闭环、失败释放、差分测试，保留 TypeScript 默认。
5. 再按 TODO 顺序迁移。不要把编译通过、调度器单测或 README 声明当作完整兼容/省内存验收。

## 已有 Rust Pi：2026-10-21 初查

这些结论来自仓库 README 与具体源码，未下载运行或测量性能。

| 项目 | 与目标的关系 | 已确认的兼容边界 |
| --- | --- | --- |
| [yannelli/pi-rust](https://github.com/yannelli/pi-rust) | 最值得先评估的 Pi 全栈 Rust 移植，包含 Agent、Provider、TUI、会话及扩展运行时 | 使用 QuickJS 与 Node 兼容模块；不能加载原生 npm addon，并有明确的 API 行为差异 |
| [Dicklesworthstone/pi_agent_rust](https://github.com/Dicklesworthstone/pi_agent_rust) | Rust 原生 Agent，有自己的扩展与资源控制实现 | 当前 README 明确不再追求严格替代 TypeScript Pi，不能当作插件完全兼容方案 |
| [bigfish1913/pi-rust](https://github.com/bigfish1913/pi-rust) | 可参考 Rust SDK 分层及工具实现 | 使用 Rust cdylib/C ABI 插件，明确不加载 Pi TypeScript SDK 或 npm 扩展 |

`yannelli/pi-rust` 的 [unsupported.rs](https://github.com/yannelli/pi-rust/blob/main/crates/pi-js/src/unsupported.rs) 列出未实现的 fs/crypto/stream 等 API，以及 async_hooks、子进程、网络和对象行为差异。模块可 import 不等于对应 Node 行为完全一致。其 [v0.5.0](https://github.com/yannelli/pi-rust/releases/tag/v0.5.0) 发布于 2026-10-20。

`Dicklesworthstone/pi_agent_rust` 的 [README](https://github.com/Dicklesworthstone/pi_agent_rust/blob/main/README.md) 已改变产品方向；[LICENSE](https://github.com/Dicklesworthstone/pi_agent_rust/blob/main/LICENSE) 带额外限制，不应按普通 MIT 项目直接移植。

建议先用冻结的插件集合评估 `yannelli/pi-rust`。在没有行为和内存实测前，不宣称它能满足完整插件兼容，也不直接替换已选的 Node + Rust 架构。

## 交付状态

wayamac 的 SSH 连接在两次有限尝试中超时，尚未向 Hermes 投递或启动远程任务。GitHub 交接分支用于后续拉取，不等于 Hermes 已接单。
