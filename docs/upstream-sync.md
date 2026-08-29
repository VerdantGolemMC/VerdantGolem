# 上游同步机制（Upstream Sync）

VerdantGolem 通过 GitHub Actions 自动跟踪上游 [Pumpkin-MC/Pumpkin](https://github.com/Pumpkin-MC/Pumpkin)。

## 工作方式

`.github/workflows/upstream-sync.yml` 每天定时（03:00 UTC）和手动（`workflow_dispatch`）执行：

1. **检查**：`git merge-base HEAD upstream/master`，若上游无新提交则直接结束。
2. **合并**：`git merge upstream/master`。因为合并基线是一次完整改名后的上游树，
   git 的改名检测会自动把上游改动映射到 `verdantgolem-*` 目录——绝大多数文件无需人工介入。
3. **自动策略**（仅在有冲突时生效）：
   - `crates/pumpkin-plugin-wit` 子模块指针 → 采上游（保持 WIT 契约对齐）。
   - 品牌/CI/部署文件 → 采我们侧（`README.md`、`Dockerfile`、`flake.nix`、`rust.yml` 矩阵等，
     完整清单见工作流内 `keep_ours` 数组）。
4. **干净合并** → 推送 `auto/upstream-sync` 分支并开 PR，**CI 全部通过后自动合入 master**；
   master 的 push 会再次触发发布流水线刷新 Nightly。
5. **有残留冲突** → 自动创建/更新带 `upstream-sync` 标签的 issue，列出全部冲突路径。
   此时不会推送任何半成品。

## 为什么以后冲突会很少

首次同步（改名）时合并基线在改名之前，产生大量路径冲突。此后每次同步的
合并基线都是"已改名且内容与上游一致"的树，git 改名检测按相似度映射即可，
只有"上游改动"与"我们的 carpet 接线/品牌改动"落在同一文件同一区域时才冲突。

## 预期会冲突的文件（需要人工看一眼）

- `Cargo.toml` / 各 crate `Cargo.toml`（上游加依赖 vs 我们的成员改名——内容合并不自动豁免）
- `assets/translations/*.json`（上游新增翻译键 vs 我们的品牌值改写）
- 承载 carpet 规则的核心文件（`tnt.rs`、`explosion.rs`、`hopper.rs`、`player.rs`、`world/mod.rs` 等）

出现这类冲突时以 issue 清单为准，逐个手动解：上游语义优先（§ 同步原则），
把我们的规则钩子按最新 API 重新挂上。

## 同步原则（不可推翻）

- 上游的同步 API 是骨架，carpet 钩子随之上移，不恢复旧接口。
- WIT 契约里的 Pumpkin 名称是刻意保留的 ABI 名称：
  `crates/pumpkin-plugin-wit`、`pumpkin:plugin`、`PUMPKIN_API_VERSION`、
  `PumpkinCustomData`、`pumpkin_block*`、`PumpkinServer`。
- 存档/配置兼容键不变：`pumpkin.toml`、`pumpkin_custom_data.nbt`、`pumpkin:` 命名空间值。

## 手动同步

```bash
git fetch upstream
git merge upstream/master
# 有冲突时参考 issue / 本文档策略解决
cargo 侧验证交给 GitHub CI（push 或 PR 触发）
```

也可在 GitHub Actions 页面手动触发 "Upstream Sync" 立即同步。
