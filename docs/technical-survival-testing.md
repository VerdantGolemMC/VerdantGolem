# 生电回归测试

这组测试覆盖旋转器、强加载持久化、假人动作和漏斗测产。测试结果应以当前提交实际运行的输出为准，
而不是仅依据规则目录或历史 CI。

本次开发后续的所有构建、测试、lint 与同步预演统一以 GitHub Actions 为验收来源。
本地只编辑并推送代码；下面的 Cargo/Python 命令也由 CI job 执行。

## Rust 检查

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features
cargo nextest run --locked --workspace
cargo test --locked --workspace --doc
```

内存较小的开发机器可以设置 `CARGO_BUILD_JOBS=2`、`CARGO_PROFILE_DEV_DEBUG=0` 和
`CARGO_PROFILE_TEST_DEBUG=0`，减少同时编译与调试信息的开销；这不会关闭测试或业务断言。

主要回归分组：

- `block::blocks::redstone::dispenser::tests`：旋转成功/目标不变都保留仙人掌；关闭规则恢复普通发射。
- `world::forced_chunks::tests`：存档与 ticket 恢复、重复操作、删除、维度隔离、旧路径、损坏文件、
  越界坐标、原子写失败和并发变更。
- `carpet::fake_player::actions::tests`：单次/持续/间隔调度、精确 tick 间隔、松开、停止与移动输入。
- `entity::player::interaction::tests`：射线/实际方块轮廓、遮挡、主副手优先级、插件否决后的停止。
- `entity::player::local_movement::tests` 与实体物理测试：仅 Local 使用服务器运动、状态门控、墙体碰撞和朝向。
- `carpet::counters::tests`、`command::commands::counter::tests`、`block::entities::hopper::tests`：
  两种时钟、并发快照、批量计数、重置、溢出和命令兼容性。

## 临时世界端到端测试

```sh
python3 -m unittest discover -s tools/tests -p 'test_technical_survival_smoke.py' -v
cargo build --locked -p verdantgolem --bin verdantgolem
python3 tools/tests/technical_survival_smoke.py --server "$(pwd)/target/debug/verdantgolem" --keep
```

脚本用 Python 标准库启动指定二进制，创建独立临时存档，并只监听 loopback 随机端口。
它通过 RCON 控制模拟 tick，通过服务器实际写出的 Anvil/NBT 验证方块和物品数据，
不会使用或清理仓库中的现有世界，也不需要真实 Minecraft 客户端。

覆盖场景：

1. 远离出生区的 `/forceload add`，正常停止、重启后查询并验证区块实际加载，再移除并再次重启。
2. 开启 `rotatorBlock` 时对石头和空气触发旋转器，核对库存未减少；关闭规则的负对照确实消耗一个仙人掌。
3. 上方箱子向初始为空的计数漏斗供料；另测多槽批量计数、实际消耗、freeze/step/rate/sprint 与重置。
4. 临时命令方块生成假人，验证移动/停止、单次物品放置，以及保存后的方块状态和库存数量；
   主手持无空中使用动作的方块、副手持经验瓶时，验证只消耗副手物品。

失败时保留临时目录，其中有各次启动日志、RCON 命令记录、失败原因和汇总文件。
成功默认删除临时世界；`--keep` 保留成功现场。脚本仅停止自己创建的子进程。
`--skip-fake-player` 可用于单独排查前三组，但不应作为全量验收的替代。

## 前阶段本地验证历史（2026-09-12）

环境：macOS ARM64、Rust 1.97.0；基线 `c52839ee3` 加本地
`codex/technical-survival-completion` 分支的实现。这些记录产生于切换 GH 验证之前；最新交付以 PR 中的 Actions 结果为准。

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` / `git diff --check` | 通过 |
| 全工作区 `clippy --all-targets --all-features` | 通过 |
| 主服务端 nextest | 413 通过，0 失败，0 跳过 |
| 其余工作区 nextest | 445 通过，0 失败，0 跳过 |
| Python smoke 工具自检 | 16 通过 |
| 临时世界端到端检查 | 10 通过，包含 3 次正常启动/停止 |
| 工作区文档测试 | 7 通过；23 个仓库原有 `ignore` 示例保持忽略 |

端到端结果还确认了：旋转规则启用时仙人掌保持 4 个，禁用的负对照减少为 3 个；
上方箱子经空计数漏斗传入的 3 个物品全部计入且存档库存清空；假人单次放置把 4 个石头变为 3 个，
随后使用副手的 2 个经验瓶只消耗其中 1 个，主手仍保留 3 个石头。

Clippy 仍报告基线中已有的未知 `unused_async_trait_impl` lint 和旧插件 lint expectation 警告；
依赖 `proc-macro-error2` 有 future-incompatibility 提示。调试构建的 macOS linker 也提示 unwind 表较大。
这些检查没有新增错误，也没有通过关闭断言或移除测试来规避问题；此轮未进行发布构建性能测试。
